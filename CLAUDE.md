# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Amadeus is a Rust SDK for building AI agents with LLM support, featuring multi-provider compatibility (Anthropic, OpenAI), streaming responses, and a powerful tool system. It follows a "Bash is All You Need" philosophy and uses the ReAct (Reason + Act) pattern for agent orchestration on the Tokio async runtime.

> **Workspace layout.** Amadeus is a **Cargo workspace**, not a single crate. The root `amadeus` crate is a thin compatibility facade that re-exports `amadeus_core` and conditionally the `api`/`tui` adapters. Almost all implementation lives under `crates/`. Library callers keep importing through `amadeus::...`; the implementation keeps moving into workspace crates underneath. See `docs/ARCHITECTURE.md` for the authoritative deep-dive.

> **Synced instruction files.** `AGENTS.md`, `GEMINI.md`, and this file describe the same codebase for different tools and are meant to stay in sync. When you update one, consider whether the others need the same change.

## Common Commands

### Building

```bash
# Build with all features (recommended for development)
cargo build --features full

# Build release
cargo build --release --features full

# Build with specific features only
cargo build --features tui        # Terminal UI only
cargo build --features api         # HTTP API only
cargo build --features orchestra   # Multi-agent orchestration (canonical feature)

# Build/test a single workspace crate (much faster during iteration)
cargo build -p core
cargo check -p runtime
cargo test  -p core
```

> `orchestra` is the only multi-agent feature flag. The removed `team`, `supervisor`, and `mesh` feature names are not supported.

### Running

```bash
# Run TUI (Terminal UI)
cargo run --features full

# Run HTTP API server (default port 3000)
cargo run --features full -- --server
cargo run --features full -- --server 8080

# Other CLI flags
cargo run --features full -- --record [DIR]                 # record session to JSON log
cargo run --features full -- --export PATH                  # export conversation to .md/.json on exit
cargo run --features full -- --permission-mode MODE         # read-only|workspace-write|danger-full-access|prompt
cargo run --features full -- --assess-features [DIR]        # read-only feature assessment + report
cargo run --features full -- --llm-trace [DIR]              # log full LLM request/response payloads per turn

# Run example programs
cargo run --example tui --features tui
cargo run --example server --features api

# Run via installed launcher (if ~/bin/amadeus symlink is set up)
amadeus
amadeus --server
```

### Testing

```bash
# Run all tests (including simulations)
cargo test --features full

# Run a single test by name
cargo test test_name --features full

# Run integration tests only
cargo test --test p2p_test --features full
cargo test --test simulation_p2p --features full
cargo test --test e2e_product_flow --features full   # requires the `orchestra` feature

# Show test output
cargo test --features full -- --nocapture
```

Some integration targets declare `required-features` in the root `Cargo.toml` (e.g. `agent_integration_test` needs `test-utils`, `e2e_product_flow` needs `orchestra`). `--features full` satisfies all of them, which is why it is the default convention for new suites.

### Full Verification Gate

```bash
./verify.sh
```

`verify.sh` is the canonical pre-PR gate and runs, in order: `scripts/check_source_headers.py`, `cargo fmt --all -- --check`, `cargo metadata`, `cargo clippy --all-features -- -D warnings`, a four-way feature matrix check (`--no-default-features`, `tui`, `api`, `full`), then `cargo test --features full`. Warnings are errors — fix them in the change that introduces them.

### Web & Desktop Client

The React/Tauri client lives in `apps/web` and talks to the versioned session HTTP API (`cargo run --features full -- --server 3000`).

```bash
cd apps/web
npm install
npm run dev              # Vite dev server on http://127.0.0.1:5173
npm run test             # node --test src/*.test.js (no test framework dependency)
npm run lint             # eslint
npm run mock-api         # mock-server.mjs — drive the UI without a Rust server
npm run desktop:dev      # Tauri desktop shell (builds the sidecar first)
npm run desktop:build
```

