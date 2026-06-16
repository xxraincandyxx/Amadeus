# TUI Test Replay & Observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the TUI testable and verifiable by an automated agent — a real headless driver that renders the actual `App`, a readable frame→text renderer, and a converter that turns a recorded `SessionLog` into a replayable `ScenarioMockClient` scenario.

**Architecture:** Three layers. (1) Move the scenario *data types* into `amadeus_core::test_utils` so both the converter (core) and the test mock (tests/) share one definition. (2) Add a generic `HeadlessApp<C: LLMClient>` in the `tui` crate behind `test-utils` that owns an `App<C>` + a `Terminal<TestBackend>`, drives it with the same `Session` methods the inline `app.rs` tests use, and captures real rendered buffers into `TuiFrameSnapshot`. (3) A pure converter `session_log_to_scenario(&SessionLog) -> ScenarioDefinition` that reads `timeline` `AgentEvent`s and groups them into one scripted step per assistant response. The harness never depends on `ScenarioMockClient`; integration tests supply it.

**Tech Stack:** Rust workspace (`crates/core`, `crates/tui`), ratatui `TestBackend`, tokio, serde_json. TDD per task. `cargo test -p <crate> --features test-utils`.

**Key facts established during research (do not re-litigate):**
- `Agent::builder(client, Arc::new(Config::default())).build()` → `App::new(agent, workdir: PathBuf, model_name: String)`. Client is injected via `Agent<C>`. (`crates/tui/src/ui/app.rs:3642`, `:4696`)
- `Session<C>` is private (`app.rs:657`); inline tests drive it via `session.input.handle_char(ch)`, `session.submit_input().await`, `session.render(frame)`, and read `session.stream_rx` (Some while a turn is streaming). (`app.rs:5277`, `:5281`, `:3509`, `:5304`)
- Canonical frame types: `amadeus_core::test_utils::testflow::types::{TuiFrameSnapshot, TuiCellSnapshot, TuiCursorSnapshot}`. (`crates/core/src/test_utils/testflow/types.rs:498`)
- `SessionRecorder::save()` writes `session_<ts>_<id>.json` as a `SessionLog`; `load_session(path) -> SessionLog` reads it. (`recorder.rs:366`, `:451`)
- `SessionLog { version, metadata, timeline: Vec<TimelineEvent>, summaries, snapshots }`; `TimelineEvent.event_type: RecordedEvent`; `RecordedEvent::AgentEvent { event: AgentEventData }`. (`types.rs:37`, `:170`)
- `AgentEventData` variants map to `StreamEventDef`: `TextDelta{delta}`, `ThinkingDelta{delta}`/`ThinkingComplete{thinking}`, `ToolStart{id,name,..}`, `ToolInputDelta{id,delta}`, `ToolComplete{id,..}`, `TokenUsage{input_tokens,output_tokens,..}`, `Done{..}`, `Error{message}`. (`types.rs` `enum AgentEventData`)
- Target scenario types today live in `tests/mocks/scenario_client.rs`: `ScenarioDefinition{name,description,steps}`, `ScenarioStepDef{delay_ms,events,error}`, `StreamEventDef` (serde `tag="type"`, `rename_all="snake_case"`). `ScenarioMockClient::from_json(&str)` + `from_steps`. (`tests/mocks/scenario_client.rs:43-217`)
- The existing `tests/tui/harness.rs`/`capture.rs`/`run_scenario` are **non-functional stubs** (always return empty/blank frames) — superseded by this plan; deprecated in Task 7.

---

## File Structure

**Create:**
- `crates/core/src/test_utils/scenario.rs` — scenario data types (`ScenarioDefinition`, `ScenarioStepDef`, `StreamEventDef`) + `From` impls. Pure data, no `LLMClient` dependency. (`#[cfg(any(test, feature = "test-utils"))]`)
- `crates/core/src/test_utils/replay.rs` — `session_log_to_scenario(&SessionLog) -> ScenarioDefinition`. (`#[cfg(any(test, feature = "test-utils"))]`)
- `crates/core/src/test_utils/frame_text.rs` — `render_frame_text(&TuiFrameSnapshot) -> String`. (`#[cfg(any(test, feature = "test-utils"))]`)
- `crates/tui/src/ui/headless.rs` — `HeadlessApp<C>` generic driver. (`#[cfg(feature = "test-utils")] pub`)
- `examples/convert_session.rs` — CLI to convert a recorded `session_*.json` into a scenario JSON.
- `tests/tui/scenarios/text_turn.json`, `tests/tui/scenarios/tool_turn.json` — seed replay fixtures.
- `tests/tui_replay_test.rs` — end-to-end: load fixture → scenario → `HeadlessApp` → render → assert text.

**Modify:**
- `crates/core/src/test_utils/mod.rs` — declare the three new submodules.
- `crates/tui/src/ui/mod.rs` — declare `#[cfg(feature = "test-utils")] pub mod headless;`.
- `crates/tui/src/ui/app.rs` — add a `#[cfg(feature = "test-utils")] impl` block exposing `pub(crate)` drive wrappers on `App`/`Session`.
- `tests/mocks/scenario_client.rs` — re-export the data types from core; keep `ScenarioMockClient` + add `from_definition`.
- `tests/tui/harness.rs` — add a deprecation note pointing at `HeadlessApp` (Task 7).

---

## Task 1: Move scenario data types into `amadeus_core::test_utils`

**Why:** The converter (core) must return `ScenarioDefinition`, but that type currently lives in a test file (`tests/mocks/scenario_client.rs`) which no crate can import. Move only the *data* types into core; `ScenarioMockClient` (which needs `LLMClient`) stays in the test file and re-exports them.

