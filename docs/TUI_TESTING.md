# Testing the TUI — A Coding-Agent's Guide

> How to write and run automated, deterministic tests for the Amadeus terminal UI **without a real terminal, without real API calls, and without tmux**.
>
> Companion to `docs/TMUX_TEST_FLOW.md` (the *manual* acceptance flow). This doc covers the *automated* path that agents should prefer.

## TL;DR

```bash
# Run every TUI test (headless integration tests + inline unit tests)
cargo test --features full tui

# Run one headless integration test
cargo test --features full --test tui_edge_cases_test -- multi_turn_advances

# See println!/panic output
cargo test --features full --test tui_edge_cases_test -- --nocapture multi_turn
```

You always need `--features full` (or at minimum `test-utils`). The crate has **no default features** — without a feature flag, the TUI tests will not compile.

---

## The mental model (read this first)

The TUI has **two render paths**, and mixing them up is the #1 source of flaky/buggy tests:

| Path | What it shows | How to read it in a test | Method |
|------|---------------|--------------------------|--------|
| **Scrollback** | Committed conversation messages (user + assistant + tool results) | `app.messages_text(width)` | drives `terminal.insert_before` (gemini-cli-style) |
| **Frame buffer** | Chrome: footer, status bar, live viewport, tool monitor, dashboard, streaming buffer | `app.capture().1` (the `String`) | drives ratatui `TestBackend` |

**The scrollback queue is destructive (draining).** Each call to `messages_text(width)` returns only the lines committed *since the previous call*, then clears them. To assert across multiple turns, **accumulate** the returned strings — do not expect earlier turns to still be there on the next call.

```rust
// CORRECT — accumulate across turns
let mut transcript = String::new();
app.submit().await;
transcript.push_str(&app.messages_text(80));
app.submit().await;
transcript.push_str(&app.messages_text(80));
assert!(transcript.contains("first answer"));
assert!(transcript.contains("second answer"));

// WRONG — the second call already drained the first turn
app.submit().await;
let _ = app.messages_text(80);
app.submit().await;
let only_second = app.messages_text(80);
assert!(only_second.contains("first answer")); // FAILS
```

This semantics is documented at `crates/tui/src/ui/headless.rs:74` and `crates/tui/src/ui/app.rs:4843` (`test_drain_unrendered_text`).

---

## The three layers

```
                    ┌─────────────────────────────┐
   your test  ───▶  │  HeadlessApp<C>             │  real App + Session
                    │  (tui crate, test-utils)    │  against TestBackend
                    └──────────────┬──────────────┘
                                   │ injects
                    ┌──────────────▼──────────────┐
                    │  ScenarioMockClient          │  scripted LLM
                    │  (tests/mocks/, full)        │  (Arc<Mutex> queue)
                    └──────────────┬──────────────┘
                                   │ built from
                    ┌──────────────▼──────────────┐
                    │  ScenarioDefinition          │  data types
                    │  (core/test_utils, full)     │  (serde, JSON)
                    └─────────────────────────────┘
```

1. **`ScenarioDefinition`** — pure data: an ordered list of steps, each a batch of `StreamEvent`s (or an error). JSON-serializable so tests can load fixtures from disk.
2. **`ScenarioMockClient`** — implements `LLMClient`. Each `create_message_stream` call pops the next step and replays its events as a real stream. Records every request for assertion.
3. **`HeadlessApp<C>`** — drives the **real** `App`/`Session` into a ratatui `TestBackend`. The only fakes are the injected client and the backend.

The harness does **not** depend on `ScenarioMockClient` — it's generic over `C: LLMClient`. Tests supply the client.

---

## Writing a TUI test — the recipe

### 1. Pull in the mock client

Integration tests in `tests/` share one mock module:

```rust
// at the top of tests/my_tui_test.rs
#[path = "mocks/mod.rs"]
mod mocks;
use mocks::ScenarioMockClient;

use amadeus::ui::headless::HeadlessApp;
use amadeus::test_utils::scenario::{ScenarioDefinition, ScenarioStepDef, StreamEventDef};
// StreamEvent (the non-Def runtime type) is needed for the scripted() helper:
use amadeus::client::StreamEvent;
```