`VITE_AMADEUS_API_URL` overrides the default API address (`http://127.0.0.1:3000`). See `apps/web/README.md` for the client slash-command catalog and `docs/WEB_DESIGN_SYSTEM.md` for visual conventions.

### Benchmarks & Experiments

```bash
# In-repo offline benchmark runner (fixtures → run artifacts)
cargo run --bin benchmark --features full -- --fixtures <dir> --output <dir> [--suite NAME] [--case ID] [--mode mock|live]
```

External harnesses are Python and live outside the Cargo workspace: `benchmarks/` (intercode/MBPP via Docker, `fs_bench`, `run_amadeus.py`) and `runtime/` (`locomo` conversational-memory eval, `rag_eval`). Each has its own README. `python-sdk/` holds the Python client package.

### Housekeeping

```bash
make clean            # build output + generated logs/results + local caches
make clean-build      # cargo clean (root + src-tauri), web dist, tauri gen/binaries
make clean-generated  # logs, benchmark_runs, benchmarks/results, locomo/rag_eval results
make clean-cache      # __pycache__, .pytest_cache
```

`make clean` never touches dependency installs (`node_modules`) or user configuration.

### TUI Capture

When `test-utils` is enabled and session recording is on, the TUI writes rendered frame snapshots to `tui_capture.log` in the recording directory. The log is JSONL and includes visible cell content plus styling metadata, which makes it useful for visual regression debugging.

### Linting & Checking

```bash
# Check without building
cargo check --features full

# Format code
cargo fmt

# Run clippy the way CI does (warnings are errors)
cargo clippy --all-features -- -D warnings

# Validate @amadeus-header blocks on source files (see Code Style)
python scripts/check_source_headers.py
```

## Feature Flags

Amadeus is highly modular. Canonical features (defined in the root `Cargo.toml`, mirrored in `crates/core`):

- `tui` — Terminal UI (ratatui-based); implies `concurrency`
- `api` — Axum HTTP server; implies `orchestra`
- `concurrency` — Concurrency primitives (locks, file locking, coordination)
- `orchestra` — Multi-agent orchestration system; implies `concurrency`
- `context` — Context management
- `test-utils` — Test utilities (session recording, fixtures, assertions)
- `full` — All of the above

The removed `team`, `supervisor`, and `mesh` feature names are not supported; use `orchestra`.

## Workspace Architecture

### Workspace Shape

```text
amadeus/
├── src/
│   ├── lib.rs          # thin facade: re-exports amadeus_core (+ api/tui when enabled)
│   └── main.rs         # CLI mode switch (TUI / server / record / assess)
├── crates/
│   ├── core/           # THE aggregator: agent loop, client, tools, policy, permissions,
│   │                   #   assessment, benchmark, mcp, security, audit, bridge, transcript,
│   │                   #   hooks, skills — and re-exports the leaf crates below
│   ├── runtime/        # reusable orchestration data models + selectors
│   │                   #   (agent, orchestra, team, worker, scheduler)
│   ├── api/            # Axum router + handlers
│   ├── tui/            # ratatui application + components
│   ├── config/         # layered settings loading
│   ├── commands/       # slash-command parsing + context/citation helpers
│   ├── skills/         # skill registry and loading
│   ├── prompts/        # prompt builder + sections
│   ├── profiles/       # prompt/tool profile composition
│   ├── permissions/    # permission modes + rules
│   ├── messages/       # message / content-block types
│   ├── events/         # event + tool-call payloads
│   ├── ids/            # ID generation
│   ├── telemetry/      # telemetry sinks + events
│   ├── hooks/          # extensibility hooks
│   ├── compaction/     # context compaction trigger logic
│   ├── context/        # context/memory stores (session, file, json)
│   └── rag/            # retrieval: vector store, embedding, chunker
├── tests/              # integration suites + shared harnesses
├── examples/           # adapter bootstraps
├── apps/web/           # React + Tauri client (own npm project, not in the Cargo workspace)
├── python-sdk/         # Python client package
├── benchmarks/         # external Python harnesses (intercode/MBPP, fs_bench)
├── runtime/            # live experiments + evals (locomo memory bench, rag_eval)
├── scripts/            # header validation, launchers, benchmark drivers
├── skills/             # skill definitions loaded by crates/skills
└── docs/
```