**Files:**
- Create: `crates/core/src/test_utils/scenario.rs`
- Modify: `crates/core/src/test_utils/mod.rs`
- Modify: `tests/mocks/scenario_client.rs` (re-export + add `from_definition`)
- Test: `crates/core/src/test_utils/scenario.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Write the failing test**

Create `crates/core/src/test_utils/scenario.rs` with only the test first (types not yet defined will fail to compile — that is the failure):

```rust
// @amadeus-header
// summary: Replay scenario data types shared by the mock client and the session converter.
// layer: test
// status: test-only
// feature_flags:
// - test-utils
// provides:
// - module: crate::test_utils::scenario
// - type: crate::test_utils::scenario::ScenarioDefinition
// - type: crate::test_utils::scenario::ScenarioStepDef
// - type: crate::test_utils::scenario::StreamEventDef
// uses:
// - module: crate::client
// - protocol: serde serialization
// invariants:
// - JSON wire format stays identical to the historical tests/mocks/scenario_client.rs layout.
// side_effects: none
// tests:
// - cmd: cargo test -p core --features test-utils scenario
// @end-amadeus-header

//! Data-only scenario types. `ScenarioMockClient` (which implements `LLMClient`)
//! remains in `tests/mocks/scenario_client.rs` and re-exports these.

use serde::{Deserialize, Serialize};