### 2. Build a scenario (three ways)

**A. Hand-roll from steps** — most explicit:
```rust
fn step(events: Vec<StreamEventDef>) -> ScenarioStepDef {
    ScenarioStepDef { delay_ms: None, events, error: None }
}
let def = ScenarioDefinition {
    name: "hello".into(),
    description: "one turn".into(),
    steps: vec![
        step(vec![
            StreamEventDef::TextDelta { text: "Hello!".into() },
            StreamEventDef::StopReason { reason: "end_turn".into() },
        ]),
    ],
};
```

**B. The `scripted()` shortcut** — tersest, uses runtime `StreamEvent`:
```rust
let client = ScenarioMockClient::scripted(vec![
    vec![StreamEvent::TextDelta { text: "Hello!".into() },
         StreamEvent::StopReason("end_turn".into())],
]);
```

**C. Load a JSON fixture** — best for replaying recorded sessions:
```rust
let json = std::fs::read_to_string("tests/tui/scenarios/text_turn.json").unwrap();
let client = ScenarioMockClient::from_json(&json).unwrap();
```

### 3. Drive the app

```rust
#[tokio::test]
async fn my_scenario_renders_assistant_text() {
    let client = ScenarioMockClient::from_definition(def);
    let mut app = HeadlessApp::new(client, ".", "test-model", 80, 24);

    app.type_text("what is 2+2");
    app.submit().await;                 // drains the stream to completion

    // Assert on committed message content (scrollback path):
    let transcript = app.messages_text(80);
    assert!(transcript.contains("Hello!"));

    // Assert on chrome (frame-buffer path):
    let (_snapshot, frame_text) = app.capture();
    assert!(frame_text.contains("test-model"));
}
```

Use a **realistic** terminal size (e.g. `80×24`). Very small heights (≤3 rows) collapse the input box — fine for regression tests, but your assertions about content should use a normal size. See `tiny_viewport_does_not_panic_on_render` (`tests/tui_edge_cases_test.rs:152`) for the floor case.

### 4. Asserting on what the LLM "saw"

`ScenarioMockClient` records every request:
```rust
let client = ScenarioMockClient::from_definition(def);
let clone = client.clone(); // Arc<Mutex> — shareable
let mut app = HeadlessApp::new(client, ".", "m", 80, 24);
app.type_text("hi"); app.submit().await;

let reqs = clone.captured_requests();
assert_eq!(reqs.len(), 1);
assert!(reqs[0].messages.iter().any(|m| m.content_contains("hi")));
```

---

## The slash-command + UI-only path

Slash commands like `/help`, `/viewport`, `/export` are **pure UI** — they must NOT consume a mock LLM step. Write these as inline unit tests in `crates/tui/src/ui/app.rs` using the `test_app()` helper and the `test_*` `pub(crate)` API, not via `HeadlessApp`:

```rust
#[test]
fn slash_viewport_toggles_mode() {
    let mut app = test_app();
    let session = active_session_mut(&mut app);
    let note = session.apply_viewport_command(Some("auto"));
    assert!(note.contains("**hidden** → **auto**"));
}
```

Reference: the `mod tests` block at `crates/tui/src/ui/app.rs:4853` (~50 tests) and `crates/tui/src/ui/headless.rs:147`.

---

## Record → replay: testing against real session behavior

When a bug only reproduces with a real provider run, **record once, replay forever**:

1. Run the binary with session recording on (`session_log_dir` in settings, or the TUI capture path).
2. This produces `session_<ts>_<id>.json` — a `SessionLog`.
3. Convert it to a scenario:
   ```bash
   cargo run --example convert_session --features test-utils -- \
       path/to/session.json > tests/tui/scenarios/my_bug.json
   ```
   (`examples/convert_session.rs`; converter logic in `crates/core/src/test_utils/replay.rs:30`, `session_log_to_scenario`.)
4. Load it in a test via `ScenarioMockClient::from_json`.