Only `crates/*` (plus the root facade) are Cargo workspace members. `apps/web/src-tauri` is a **separate** Cargo project with its own manifest — `cargo` commands at the root do not build it.

**Mental model:** `CLI or library call → config + provider selection → crates/core runtime → TUI or HTTP adapter`.

The `core` crate (`amadeus_core`) is where the flat `amadeus::*` module namespace is assembled. When a path below says `core/src/...`, it means `crates/core/src/...`. The leaf crates (e.g. `messages`, `events`, `permissions`) hold reusable building blocks; `core` depends on them and re-exports the public surface.

### Agent Loop (ReAct Pattern)

The heart of the SDK is `core/src/agent/loop_agent.rs`. It implements the ReAct pattern:

1. **Turn-based execution**: each interaction is a "turn" with text response and tool calls
2. **Internal history**: the `Agent` struct manages its own `Arc<RwLock<Vec<Message>>>` history
3. **Streaming**: supports real-time event streaming via `run_stream()`
4. **Approval flow**: tools requiring approval use channels for UI communication

### Multi-Agent System (Orchestra)

Orchestration lives in `crates/core/src/agent/orchestra.rs` and `crates/core/src/agent/worker.rs`, with reusable data models and selection logic in the runtime crate (`crates/runtime/src/orchestra.rs`, `worker.rs`, `team.rs`, and `scheduler.rs`). The runtime `team.rs` file remains an internal data-model implementation; the public surface uses orchestra terminology.

- **Orchestra**: manages a pool of specialized worker agents
- **Concurrency**: uses `tokio::task::JoinSet` for parallel task execution
- **Queueing**: task queue with backpressure (`max_pending_tasks`)
- **P2P collaboration**: routes `HelpRequest` events between workers via a central bus

### Context Compaction

When conversations grow long, `core/src/agent/compaction.rs` (with `crates/compaction`) provides automatic compaction: monitors token usage, triggers summarization when approaching context limits (default 75% threshold), preserves recent messages, and uses the LLM to generate summaries.

### LLM Client Trait

Provider-agnostic abstraction in `core/src/client/mod.rs`, implemented for Anthropic (`client/anthropic.rs`) and OpenAI (`client/openai.rs`). `Agent<C>` is generic over the provider, enabling zero-cost provider switching.

### Tool System

Tools implement the `Tool` trait from `core/src/tools/tool_trait.rs`:

```rust
pub trait Tool: Send + Sync {
  fn name(&self) -> &'static str;
  fn schema(&self) -> &'static Value;
  async fn execute(&self, input: Value) -> Result<String>;
}
```

Built-in tools live in `core/src/tools/` and are organized into composable **profiles/packs** (`ToolPack`, `ToolProfile`, `ToolPolicy`, `ToolSpec`, `ToolCatalogView`) registered via the `ToolRegistry`. Built-ins include `bash`, `read_file`/`write_file`/`edit_file`, `glob`, `grep`, `web`, `peer`, plus `todo`, `memory`, `sub_agent`, and `platform`.

### Policy & Permissions

Two layers, with distinct roles:

- **Permissions** (`core/src/permissions.rs` + `crates/permissions`) — the **default and always-on** gate. The `PermissionMode` enum (`read-only` | `workspace-write` | `danger-full-access` | `prompt`) is enforced by `PermissionEnforcer`, selected at the CLI with `--permission-mode`, and is what gates `/execute` and the agent loop in every default path.
- **Policy** (`core/src/policy/mod.rs`) — an **opt-in** secondary layer with three `ApprovalMode`s: `Auto` (execute all), `Ask` (ask only for dangerous ops), `Strict` (ask for all). It only applies when a caller explicitly builds the agent with `with_policy(...)` (e.g. the benchmark runner). The agent loop and `/execute` do **not** wire it in by default, and `Policy::from_config` is currently a stub (see `docs/plans/2026-08-05-structure-improvement-plan.md` F2c). Dangerous patterns it knows (`sudo`, `chmod 777`, `rm -rf /`, writes to `.env`/`.pem`/`.key`, shell pipes to `bash`/`sh`) therefore only fire when it is opted into.

