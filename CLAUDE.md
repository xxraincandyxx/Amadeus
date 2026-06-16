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

> `supervisor` and `team` are legacy feature aliases that still compile but resolve to `orchestra`. Prefer `orchestra`. There is **no** `mesh` feature.

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

### TUI Capture

When `test-utils` is enabled and session recording is on, the TUI writes rendered frame snapshots to `tui_capture.log` in the recording directory. The log is JSONL and includes visible cell content plus styling metadata, which makes it useful for visual regression debugging.

### Linting & Checking

```bash
# Check without building
cargo check --features full

# Format code
cargo fmt

# Run clippy
cargo clippy --features full

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

Legacy aliases (kept for compatibility, prefer the canonical name): `team` and `supervisor` both enable `orchestra`. There is no `mesh` feature despite what older docs may suggest.

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
└── docs/
```

**Mental model:** `CLI or library call → config + provider selection → crates/core runtime → TUI or HTTP adapter`.

The `core` crate (`amadeus_core`) is where the flat `amadeus::*` module namespace is assembled. When a path below says `core/src/...`, it means `crates/core/src/...`. The leaf crates (e.g. `messages`, `events`, `permissions`) hold reusable building blocks; `core` depends on them and re-exports the public surface.

### Agent Loop (ReAct Pattern)

The heart of the SDK is `core/src/agent/loop_agent.rs`. It implements the ReAct pattern:

1. **Turn-based execution**: each interaction is a "turn" with text response and tool calls
2. **Internal history**: the `Agent` struct manages its own `Arc<RwLock<Vec<Message>>>` history
3. **Streaming**: supports real-time event streaming via `run_stream()`
4. **Approval flow**: tools requiring approval use channels for UI communication

### Multi-Agent System (Orchestra)

Orchestration lives in `core/src/agent/orchestra.rs` and `core/src/agent/worker.rs`, with the reusable data models and selection logic in the `runtime` crate (`runtime/src/orchestra.rs`, `worker.rs`, `team.rs`, `scheduler.rs`). Note: `team.rs` and `supervisor.rs` still exist but are legacy; new work should target the `orchestra` types.

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

Two cooperating layers:

- **Policy** (`core/src/policy/mod.rs`) — approval gating with three modes: **Auto** (execute all), **Ask** (default; ask only for dangerous ops), **Strict** (ask for all). Dangerous patterns are auto-blocked: `sudo`, `chmod 777`, `rm -rf /`, writes to `.env`/`.pem`/`.key`, shell pipes to `bash`/`sh`.
- **Permissions** (`core/src/permissions.rs` + `crates/permissions`) — the `PermissionMode` enum (`read-only` | `workspace-write` | `danger-full-access` | `prompt`), enforced by `PermissionEnforcer` and selectable at the CLI with `--permission-mode`.

### Other notable surfaces

- `assessment` (`core/src/assessment/`) — read-only feature assessment runner (`--assess-features`).
- `benchmark` (`core/src/benchmark/`) — case/eval/metrics/runner for offline LLM benchmarking (see `src/bin/benchmark.rs`).
- `rag` (`crates/rag/`) — retrieval-augmented generation: chunker, embedding, vector store, tool.
- `mcp` (`core/src/mcp/`) — Model Context Protocol client + tool adapter.
- `transcript`, `audit`, `bridge`, `security` — supporting modules in `core/src/`.

## Testing Strategy

Amadeus prioritizes **Mock-First Testing** to ensure stability without API costs.

- **Unit tests** live alongside the code in each crate's `src/`.
- **Integration tests** are in `tests/` (e.g. `p2p_test.rs`, `simulation_p2p.rs`, `e2e_product_flow.rs`, `agent_integration_test.rs`, `compaction_test.rs`, `tool_approval_test.rs`, `monitoring_harness_test.rs`, `tui_replay_test.rs`, plus the `tests/mocks/` and `tests/scenarios/` harnesses). Note: `tests/tui/harness.rs` is a deprecated non-functional stub — use `HeadlessApp` instead (see below).
- **Mock utilities**: `mockito` / `wiremock` for HTTP, `tests/mock_llm.rs` for a mock LLM client, `tests/mocks/scenario_client.rs` (`ScenarioMockClient`) for scripted scenario-driven captures, `tests/scenarios/timeline.rs` for timestamped event timelines.

### TUI Testing (feature `test-utils`)

Drive the **real** `App` headlessly against a ratatui `TestBackend` via `amadeus::ui::headless::HeadlessApp<C: LLMClient>` (in `crates/tui/src/ui/headless.rs`). Build it with any `LLMClient` — typically `ScenarioMockClient::from_json(...)` from a fixture in `tests/tui/scenarios/`. API: `type_text`, `submit().await` (runs a full agent turn headlessly), `capture() -> (TuiFrameSnapshot, String)` (real rendered frame + `render_frame_text` text), and `messages_text(width)` (committed conversation).