Fixture under test: `tests/testflow/fixtures/sample_session.json` (guarded by `on_disk_fixture_round_trips_through_convert_session`).

---

## Feature flags — exactly what you need

| Flag | Unlocks |
|------|---------|
| `test-utils` | `HeadlessApp`, `Session::test_*`, `test_utils::testflow` (record/replay), `frame_text`, `scenario` types |
| `full` | everything (includes `test-utils` + `tui` + the full client stack the mocks need) |

**Always pass `--features full` for TUI integration tests.** The mock client in `tests/mocks/scenario_client.rs` needs the full stack, so `test-utils` alone is not enough for `tests/tui_*.rs`. For inline `app.rs`/`headless.rs` unit tests, `--features test-utils -p tui` also works.

`Cargo.toml` only declares `required-features` for two test binaries (`agent_integration_test`, `e2e_product_flow`); the TUI tests rely on you passing the flag on the CLI. If `amadeus::ui::headless` fails to resolve, you forgot the flag.

---

## Command cheat sheet

```bash
# All TUI tests (integration + inline)
cargo test --features full tui

# Just the two headless integration suites
cargo test --features full --test tui_replay_test
cargo test --features full --test tui_edge_cases_test

# Inline unit tests in the tui crate (app.rs, headless.rs)
cargo test --features full -p tui

# One test, with output
cargo test --features full --test tui_edge_cases_test -- --nocapture multi_turn

# Infrastructure self-tests (no TUI app needed)
cargo test -p core --features test-utils scenario
cargo test -p core --features test-utils replay
cargo test -p core --features test-utils frame_text
cargo test -p core --features test-utils testflow

# Record→replay converter CLI
cargo run --example convert_session --features test-utils -- path/to/session.json
```

---

## What to copy from

| File | What it demonstrates |
|------|----------------------|
| `tests/tui_edge_cases_test.rs` | Multi-turn accumulate pattern, error-step recovery, tiny viewport, unicode, slash `/help`, mid-stream capture |
| `tests/tui_replay_test.rs` | End-to-end happy path + JSON fixture loading |
| `crates/tui/src/ui/app.rs` (`mod tests`, line ~4853) | ~50 inline tests: tool monitor, session switching, key chords, slash commands, rewind/export, render chrome |
| `crates/tui/src/ui/headless.rs:147` | `HeadlessApp` self-tests |
| `tests/mocks/scenario_client.rs:213` | Mock client self-tests |

---

## Anti-patterns to avoid

- **Don't use `tests/tui/` (the legacy snapshot harness).** `tests/tui/harness.rs` is explicitly deprecated — it captures blank frames and does not drive the real `App`. Use `HeadlessApp` instead.
- **Don't assert message content via `capture()`.** Committed messages live in scrollback, not the frame buffer. Use `messages_text()`.
- **Don't call `messages_text()` twice expecting old content.** It drains.
- **Don't forget `--features full`.** The crate has no default features.
- **Don't expect slash/UI-only commands to consume a mock step.** They're pure UI; a spurious mock-pop will misalign your scenario for the *next* turn.

---

## Quick reference: the `test_*` API on `Session`

All `pub(crate)`, gated `#[cfg(feature = "test-utils")]`, in `crates/tui/src/ui/app.rs`:

| Method | Line | Purpose |
|--------|------|---------|
| `App::test_session_mut()` | `:4780` | borrow the active `Session<C>` |
| `Session::test_type_char(c)` | `:4788` | inject one input char |
| `Session::test_submit()` | `:4793` | run `submit_input` |
| `Session::test_render(frame)` | `:4798` | render into a frame |
| `Session::test_pump_turn()` | `:4808` | drain `stream_rx` to completion (headless `run_loop`) |
| `Session::test_drain_unrendered_text(w)` | `:4843` | drain the scrollback queue (one-shot) |

`HeadlessApp` (`crates/tui/src/ui/headless.rs`) wraps these: `new` `:44`, `type_text` `:58`, `submit` `:66`, `messages_text` `:78`, `capture` `:85`.