### Other notable surfaces

- `assessment` (`core/src/assessment/`) — read-only feature assessment runner (`--assess-features`).
- `benchmark` (`core/src/benchmark/`) — case/eval/metrics/runner for offline LLM benchmarking (see `src/bin/benchmark.rs`).
- `rag` (`crates/rag/`) — retrieval-augmented generation: chunker, embedding, vector store, tool.
- `mcp` (`core/src/mcp/`) — Model Context Protocol client + tool adapter.
- `transcript`, `audit`, `bridge`, `security` — supporting modules in `core/src/`.

### Localization (TUI + web)

Both UIs ship English (`en`, default) and Simplified Chinese (`zh-CN`) and there are two independent catalogs — adding a string to one does **not** cover the other:

- **TUI** — `crates/tui/src/ui/i18n.rs`. Look up user-facing text through `i18n::text("key")` (plus `command_summary`, `thought_label`, `thought_summary`); locale is process-wide state set via `i18n::set_language`. English is the fallback for untranslated keys. Not every component is migrated yet — when you touch a component that still holds literal strings, route them through the catalog.
- **Web** — `apps/web/src/i18n.js` (`translate(language, key, variables)` with ICU-like `{named}` placeholders). `App.jsx` exposes it as `t(...)` through a React context; English source strings are themselves the keys.

Selection: `tui.language` in any settings layer (user-wide `~/.amadeus/settings.json` is the intended home), or `/language en` | `/language zh-CN` (`/lang` alias) at runtime. The command is declared in `crates/commands/src/lib.rs` (`SLASH_COMMAND_SPECS` + `SlashCommand::Language`) — new slash commands must be added in all three places: the spec table, the enum, and `parse`/`name`.

## Testing Strategy

Amadeus prioritizes **Mock-First Testing** to ensure stability without API costs.

- **Unit tests** live alongside the code in each crate's `src/`.
- **Integration tests** are in `tests/` (e.g. `p2p_test.rs`, `simulation_p2p.rs`, `e2e_product_flow.rs`, `agent_integration_test.rs`, `compaction_test.rs`, `tool_approval_test.rs`, `monitoring_harness_test.rs`, and `tui_replay_test.rs`, plus the `tests/mocks/` and `tests/scenarios/` harnesses).
- **Mock utilities**: `mockito` / `wiremock` for HTTP, `tests/mock_llm.rs` for a mock LLM client, `tests/mocks/scenario_client.rs` (`ScenarioMockClient`) for scripted scenario-driven captures, `tests/scenarios/timeline.rs` for timestamped event timelines.

### TUI Testing (feature `test-utils`)

Drive the **real** `App` headlessly against a ratatui `TestBackend` via `amadeus::ui::headless::HeadlessApp<C: LLMClient>` (in `crates/tui/src/ui/headless.rs`). Build it with any `LLMClient` — typically `ScenarioMockClient::from_json(...)` from a fixture in `tests/fixtures/scenarios/`. API: `type_text`, `submit().await` (runs a full agent turn headlessly), `capture() -> (TuiFrameSnapshot, String)` (real rendered frame + `render_frame_text` text), and `messages_text(width)` (committed conversation).

```rust
let client = ScenarioMockClient::from_json(&std::fs::read_to_string("tests/fixtures/scenarios/basic_query.json")?)?;
let mut app = HeadlessApp::new(client, ".", "model", 80, 24); // use a realistic size so layout fits
app.type_text("hi");
app.submit().await;
assert!(app.messages_text(80).contains("answer"));
```