```rust
let client = ScenarioMockClient::from_json(&std::fs::read_to_string("tests/tui/scenarios/text_turn.json")?)?;
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

- **Indentation**: 4 spaces (default `rustfmt`; there is no `rustfmt.toml`)
- **Naming**: `snake_case` for variables/functions, `PascalCase` for types
- **Error handling**: use `crate::error::Result` and avoid `unwrap()`
- **Async/await**: Tokio runtime throughout
- **Documentation**: rustdoc comments on public APIs
- **File headers (required):** hand-maintained source files (`src/**/*.rs`, `tests/**/*.rs`, `examples/**/*.rs`, `scripts/**/*.sh`) **must** start with an `@amadeus-header` … `@end-amadeus-header` block with the fields defined in `docs/SOURCE_FILE_HEADERS.md` (`summary`, `layer`, `status`, `feature_flags`, `provides`, `uses`, `invariants`, `side_effects`, `tests`). Match the existing headers when adding a file; validate with `python scripts/check_source_headers.py`.

## Configuration

Create `.amadeus/settings.json` in the workspace, or `~/.amadeus/settings.json` for global defaults:

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
  "blocked_commands": ["rm -rf /", "sudo"]
}
```

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
- `crates/core/src/agent/orchestra.rs` — multi-agent orchestration (canonical; `team.rs`/`supervisor.rs` are legacy)
- `crates/runtime/src/` — reusable orchestration models + selectors
- `crates/core/src/client/` — LLM provider clients (`anthropic.rs`, `openai.rs`, trait in `mod.rs`)
- `crates/core/src/tools/` — tool system + registry
- `crates/core/src/policy/mod.rs` — approval/policy system
- `crates/core/src/permissions.rs` — permission modes + enforcer
- `crates/core/src/agent/compaction.rs` — context compaction
- `crates/api/`, `crates/tui/` — HTTP and terminal adapters
- `crates/tui/src/ui/headless.rs` — `HeadlessApp` headless TUI test driver (feature `test-utils`)
- `crates/core/src/test_utils/` — `scenario.rs` (scenario types), `replay.rs` (`session_log_to_scenario`), `frame_text.rs` (`render_frame_text`), `testflow/` (`SessionRecorder`, frame snapshots)
- `tests/tui/scenarios/` — replayable scenario JSON fixtures; `examples/convert_session.rs` — record→scenario CLI
- `tests/` — integration tests directory
- `Cargo.toml` — workspace definition, features, and the root facade package
- `docs/ARCHITECTURE.md` — authoritative architecture deep-dive
- `docs/SOURCE_FILE_HEADERS.md` — mandatory file-header schema

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **amadeus** (42534 symbols, 114089 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## When Debugging

1. `gitnexus_query({query: "<error or symptom>"})` — find execution flows related to the issue
2. `gitnexus_context({name: "<suspect function>"})` — see all callers, callees, and process participation
3. `READ gitnexus://repo/amadeus/process/{processName}` — trace the full execution flow step by step
4. For regressions: `gitnexus_detect_changes({scope: "compare", base_ref: "main"})` — see what your branch changed

## When Refactoring

- **Renaming**: MUST use `gitnexus_rename({symbol_name: "old", new_name: "new", dry_run: true})` first. Review the preview — graph edits are safe, text_search edits need manual review. Then run with `dry_run: false`.
- **Extracting/Splitting**: MUST run `gitnexus_context({name: "target"})` to see all incoming/outgoing refs, then `gitnexus_impact({target: "target", direction: "upstream"})` to find all external callers before moving code.
- After any refactor: run `gitnexus_detect_changes({scope: "all"})` to verify only expected files changed.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Tools Quick Reference

| Tool | When to use | Command |
|------|-------------|---------|
| `query` | Find code by concept | `gitnexus_query({query: "auth validation"})` |
| `context` | 360-degree view of one symbol | `gitnexus_context({name: "validateUser"})` |
| `impact` | Blast radius before editing | `gitnexus_impact({target: "X", direction: "upstream"})` |
| `detect_changes` | Pre-commit scope check | `gitnexus_detect_changes({scope: "staged"})` |
| `rename` | Safe multi-file rename | `gitnexus_rename({symbol_name: "old", new_name: "new", dry_run: true})` |
| `cypher` | Custom graph queries | `gitnexus_cypher({query: "MATCH ..."})` |

## Impact Risk Levels

| Depth | Meaning | Action |
|-------|---------|--------|
| d=1 | WILL BREAK — direct callers/importers | MUST update these |
| d=2 | LIKELY AFFECTED — indirect deps | Should test |
| d=3 | MAY NEED TESTING — transitive | Test if critical path |

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/amadeus/context` | Codebase overview, check index freshness |
| `gitnexus://repo/amadeus/clusters` | All functional areas |
| `gitnexus://repo/amadeus/processes` | All execution flows |
| `gitnexus://repo/amadeus/process/{name}` | Step-by-step execution trace |

## Self-Check Before Finishing

Before completing any code modification task, verify:
1. `gitnexus_impact` was run for all modified symbols
2. No HIGH/CRITICAL risk warnings were ignored
3. `gitnexus_detect_changes()` confirms changes match expected scope
4. All d=1 (WILL BREAK) dependents were updated

## Keeping the Index Fresh

After committing code changes, the GitNexus index becomes stale. Re-run analyze to update it:

```bash
npx gitnexus analyze
```

If the index previously included embeddings, preserve them by adding `--embeddings`:

```bash
npx gitnexus analyze --embeddings
```

To check whether embeddings exist, inspect `.gitnexus/meta.json` — the `stats.embeddings` field shows the count (0 means no embeddings). **Running analyze without `--embeddings` will delete any previously generated embeddings.**

> Claude Code users: A PostToolUse hook handles this automatically after `git commit` and `git merge`.

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