use crate::client::StreamEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioDefinition {
    pub name: String,
    pub description: String,
    pub steps: Vec<ScenarioStepDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioStepDef {
    pub delay_ms: Option<u64>,
    pub events: Vec<StreamEventDef>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEventDef {
    TextDelta { text: String },
    ThinkingDelta { text: String },
    ToolCallStart { id: String, name: String },
    ToolCallDelta { arguments: String },
    ToolCallDone { id: String },
    StopReason { reason: String },
    TokenUsage { input_tokens: u32, output_tokens: u32 },
}

impl From<StreamEventDef> for StreamEvent {
    fn from(def: StreamEventDef) -> Self {
        match def {
            StreamEventDef::TextDelta { text } => StreamEvent::TextDelta(text),
            StreamEventDef::ThinkingDelta { text } => StreamEvent::ThinkingDelta(text),
            StreamEventDef::ToolCallStart { id, name } => StreamEvent::ToolCallStart { id, name },
            StreamEventDef::ToolCallDelta { arguments } => StreamEvent::ToolCallDelta { arguments },
            StreamEventDef::ToolCallDone { id } => StreamEvent::ToolCallDone(id),
            StreamEventDef::StopReason { reason } => StreamEvent::StopReason(reason),
            StreamEventDef::TokenUsage { input_tokens, output_tokens } => StreamEvent::TokenUsage {
                input_tokens,
                output_tokens,
            },
        }
    }
}

impl From<StreamEvent> for StreamEventDef {
    fn from(event: StreamEvent) -> Self {
        match event {
            StreamEvent::TextDelta(text) => StreamEventDef::TextDelta { text },
            StreamEvent::ThinkingDelta(text) => StreamEventDef::ThinkingDelta { text },
            StreamEvent::ToolCallStart { id, name } => StreamEventDef::ToolCallStart { id, name },
            StreamEvent::ToolCallDelta { arguments } => StreamEventDef::ToolCallDelta { arguments },
            StreamEvent::ToolCallDone(id) => StreamEventDef::ToolCallDone { id },
            StreamEvent::StopReason(reason) => StreamEventDef::StopReason { reason },
            StreamEvent::TokenUsage { input_tokens, output_tokens } => {
                StreamEventDef::TokenUsage { input_tokens, output_tokens }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_definition_round_trips() {
        let def = ScenarioDefinition {
            name: "t".to_string(),
            description: "d".to_string(),
            steps: vec![ScenarioStepDef {
                delay_ms: None,
                events: vec![
                    StreamEventDef::TextDelta { text: "Hi".to_string() },
                    StreamEventDef::StopReason { reason: "end_turn".to_string() },
                ],
                error: None,
            }],
        };
        let json = serde_json::to_string(&def).unwrap();
        let back: ScenarioDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(back.steps.len(), 1);
        assert_eq!(back.steps[0].events.len(), 2);
    }

    #[test]
    fn wire_format_uses_snake_case_type_tag() {
        let json = r#"{"type":"text_delta","text":"x"}"#;
        let ev: StreamEventDef = serde_json::from_str(json).unwrap();
        assert!(matches!(ev, StreamEventDef::TextDelta { text } if text == "x"));
    }
}
```

- [ ] **Step 2: Declare the module and run the test to verify it fails**

In `crates/core/src/test_utils/mod.rs`, add inside the existing `#[cfg(any(test, feature = "test-utils"))] pub mod ...` group:

```rust
pub mod scenario;
```

(Place it next to the other `pub mod` lines already gated by the same cfg. If `mod.rs` declares modules without an outer cfg gate, wrap this line: `#[cfg(any(test, feature = "test-utils"))] pub mod scenario;`.)

Run: `cargo test -p core --features test-utils scenario -- --nocapture`
Expected: PASS (the types and tests are defined in the same file). If you added only the test first it would FAIL to compile; since the file above includes both, this confirms the module wires up.

- [ ] **Step 3: Re-export the types from the test mock and add `from_definition`**

In `tests/mocks/scenario_client.rs`, **delete** the local definitions of `ScenarioDefinition`, `ScenarioStepDef`, `StreamEventDef`, and both `From` impls (lines ~43–123), and replace the top of the file's type section with a re-export. Keep `CapturedRequest`, `ScenarioMockClient`, and its `impl`/`LLMClient` blocks unchanged except for the new constructor.

Replace the block from `#[derive(Debug, Clone, Serialize, Deserialize)]\npub struct ScenarioDefinition {` through the end of `impl From<StreamEvent> for StreamEventDef { ... }` with:

```rust
pub use amadeus::test_utils::scenario::{
    ScenarioDefinition, ScenarioStepDef, StreamEventDef,
};
```

Then add one constructor to the `impl ScenarioMockClient` block (next to `from_json`):

```rust
    pub fn from_definition(def: ScenarioDefinition) -> Self {
        Self {
            steps: Arc::new(Mutex::new(def.steps.into_iter().collect())),
            captured_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
```

- [ ] **Step 4: Verify the whole test suite that touched scenarios still passes**

Run: `cargo test --features full`
Expected: PASS — existing `tests/mocks/scenario_client.rs` tests and `test_scenario_from_json` still pass because the re-exported types are wire-compatible (identical serde attributes).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/test_utils/scenario.rs crates/core/src/test_utils/mod.rs tests/mocks/scenario_client.rs
git commit -m "refactor(test): move scenario data types into amadeus_core::test_utils"
```

---

## Task 2: Frame→text renderer

**Why:** `TuiFrameSnapshot` cells are JSON; an agent verifier needs a compact human/agent-readable rendering of a frame (and a filmstrip of many). The existing `tests/tui/capture.rs::to_terminal_view` is on a *different* stub type and truncates to 10×40. Build the real one on the canonical type.

**Files:**
- Create: `crates/core/src/test_utils/frame_text.rs`
- Modify: `crates/core/src/test_utils/mod.rs`
- Test: inline in `frame_text.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/core/src/test_utils/frame_text.rs`:

```rust
// @amadeus-header
// summary: Readable text rendering of a captured TUI frame for agent verification.
// layer: test
// status: test-only
// feature_flags:
// - test-utils
// provides:
// - fn: crate::test_utils::frame_text::render_frame_text
// - fn: crate::test_utils::frame_text::render_frames_filmstrip
// uses:
// - type: crate::test_utils::testflow::types::TuiFrameSnapshot
// invariants:
// - Output is deterministic for a given snapshot (no timing/random data).
// side_effects: none
// tests:
// - cmd: cargo test -p core --features test-utils frame_text
// @end-amadeus-header

//! Render captured TUI frames as plain text so a human or agent can read them.

use crate::test_utils::testflow::types::{TuiCellSnapshot, TuiFrameSnapshot};

/// Render a single frame as text: a header line, one line per terminal row
/// (trailing spaces trimmed), and a cursor line. Deterministic.
pub fn render_frame_text(snapshot: &TuiFrameSnapshot) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# frame {} ({}x{} @{})\n",
        snapshot.frame_id, snapshot.width, snapshot.height, snapshot.timestamp_ms
    ));

    let mut grid: Vec<Vec<char>> =
        (0..snapshot.height).map(|_| vec![' '; snapshot.width as usize]).collect();
    for cell in &snapshot.cells {
        if cell.y < snapshot.height && cell.x < snapshot.width {
            let sym = cell.symbol.chars().next().unwrap_or(' ');
            grid[cell.y as usize][cell.x as usize] = sym;
        }
    }
    for row in grid {
        let line: String = row.iter().collect();
        out.push_str(line.trim_end());
        out.push('\n');
    }

    if let Some(cursor) = &snapshot.cursor {
        if cursor.visible {
            out.push_str(&format!("^ cursor @ ({},{})\n", cursor.x, cursor.y));
        }
    }
    out
}

/// Render many frames as a filmstrip, frame separated by a blank line.
pub fn render_frames_filmstrip<'a, I>(snapshots: I) -> String
where
    I: IntoIterator<Item = &'a TuiFrameSnapshot>,
{
    let mut out = String::new();
    for snap in snapshots {
        out.push_str(&render_frame_text(snap));
        out.push('\n');
    }
    out
}

/// Build a snapshot from a simple text grid (test helper / fixture builder).
pub fn snapshot_from_text(frame_id: u64, rows: &[&str]) -> TuiFrameSnapshot {
    let height = rows.len() as u16;
    let width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as u16;
    let mut cells = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if ch != ' ' {
                cells.push(TuiCellSnapshot {
                    x: x as u16,
                    y: y as u16,
                    symbol: ch.to_string(),
                    fg: "default".to_string(),
                    bg: "default".to_string(),
                    underline_color: "default".to_string(),
                    add_modifier: String::new(),
                    sub_modifier: String::new(),
                });
            }
        }
    }
    TuiFrameSnapshot {
        session_id: "test".to_string(),
        frame_id,
        timestamp_ms: 0,
        width,
        height,
        cursor: None,
        cells,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_rows_and_trims_trailing_spaces() {
        let snap = snapshot_from_text(3, &["hello", "hi   ", "world"]);
        let text = render_frame_text(&snap);
        assert!(text.contains("# frame 3"));
        assert!(text.contains("hello\n"));
        assert!(text.contains("\nhi\n"), "trailing spaces trimmed: {text:?}");
        assert!(text.ends_with("world\n"));
    }

    #[test]
    fn filmstrip_separates_frames() {
        let a = snapshot_from_text(0, &["a"]);
        let b = snapshot_from_text(1, &["b"]);
        let text = render_frames_filmstrip(&[a, b]);
        assert_eq!(text.matches("# frame").count(), 2);
    }
}
```

- [ ] **Step 2: Declare the module**

In `crates/core/src/test_utils/mod.rs` add (same cfg gate as Task 1):

```rust
pub mod frame_text;
```

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test -p core --features test-utils frame_text -- --nocapture`
Expected: PASS (2 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/test_utils/frame_text.rs crates/core/src/test_utils/mod.rs
git commit -m "feat(test): add render_frame_text for readable TUI frame dumps"
```

---

## Task 3: Expose a minimal `Session`/`App` drive surface behind `test-utils`

**Why:** `HeadlessApp` (Task 4) lives in a sibling module and must call private `Session` methods. Add a single gated `impl` block with `pub(crate)` wrappers — no change to existing signatures, so no impact on production builds.

**Files:**
- Modify: `crates/tui/src/ui/app.rs` (add one `#[cfg(feature = "test-utils")] impl` block)

- [ ] **Step 1: Add the gated drive wrappers**

Append this block to `crates/tui/src/ui/app.rs` (after the existing `impl<C: LLMClient + Clone + 'static> App<C>` block, before `mod tests`):

```rust
#[cfg(feature = "test-utils")]
impl<C: LLMClient + Clone + 'static> App<C> {
    /// Test-only accessor for the active session (headless driver).
    pub(crate) fn test_session_mut(&mut self) -> &mut Session<C> {
        self.active_session_mut()
    }
}

#[cfg(feature = "test-utils")]
impl<C: LLMClient + Clone + 'static> Session<C> {
    /// Type one character into the input box.
    pub(crate) fn test_type_char(&mut self, c: char) {
        self.input.handle_char(c);
    }

    /// Read the current input text.
    pub(crate) fn test_input_text(&self) -> String {
        self.input.get_input()
    }

    /// Submit the current input (starts a prompt turn or runs a slash command).
    pub(crate) async fn test_submit(&mut self) -> Result<()> {
        self.submit_input().await
    }

    /// True while a prompt turn is streaming (used to await settle).
    pub(crate) fn test_is_streaming(&self) -> bool {
        self.stream_rx.is_some()
    }

    /// Render the session into a ratatui frame (TestBackend draws into a buffer).
    pub(crate) fn test_render(&mut self, frame: &mut ratatui::Frame) {
        self.render(frame);
    }
}
```

Note: `submit_input` returns the crate `Result<()>` (verified at `app.rs:3260`), and `Result` is in scope in the tui crate via the `amadeus_core` glob re-export, so the bare `Result<()>` above matches exactly.

- [ ] **Step 2: Verify it compiles under test-utils and production is unaffected**

Run: `cargo check -p tui --features test-utils`
Expected: compiles.

Run: `cargo check -p tui`
Expected: compiles (the block is cfg-gated out).

- [ ] **Step 3: Commit**

```bash
git add crates/tui/src/ui/app.rs
git commit -m "feat(tui): expose test-utils drive wrappers on App/Session"
```

---

## Task 4: `HeadlessApp<C>` generic headless driver

**Why:** The reusable enabler. Owns `App<C>` + `Terminal<TestBackend>`, drives input, settles async turns, and captures real rendered buffers into `TuiFrameSnapshot`. Generic over `C: LLMClient` so it never imports the test-only mock.

**Files:**
- Create: `crates/tui/src/ui/headless.rs`
- Modify: `crates/tui/src/ui/mod.rs` (declare module)
- Test: `crates/tui/src/ui/headless.rs` (inline) using `BenchmarkMockClient` (already available to the crate as `crate::benchmark::mock::BenchmarkMockClient`)

- [ ] **Step 1: Write the failing test**

Create `crates/tui/src/ui/headless.rs`:

```rust
// @amadeus-header
// summary: Headless TUI driver that renders the real App against a TestBackend.
// layer: ui
// status: test-only
// feature_flags:
// - test-utils
// provides:
// - module: crate::ui::headless
// - type: crate::ui::headless::HeadlessApp
// uses:
// - module: crate::ui::app
// - type: amadeus_core::test_utils::testflow::types::TuiFrameSnapshot
// invariants:
// - Captured frames reflect the real App render, not stub data.
// side_effects: none
// tests:
// - cmd: cargo test -p tui --features test-utils headless
// @end-amadeus-header

//! Reusable headless driver for TUI tests. Integration tests supply the
//! `LLMClient` (e.g. `ScenarioMockClient`); this struct stays client-agnostic.

use std::sync::Arc;

use ratatui::{backend::TestBackend, Terminal};

use amadeus_core::agent::config::Config;
use amadeus_core::agent::loop_agent::Agent;
use amadeus_core::client::LLMClient;
use amadeus_core::test_utils::frame_text::render_frame_text;
use amadeus_core::test_utils::testflow::types::{TuiCellSnapshot, TuiFrameSnapshot};

use super::app::App;

pub struct HeadlessApp<C: LLMClient + Clone + 'static> {
    app: App<C>,
    terminal: Terminal<TestBackend>,
    width: u16,
    height: u16,
    frame_counter: u64,
}

impl<C: LLMClient + Clone + 'static> HeadlessApp<C> {
    pub fn new(client: C, workdir: &str, model: &str, width: u16, height: u16) -> Self {
        let agent = Agent::builder(client, Arc::new(Config::default())).build();
        let app = App::new(agent, workdir.into(), model.to_string());
        let terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
        Self { app, terminal, width, height, frame_counter: 0 }
    }

    /// Type a string into the input box (no submission).
    pub fn type_text(&mut self, text: &str) {
        let session = self.app.test_session_mut();
        for c in text.chars() {
            session.test_type_char(c);
        }
    }

    /// Submit current input and await the turn to settle.
    pub async fn submit(&mut self) {
        {
            let session = self.app.test_session_mut();
            let _ = session.test_submit().await;
        }
        self.settle().await;
    }

    /// Pump until the streaming turn finishes (or we give up after many yields).
    async fn settle(&mut self) {
        for _ in 0..10_000 {
            if !self.app.test_session_mut().test_is_streaming() {
                return;
            }
            tokio::task::yield_now().await;
        }
    }

    /// Render the current state and return a populated snapshot + its text form.
    pub fn capture(&mut self) -> (TuiFrameSnapshot, String) {
        let width = self.width;
        let height = self.height;
        self.terminal
            .draw(|frame| {
                self.app.test_session_mut().test_render(frame);
            })
            .expect("draw");

        let buffer = self.terminal.backend().buffer();
        let mut cells = Vec::with_capacity((width as usize) * (height as usize));
        for y in 0..height {
            for x in 0..width {
                let cell = &buffer[(x, y)];
                let style = cell.style();
                cells.push(TuiCellSnapshot {
                    x,
                    y,
                    symbol: cell.symbol().to_string(),
                    fg: color_to_string(style.fg),
                    bg: color_to_string(style.bg),
                    underline_color: color_to_string(style.underline_color),
                    add_modifier: format!("{:?}", style.add_modifier),
                    sub_modifier: format!("{:?}", style.sub_modifier),
                });
            }
        }
        let frame_id = self.frame_counter;
        self.frame_counter += 1;
        let snapshot = TuiFrameSnapshot {
            session_id: "headless".to_string(),
            frame_id,
            timestamp_ms: 0,
            width,
            height,
            cursor: None,
            cells,
        };
        let text = render_frame_text(&snapshot);
        (snapshot, text)
    }
}

fn color_to_string(color: Option<ratatui::style::Color>) -> String {
    match color {
        Some(c) => format!("{:?}", c),
        None => "default".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use amadeus_core::benchmark::case::MockScript;
    use amadeus_core::benchmark::mock::BenchmarkMockClient;

    #[tokio::test]
    async fn typing_then_capture_shows_typed_text() {
        let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
        let mut app = HeadlessApp::new(client, ".", "m", 40, 6);
        app.type_text("hello");
        let (_snap, text) = app.capture();
        assert!(text.contains("hello"), "capture should show typed text:\n{text}");
    }

    #[test]
    fn capture_returns_full_grid_dimensions() {
        let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
        let mut app = HeadlessApp::new(client, ".", "m", 12, 3);
        let (snap, _text) = app.capture();
        assert_eq!(snap.width, 12);
        assert_eq!(snap.height, 3);
        assert_eq!(snap.cells.len(), 12 * 3);
    }
}
```

- [ ] **Step 2: Declare the module**

In `crates/tui/src/ui/mod.rs`, add (place near the other `pub mod` declarations):

```rust
#[cfg(feature = "test-utils")]
pub mod headless;
```

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test -p tui --features test-utils headless -- --nocapture`
Expected: PASS (2 tests). If the "typed text appears" assertion fails, inspect `text` output: the input component may render the cursor/box on a specific row — adjust the assertion substring to a visible token (e.g. the first typed char). The grid-dimension test must pass regardless.

If `BenchmarkMockClient`/`MockScript` paths differ (confirm via `crates/core/src/benchmark/mock.rs:39` and `case.rs`), adjust the `use` lines accordingly.

- [ ] **Step 4: Commit**

```bash
git add crates/tui/src/ui/headless.rs crates/tui/src/ui/mod.rs
git commit -m "feat(tui): add HeadlessApp test-utils driver over TestBackend"
```

---

## Task 5: `session_log_to_scenario` converter

**Why:** Turns a recorded `SessionLog` (from `SessionRecorder::save()` / `load_session`) into a `ScenarioDefinition` the mock can replay. Source = `timeline` `AgentEvent`s (the only place thinking + tool calls are both captured). Group one `ScenarioStepDef` per assistant response: split at `ToolComplete` (tool finishes → next response) and close on `Done`/`Error`.

**Files:**
- Create: `crates/core/src/test_utils/replay.rs`
- Modify: `crates/core/src/test_utils/mod.rs`
- Test: inline in `replay.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/core/src/test_utils/replay.rs`:

```rust
// @amadeus-header
// summary: Convert a recorded SessionLog into a replayable ScenarioDefinition.
// layer: test
// status: test-only
// feature_flags:
// - test-utils
// provides:
// - fn: crate::test_utils::replay::session_log_to_scenario
// uses:
// - type: crate::test_utils::testflow::types::SessionLog
// - type: crate::test_utils::scenario::ScenarioDefinition
// invariants:
// - One step per assistant response; thinking and tool_use are preserved.
// side_effects: none
// tests:
// - cmd: cargo test -p core --features test-utils replay
// @end-amadeus-header

//! Reconstruct a scripted scenario from a recorded session timeline.

use crate::test_utils::scenario::{ScenarioDefinition, ScenarioStepDef, StreamEventDef};
use crate::test_utils::testflow::types::{AgentEventData, RecordedEvent, SessionLog};

/// Convert a recorded session into a replayable scenario.
///
/// Grouping: accumulate `AgentEvent`s into the current step. `ToolComplete`
/// closes the current step (stop reason `tool_use`) because the next agent
/// response is a fresh LLM call. `Done` closes with `end_turn`; `Error` closes
/// with a step-level error.
pub fn session_log_to_scenario(log: &SessionLog) -> ScenarioDefinition {
    let mut steps: Vec<ScenarioStepDef> = Vec::new();
    let mut current = ScenarioStepDef { delay_ms: None, events: Vec::new(), error: None };
    let mut started = false;

    let close = |step: &ScenarioStepDef, reason: &str| -> ScenarioStepDef {
        let mut s = ScenarioStepDef { delay_ms: None, events: step.events.clone(), error: step.error.clone() };
        s.events.push(StreamEventDef::StopReason { reason: reason.to_string() });
        s
    };

    for event in &log.timeline {
        let RecordedEvent::AgentEvent { event } = &event.event_type else { continue };
        started = true;
        match event {
            AgentEventData::TextDelta { delta } => {
                current.events.push(StreamEventDef::TextDelta { text: delta.clone() });
            }
            AgentEventData::ThinkingDelta { delta } => {
                current.events.push(StreamEventDef::ThinkingDelta { text: delta.clone() });
            }
            AgentEventData::ThinkingComplete { thinking } => {
                current.events.push(StreamEventDef::ThinkingDelta { text: thinking.clone() });
            }
            AgentEventData::ToolStart { id, name, .. } => {
                current.events.push(StreamEventDef::ToolCallStart { id: id.clone(), name: name.clone() });
            }
            AgentEventData::ToolInputDelta { delta, .. } => {
                // ToolInputDelta carries { id, delta, parent_id }; forward as call args.
                current.events.push(StreamEventDef::ToolCallDelta { arguments: delta.clone() });
            }
            AgentEventData::ToolComplete { id, input, .. } => {
                current.events.push(StreamEventDef::ToolCallDelta { arguments: input.to_string() });
                current.events.push(StreamEventDef::ToolCallDone { id: id.clone() });
                steps.push(close(&current, "tool_use"));
                current = ScenarioStepDef { delay_ms: None, events: Vec::new(), error: None };
            }
            AgentEventData::TokenUsage { input_tokens, output_tokens, .. } => {
                current.events.push(StreamEventDef::TokenUsage {
                    input_tokens: *input_tokens,
                    output_tokens: *output_tokens,
                });
            }
            AgentEventData::Done { .. } => {
                steps.push(close(&current, "end_turn"));
                current = ScenarioStepDef { delay_ms: None, events: Vec::new(), error: None };
            }
            AgentEventData::Error { message } => {
                current.error = Some(message.clone());
                steps.push(current.clone());
                current = ScenarioStepDef { delay_ms: None, events: Vec::new(), error: None };
            }
            _ => {}
        }
    }

    if started && (!current.events.is_empty() || current.error.is_some()) {
        steps.push(close(&current, "end_turn"));
    }

    ScenarioDefinition {
        name: log.metadata.session_id.clone(),
        description: format!("Converted from session {}", log.metadata.session_id),
        steps,
    }
}

#[cfg(test)]
mod tests {
    use super::session_log_to_scenario;
    use crate::test_utils::scenario::StreamEventDef;
    use crate::test_utils::testflow::types::{
        AgentEventData, RecordedEvent, SessionLog, SessionMetadata, TimelineEvent,
    };

    fn agent(ev: AgentEventData) -> TimelineEvent {
        TimelineEvent {
            seq: 0,
            timestamp_ms: 0,
            event_type: RecordedEvent::AgentEvent { event: ev },
        }
    }

    fn empty_log() -> SessionLog {
        SessionLog {
            version: "1".to_string(),
            metadata: SessionMetadata { session_id: "sess_x".to_string(), ..Default::default() },
            timeline: Vec::new(),
            summaries: Default::default(),
            snapshots: Default::default(),
        }
    }

    #[test]
    fn single_text_turn_becomes_one_step() {
        let mut log = empty_log();
        log.timeline.push(agent(AgentEventData::TextDelta { delta: "Hi".to_string() }));
        log.timeline.push(agent(AgentEventData::Done { text: "Hi".to_string(), tool_call_count: 0 }));
        let def = session_log_to_scenario(&log);
        assert_eq!(def.steps.len(), 1);
        assert!(def.steps[0].events.iter().any(|e| matches!(e, StreamEventDef::TextDelta { text } if text == "Hi")));
        assert!(def.steps[0].events.iter().any(|e| matches!(e, StreamEventDef::StopReason { reason } if reason == "end_turn")));
    }

    #[test]
    fn tool_turn_splits_into_two_steps() {
        let mut log = empty_log();
        log.timeline.push(agent(AgentEventData::TextDelta { delta: "running".to_string() }));
        log.timeline.push(agent(AgentEventData::ToolStart { id: "t1".to_string(), name: "bash".to_string(), command: None, parent_id: None }));
        log.timeline.push(agent(AgentEventData::ToolComplete { id: "t1".to_string(), name: "bash".to_string(), input: serde_json::json!({"cmd":"ls"}), output: "out".to_string(), is_error: false, parent_id: None }));
        log.timeline.push(agent(AgentEventData::TextDelta { delta: "done".to_string() }));
        log.timeline.push(agent(AgentEventData::Done { text: "done".to_string(), tool_call_count: 1 }));
        let def = session_log_to_scenario(&log);
        assert_eq!(def.steps.len(), 2, "tool_use turn then follow-up");
        assert!(def.steps[0].events.iter().any(|e| matches!(e, StreamEventDef::ToolCallStart { name, .. } if name == "bash")));
    }

    #[test]
    fn thinking_is_preserved() {
        let mut log = empty_log();
        log.timeline.push(agent(AgentEventData::ThinkingDelta { delta: "hmm".to_string() }));
        log.timeline.push(agent(AgentEventData::TextDelta { delta: "ok".to_string() }));
        log.timeline.push(agent(AgentEventData::Done { text: "ok".to_string(), tool_call_count: 0 }));
        let def = session_log_to_scenario(&log);
        assert!(def.steps[0].events.iter().any(|e| matches!(e, StreamEventDef::ThinkingDelta { text } if text == "hmm")));
    }
}
```

- [ ] **Step 2: Declare the module**

In `crates/core/src/test_utils/mod.rs` add (same cfg gate):

```rust
pub mod replay;
```

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test -p core --features test-utils replay -- --nocapture`
Expected: PASS (3 tests).

**If compile fails on a `TimelineEvent` or `SessionMetadata` field name** (e.g. `timestamp` vs `ts`, or `SessionMetadata` lacks `session_id`): read `crates/core/src/test_utils/testflow/types.rs` lines 37–130 and 170–210 and correct the field names in the test builder. The conversion logic in `session_log_to_scenario` only reads `log.timeline`, `event.event_type`, `event.event` (the `AgentEventData`), and `log.metadata.session_id` — keep those aligned with the real struct.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/test_utils/replay.rs crates/core/src/test_utils/mod.rs
git commit -m "feat(test): add session_log_to_scenario replay converter"
```

---

## Task 6: End-to-end glue + `convert_session` example

**Why:** Prove the loop end-to-end (scenario JSON → `HeadlessApp` → rendered text) and give the user a CLI to turn a real recorded `session_*.json` into a scenario.

**Files:**
- Create: `examples/convert_session.rs`
- Create: `tests/tui_replay_test.rs`

- [ ] **Step 1: Write the end-to-end test**

Create `tests/tui_replay_test.rs`:

```rust
// @amadeus-header
// summary: End-to-end replay: scenario -> HeadlessApp -> rendered text assertion.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::tui_replay_test
// uses:
// - module: tests::mocks::scenario_client
// - module: amadeus::ui::headless
// invariants: none
// side_effects: none
// tests:
// - cmd: cargo test --features full --test tui_replay_test
// @end-amadeus-header

mod mocks {
    pub mod scenario_client; // existing helper module include
}

use std::sync::Arc;

use amadeus::agent::config::Config;
use amadeus::agent::loop_agent::Agent;
use amadeus::test_utils::scenario::{ScenarioDefinition, ScenarioStepDef, StreamEventDef};
use amadeus::ui::headless::HeadlessApp;

use mocks::scenario_client::ScenarioMockClient;

fn text_turn() -> ScenarioDefinition {
    ScenarioDefinition {
        name: "text".to_string(),
        description: "one text turn".to_string(),
        steps: vec![ScenarioStepDef {
            delay_ms: None,
            events: vec![
                StreamEventDef::TextDelta { text: "Hello from the mock".to_string() },
                StreamEventDef::StopReason { reason: "end_turn".to_string() },
            ],
            error: None,
        }],
    }
}

#[tokio::test]
async fn scenario_drives_app_and_renders_assistant_text() {
    let client = ScenarioMockClient::from_definition(text_turn());
    let _ = Agent::builder(client.clone(), Arc::new(Config::default())).build(); // sanity: client is an Agent client
    let mut app = HeadlessApp::new(client, ".", "test-model", 60, 8);
    app.type_text("hi");
    app.submit().await;

    let (_snap, text) = app.capture();
    assert!(
        text.contains("Hello from the mock"),
        "rendered frame should contain assistant output:\n{text}"
    );
}
```

Note: the `mod mocks { pub mod scenario_client; }` path assumes an existing `tests/mocks/scenario_client.rs` included this way. If the repo instead includes it via `#[path = "mocks/scenario_client.rs"]`, mirror whatever the existing `tests/tui_snapshot_test.rs` uses (read its top lines). `ScenarioMockClient` must be `Clone` (it is — `#[derive(Clone)]` at `scenario_client.rs:133`).

- [ ] **Step 2: Run the test to verify it passes**

Run: `cargo test --features full --test tui_replay_test -- --nocapture`
Expected: PASS. If the assistant text does not appear in the captured frame, the turn may not have settled before capture — increase the `settle()` iteration budget or add a short `tokio::time::sleep` inside `settle()`. Inspect `text` to see what actually rendered.

- [ ] **Step 3: Create the converter CLI example**

Create `examples/convert_session.rs`:

```rust
// @amadeus-header
// summary: CLI to convert a recorded session_*.json into a scenario JSON.
// layer: example
// status: active
// feature_flags:
// - test-utils
// provides:
// - module: example::convert_session
// uses:
// - fn: amadeus::test_utils::replay::session_log_to_scenario
// invariants: none
// side_effects:
// - Reads an input file; writes stdout.
// tests:
// - cmd: cargo run --example convert_session --features test-utils -- path/to/session.json
// @end-amadeus-header

//! Usage: convert_session <session_log.json>
//! Prints a ScenarioDefinition JSON to stdout.

use std::path::PathBuf;

use amadeus::test_utils::replay::session_log_to_scenario;
use amadeus::test_utils::testflow::recorder::load_session;

fn main() -> anyhow::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: convert_session <session_log.json>");
    let log = load_session(&path)?;
    let scenario = session_log_to_scenario(&log);
    println!("{}", serde_json::to_string_pretty(&scenario)?);
    Ok(())
}
```

- [ ] **Step 4: Verify the example builds**

Run: `cargo build --example convert_session --features test-utils`
Expected: compiles. (If `anyhow` isn't an example dep, replace `anyhow::Result` with `Result<(), Box<dyn std::error::Error>>`.)

- [ ] **Step 5: Commit**

```bash
git add examples/convert_session.rs tests/tui_replay_test.rs
git commit -m "feat(test): end-to-end TUI replay test + convert_session example"
```

---

## Task 7: Seed scenario corpus + deprecate the stub harness

**Why:** Establish `tests/tui/scenarios/` as the canonical, data-driven scenario home and point future authors away from the non-functional `tests/tui/harness.rs`.

**Files:**
- Create: `tests/tui/scenarios/text_turn.json`
- Create: `tests/tui/scenarios/tool_turn.json`
- Modify: `tests/tui/harness.rs` (deprecation note)

- [ ] **Step 1: Create the seed fixtures**

Create `tests/tui/scenarios/text_turn.json`:

```json
{
  "name": "text_turn",
  "description": "Assistant replies with one text message and stops.",
  "steps": [
    {
      "delay_ms": null,
      "error": null,
      "events": [
        { "type": "text_delta", "text": "Sure — here is a one-line answer." },
        { "type": "stop_reason", "reason": "end_turn" }
      ]
    }
  ]
}
```

Create `tests/tui/scenarios/tool_turn.json`:

```json
{
  "name": "tool_turn",
  "description": "Assistant calls bash, then answers.",
  "steps": [
    {
      "delay_ms": null,
      "error": null,
      "events": [
        { "type": "text_delta", "text": "Let me check." },
        { "type": "tool_call_start", "id": "call_1", "name": "bash" },
        { "type": "tool_call_delta", "arguments": "{\"command\":\"echo hi\"}" },
        { "type": "tool_call_done", "id": "call_1" },
        { "type": "stop_reason", "reason": "tool_use" }
      ]
    },
    {
      "delay_ms": null,
      "error": null,
      "events": [
        { "type": "text_delta", "text": "It printed hi." },
        { "type": "stop_reason", "reason": "end_turn" }
      ]
    }
  ]
}
```

- [ ] **Step 2: Add a fixture-driven test that loads JSON via `from_json`**

Append to `tests/tui_replay_test.rs` (Task 6 file):

```rust
#[tokio::test]
async fn loads_text_turn_fixture_from_json() {
    let json = std::fs::read_to_string("tests/tui/scenarios/text_turn.json").unwrap();
    let client = ScenarioMockClient::from_json(&json).expect("parse fixture");
    let mut app = HeadlessApp::new(client, ".", "test-model", 60, 8);
    app.type_text("ping");
    app.submit().await;
    let (_snap, text) = app.capture();
    assert!(text.contains("one-line answer"), "frame:\n{text}");
}
```

- [ ] **Step 3: Deprecate the stub harness**

At the top of `tests/tui/harness.rs`, after the header block, add:

```rust
//! **Deprecated.** This harness captures blank/empty frames and does not drive
//! the real `App`. Use `amadeus::ui::headless::HeadlessApp` (feature
//! `test-utils`) instead. Retained only until callers migrate.
```

- [ ] **Step 4: Run the full relevant test set**

Run: `cargo test --features full --test tui_replay_test && cargo test -p core --features test-utils && cargo test -p tui --features test-utils`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/tui/scenarios tests/tui_replay_test.rs tests/tui/harness.rs
git commit -m "test(tui): seed scenario corpus; deprecate stub harness"
```

---

## Self-Review

**1. Spec coverage** (the three deliverables the user chose):
- Real headless driver → Task 3 (expose surface) + Task 4 (`HeadlessApp`). ✓
- Frame→text renderer → Task 2. ✓
- Transcript/session → scenario converter → Task 1 (shared types) + Task 5 (converter) + Task 6 (CLI). ✓
- Corpus seeding + cleanup → Task 7. ✓

**2. Placeholder scan:** No "TBD"/"add error handling"/"similar to Task N". Every code step contains full code. The two spots flagged for engineer verification (Task 3 `test_submit` return type; Task 5 struct field names) give the exact source line to read and the exact fallback — not placeholders, but explicit "confirm against line N" because those signatures were read from grep, not verified by compilation.

**3. Type consistency:** `ScenarioDefinition`/`ScenarioStepDef`/`StreamEventDef` defined once (Task 1) and reused by the converter (Task 5), the e2e test (Task 6), and fixtures (Task 7). `HeadlessApp::new(client, workdir, model, w, h)` signature is identical in Task 4, 6, 7. `session_log_to_scenario(&SessionLog) -> ScenarioDefinition` consistent across Task 5 and the example in Task 6. `ScenarioMockClient::from_definition` added in Task 1 is used in Task 6; `from_json` (pre-existing) in Task 7. `render_frame_text(&TuiFrameSnapshot)` consistent across Task 2 and Task 4.

**Known risks (call out before implementing):**
- Task 3/4 depend on `Session::submit_input` + streaming settle semantics not yet verified by compilation. If `submit_input` spawns-and-detaches (returns before the turn ends), `settle()` (yield-loop on `stream_rx.is_some()`) handles it; if it blocks until done, `settle()` is a no-op. Either way it compiles. The assertion in Task 4/6 is the real check — if the assistant text isn't in the captured frame, investigate `submit_input` (`app.rs:3260`) and the stream-drain path.
- `Agent` construction in `HeadlessApp::new` mirrors `test_app()` (`app.rs:4696`) exactly; if `Agent::builder` requires tools/policy for the bash tool to execute in the `tool_turn` scenario, add `.tools(...)` per existing agent setups in `tests/` (search `Agent::builder` in `tests/` for the canonical tool set) — but text-only scenarios need no tools.