**Render model gotcha (important):** Amadeus uses gemini-cli-style *inline scrolling*. Committed conversation messages are printed via `Terminal::insert_before` (terminal scrollback) — **they do not appear in the live frame buffer**. The frame (`capture()`) only shows chrome (header/footer/status), the dashboard, in-progress streaming text, tool-activity panels, and dialogs. So:
- Assert **transcript content** (assistant answers, tool results) via `messages_text()` / agent history — **not** the frame.
- Assert **frame content** (layout, streaming, tool panels, slash dialogs, dashboard) via `capture()`.

Replay real sessions: record with `--record`, then convert a captured `session_*.json` into a scenario with `cargo run --example convert_session --features test-utils -- path/to/session_*.json` (uses `amadeus::test_utils::replay::session_log_to_scenario`).

## Code Style

`CODING_STYLE.md` is the long-form house style; the load-bearing rules:

- **Indentation**: 4 spaces (default `rustfmt`; there is no `rustfmt.toml` — do not add one)
- **Naming**: `snake_case` for variables/functions, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for consts; `#[serde(rename_all = "snake_case")]` on serialized enums
- **Error handling**: return `crate::error::Result` (backed by `AgentError`/`thiserror`, foreign errors converted with `#[from]`). **No `unwrap()`/`expect()` outside tests** — `clippy.toml` allows them only under `#[cfg(test)]`
- **Async/await**: Tokio runtime throughout; `Arc<RwLock<T>>` for read-heavy shared state, `tokio::sync` locks when held across `.await`
- **Generics over `dyn`**: prefer `Agent<C: LLMClient>` monomorphization on hot paths
- **Documentation**: rustdoc (`///`) on every public item, `//!` module docs directly under the file header
- **No inline comments** unless the user asks for them; rationale belongs in doc comments or the commit message. Never leave commented-out code
- **File size**: modules past ~600 lines are split into a subdirectory
- **File headers (required):** hand-maintained source files (`src/**/*.rs`, `tests/**/*.rs`, `examples/**/*.rs`, `scripts/**/*.sh`) **must** start with an `@amadeus-header` … `@end-amadeus-header` block with the fields defined in `docs/SOURCE_FILE_HEADERS.md` (`summary`, `layer`, `status`, `feature_flags`, `provides`, `uses`, `invariants`, `side_effects`, `tests`). Match the existing headers when adding a file; validate with `python scripts/check_source_headers.py`.

## Configuration

Settings are layered, later layers overriding earlier ones: `~/.amadeus/settings.json` → `.amadeus/settings.json` → `.amadeus/settings.local.json`. Loading lives in `crates/config`; see `.amadeus/README.md` for the authoritative key list and `.amadeus/settings.example.json` for a full example.

```json
{
  "provider": "anthropic",
  "api_key": "sk-ant-xxx",
  "base_url": "https://api.anthropic.com",
  "model": "claude-sonnet-4-5-20250929",
  "timeout_seconds": 120,
  "max_output_bytes": 50000,
  "session_log_dir": "./logs",
  "session_log_compress": true,
  "blocked_commands": ["rm -rf /", "sudo"],
  "tui": {
    "language": "en",
    "live_viewport": { "mode": "hidden", "height_percent": 32 }
  }
}
```

User-wide preferences (`tui.language`, `tui.live_viewport`) belong in `~/.amadeus/settings.json`; provider/model/workspace runtime settings belong in the project file.

## Session Management

Sessions are automatically logged with full conversation history:

```rust
// Sessions are saved automatically after each run
let result = agent.run("My prompt").await?;

// List saved sessions
let sessions = agent.list_sessions()?;

// Restore a previous session
let session = Agent::load_session(&sessions[0].0)?;
agent.restore_session(&session).await;
```

Session files are stored in JSON or compressed JSON.gz format in the configured `session_log_dir`.

### Multi-Session Types

The TUI supports two session types:

1. **Independent Sessions** - Created via `/new-agent` command. Each has a fresh agent with empty history. Ideal for parallel, unrelated tasks.
2. **Sub-Agent Sessions** - Created by the orchestra/supervisor for delegated tasks. Organized hierarchically with parent-child relationships.

## Key Design Patterns

1. **Actor-like Workers**: Workers are spawned as persistent configurations and managed by the Orchestra
2. **Generic Clients**: The `Agent<C>` struct is generic over the LLM provider, allowing zero-cost provider switching
3. **Reactive UI**: The TUI consumes an `AgentEvent` stream, decoupling logic from presentation
4. **Builder Pattern**: Use `Agent::builder()` for custom configuration with tools, policy, hooks, etc.
5. **Stream-based**: All major operations support streaming events for real-time monitoring

## Important File Paths

- `src/lib.rs` — thin compatibility facade (re-exports `amadeus_core` + adapters)
- `src/main.rs` — CLI entry point / mode switch (TUI, server, record, assess, export)
- `crates/core/src/agent/loop_agent.rs` — core agent loop (ReAct)
- `crates/core/src/agent/orchestra.rs` — canonical multi-agent orchestration
- `crates/runtime/src/` — reusable orchestration models + selectors
- `crates/core/src/client/` — LLM provider clients (`anthropic.rs`, `openai.rs`, trait in `mod.rs`)
- `crates/core/src/tools/` — tool system + registry
- `crates/core/src/policy/mod.rs` — approval/policy system
- `crates/core/src/permissions.rs` — permission modes + enforcer
- `crates/core/src/agent/compaction.rs` — context compaction
- `crates/api/`, `crates/tui/` — HTTP and terminal adapters
- `crates/tui/src/ui/headless.rs` — `HeadlessApp` headless TUI test driver (feature `test-utils`)
- `crates/core/src/test_utils/` — `scenario.rs` (scenario types), `replay.rs` (`session_log_to_scenario`), `frame_text.rs` (`render_frame_text`), `testflow/` (`SessionRecorder`, frame snapshots)
- `tests/fixtures/scenarios/` — replayable scenario JSON fixtures; `examples/convert_session.rs` — record→scenario CLI
- `tests/` — integration tests directory
- `crates/commands/src/lib.rs` — slash-command specs + parsing (shared by TUI and web palette)
- `crates/tui/src/ui/i18n.rs`, `apps/web/src/i18n.js` — the two translation catalogs
- `apps/web/src/` — `App.jsx`, `api.js`, `sessionState.js`, `slashCommands.js`, `MarkdownContent.jsx` (+ colocated `*.test.js`)
- `src/bin/benchmark.rs` — offline benchmark CLI (`cargo run --bin benchmark`)
- `Cargo.toml` — workspace definition, features, and the root facade package
- `verify.sh` — full verification pipeline; `Makefile` — clean targets
- `CODING_STYLE.md` — long-form style guide (form); `AGENTS.md` — behavior rules
- `docs/ARCHITECTURE.md` — authoritative architecture deep-dive
- `docs/SOURCE_FILE_HEADERS.md` — mandatory file-header schema
- Other docs worth knowing: `docs/TOOLS.md`, `docs/HTTP_API.md`, `docs/API_GUIDE.md`, `docs/COMPACTION.md`, `docs/TUI_TESTING.md`, `docs/WEB_DESIGN_SYSTEM.md`, `docs/MACOS_APP.md`

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **Amadeus** (7921 symbols, 19099 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> Index stale? Run `node .gitnexus/run.cjs analyze` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? `npx gitnexus analyze` (npm 11 crash → `npm i -g gitnexus`; #1939).

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows. For regression review, compare against the default branch: `detect_changes({scope: "compare", base_ref: "master"})`.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `query({search_query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `context({name: "symbolName"})`.
- For security review, `explain({target: "fileOrSymbol"})` lists taint findings (source→sink flows; needs `analyze --pdg`).

## Never Do

- NEVER edit a function, class, or method without first running `impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit changes without running `detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/Amadeus/context` | Codebase overview, check index freshness |
| `gitnexus://repo/Amadeus/clusters` | All functional areas |
| `gitnexus://repo/Amadeus/processes` | All execution flows |
| `gitnexus://repo/Amadeus/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
