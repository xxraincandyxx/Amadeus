# AGENTS.md

> Guide for AI coding agents working in the Amadeus codebase.

## Project Overview

**Amadeus** is a Rust SDK for building AI agents with LLM support.
- Multi-provider compatibility (Anthropic Claude, OpenAI GPT)
- Streaming responses, extensible tool system (bash, file ops, web, sub-agents)
- Terminal UI (ratatui) and HTTP API (axum) interfaces
- Multi-agent coordination via orchestra-based routing and delegation

---

## Quick Reference

| Task                    | Command                                          |
|-------------------------|--------------------------------------------------|
| Build (dev)             | `cargo build --features full`                    |
| Build (release)         | `cargo build --release --features full`          |
| Run TUI                 | `cargo run --features full`                      |
| Run HTTP server         | `cargo run --features full -- --server [PORT]`   |
| Run tests               | `cargo test --features full`                     |
| Run tests with output   | `cargo test --features full -- --nocapture`      |
| Run specific test       | `cargo test --test <name> --features full`       |
| Run single unit test    | `cargo test <test_fn_name> --features full`      |
| Format                  | `cargo fmt --all`                                |
| Lint                    | `cargo clippy --all-features -- -D warnings`     |
| Full verification       | `./verify.sh`                                    |
| Single crate            | `cargo test -p core` / `cargo check -p tui`      |
| Source-file headers     | `python scripts/check_source_headers.py`         |
| Clean generated output   | `make clean`                                     |
| Offline benchmark runner | `cargo run --bin benchmark --features full -- --fixtures <dir> --output <dir>` |

**Critical**: Always use `--features full` for development. The crate has no default features.

### Web / desktop client (`apps/web`)

Separate npm project (not a Cargo workspace member; `apps/web/src-tauri` has its own manifest).

| Task | Command |
|------|---------|
| Dev server | `cd apps/web && npm run dev` (needs `cargo run --features full -- --server 3000`) |
| Tests | `npm run test` (`node --test src/*.test.js`) |
| Lint | `npm run lint` |
| Mock API (no Rust server) | `npm run mock-api` |
| Desktop shell | `npm run desktop:dev` / `npm run desktop:build` |

Override the API address with `VITE_AMADEUS_API_URL`. See `apps/web/README.md`.

---

## Feature Flags

`api` (axum HTTP adapter, implies `orchestra`), `tui` (ratatui UI, implies `concurrency`),
`concurrency` (lock primitives), `orchestra` (canonical multi-agent orchestration surface, implies `concurrency`),
`context` (context management),
`test-utils` (test helpers and recording), `full` (all features).

Chains: `api` → `orchestra` → `concurrency`, `tui` → `concurrency`

---

## Code Style

### Naming & Formatting
- **Files/Modules/Functions**: `snake_case` | **Types/Traits**: `PascalCase` | **Constants**: `SCREAMING_SNAKE_CASE`
- **Indentation**: 4 spaces (`rustfmt` default). There is no `rustfmt.toml` and none should be added — the defaults are the style.
- **Imports**: Group as `std` → `third-party` → `crate modules`, separated by blank lines.
- **File size**: split modules past ~600 lines into a directory with `mod.rs`.
- Long-form rules live in `CODING_STYLE.md` (authoritative on *form*; this file stays authoritative on *behavior*).

### Error Handling
- Use `crate::error::Result<T>` (defined via `thiserror` in `crates/core/src/error.rs`).
- **Never** use `unwrap()` in production code. `unwrap()`/`expect()` allowed in tests only (`clippy.toml` enforces this).

### Async & Performance
- Use `tokio` runtime throughout. Use `join_all` for parallel tool execution.
- Prefer generic traits (`Agent<C: LLMClient>`) over dynamic dispatch for performance.
- Minimize heap allocations; use `Arc<T>` for shared ownership, `RwLock<T>` for interior mutability.

### Documentation
- Every public function needs a doc comment. Private functions doc comments encouraged.
- Document Args, Returns, and any non-obvious complexity.
- Source-file headers for in-scope code are mandatory and must follow `docs/SOURCE_FILE_HEADERS.md`.
- When touching an in-scope source file, treat header maintenance as required work in the same change.

### Agent Behavior Rules
- **No comments in code** unless explicitly requested by the user.
- Do not run destructive shell commands (`sudo`, `rm -rf /`, writing to `.env`/`.pem`/`.key` are blocked).
- Always run `cargo check --features full` and relevant tests after changes.
- Keep source-file headers accurate; stale headers are policy violations.

---

## Key Architecture

### Agent Loop (ReAct Pattern) — `crates/core/src/agent/loop_agent.rs`
User prompt → LLM call → parse response → if text: emit event | if tool: policy check → execute → add result → loop.

### LLM Client Trait — `crates/core/src/client/mod.rs`
```rust
pub trait LLMClient: Send + Sync {
    async fn create_message(...) -> Result<(String, Vec<ContentBlock>)>;
    async fn create_message_stream(...) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>>;
}
```
Implemented for Anthropic and OpenAI. `Agent<C>` is generic over provider.

### Tool Trait — `crates/core/src/tools/tool_trait.rs`
```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn schema(&self) -> &'static Value;
    async fn execute(&self, input: Value) -> Result<String>;
}
```

### Policy System — `crates/core/src/policy/mod.rs`
An **opt-in** secondary approval layer with three `ApprovalMode`s: **Auto** (all automatic), **Ask** (dangerous ops require approval), **Strict** (all require approval). It is only consulted when an agent is built with `with_policy(...)` (e.g. the benchmark runner); the agent loop and `/execute` do not wire it in by default. Real gating in every default path comes from `PermissionEnforcer`/`PermissionMode` (`read-only` | `workspace-write` | `danger-full-access` | `prompt`). See `docs/plans/2026-08-05-structure-improvement-plan.md` F2c.

### Localization — `crates/tui/src/ui/i18n.rs`, `apps/web/src/i18n.js`
Two independent catalogs, both English (`en`, default) + Simplified Chinese (`zh-CN`); English is the fallback.
- TUI: `i18n::text("key")`, plus `command_summary` / `thought_label` / `thought_summary`. Locale is process-wide (`i18n::set_language`). Migration is partial — when touching a component that still holds literal strings, route them through the catalog.
- Web: `translate(language, key, variables)` with `{named}` placeholders, exposed as `t(...)` via React context.
- Selection: `tui.language` in any settings layer, or `/language en|zh-CN` (`/lang`) at runtime.

### Slash commands — `crates/commands/src/lib.rs`
A new command must be added in three places: `SLASH_COMMAND_SPECS`, the `SlashCommand` enum, and `parse`/`name`. The web palette (`apps/web/src/slashCommands.js`) intentionally excludes TUI-only commands.

---

## Testing

- **Mock-first**: Use `tests/mock_llm.rs` for deterministic testing. HTTP mocking via `mockito`/`wiremock`.
- **Unit tests**: Inline in `src/` modules (`#[cfg(test)] mod tests`).
- **Integration tests**: In `tests/` directory. Feature gating is mixed: some suites use `Cargo.toml` `[[test]]` `required-features`, some use `cfg(feature = "...")`, and many assume `--features full`.
- **Test naming**: Name files by behavior: `tool_approval_test.rs`, `stress_memory_test.rs`.

- **TUI tests**: drive the real `App` headlessly with `HeadlessApp` (`crates/tui/src/ui/headless.rs`, feature `test-utils`). Committed transcript lines are printed via `insert_before` and are **not** in the frame buffer — assert them via `messages_text()`, and use `capture()` only for chrome/streaming/dialogs.
- **Web tests**: `cd apps/web && npm run test` (`node --test`, colocated `src/*.test.js`).

### Key integration test files
`agent_integration_test.rs`, `e2e_product_flow.rs`, `p2p_test.rs`, `simulation_p2p.rs`, `compaction_test.rs`, `mock_functional_test.rs`, `tool_approval_test.rs`, `streaming_scenarios_test.rs`

---

## Environment & Security

Copy `.env.example` to `.env`. Set `PROVIDER`, API keys (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`), optional base URLs, model ID, `SESSION_LOG_DIR`.
**Never** commit real API keys or modified `.env` files.

---

## Important Files

`src/lib.rs` (compatibility facade), `src/main.rs` (entry point + CLI flags), `crates/core/src/agent/loop_agent.rs` (core loop),
`crates/core/src/agent/orchestra.rs` (multi-agent orchestration), `crates/core/src/policy/mod.rs` (policy),
`crates/core/src/tools/tool_trait.rs` (tool trait), `crates/core/src/error.rs` (error types),
`crates/commands/src/lib.rs` (slash commands), `crates/config/src/lib.rs` (layered settings),
`apps/web/src/App.jsx` (web client), `src/bin/benchmark.rs` (benchmark CLI),
`Cargo.toml`, `verify.sh`, `Makefile`, `CODING_STYLE.md`, `CLAUDE.md`, `docs/ARCHITECTURE.md`

Only `crates/*` plus the root facade are workspace members; `apps/web`, `python-sdk`, `benchmarks/`, and `runtime/` sit outside the Cargo build.

---

## More Documentation

- **CLAUDE.md**: Extended commands, architecture details, session management
- **CODING_STYLE.md**: Long-form style guide (formatting, headers, error handling, feature gating)
- **GEMINI.md**: Performance mandates and defensive engineering guidelines
- **.github/copilot-instructions.md**: Quick reference for GitHub Copilot
- **docs/SOURCE_FILE_HEADERS.md**: Canonical schema and strict maintenance rules for source-file headers
- **apps/web/README.md**: Web/desktop client behavior and client-side slash commands
- **benchmarks/README.md**, **runtime/README.md**: External Python eval harnesses
- **docs/**: Design notes (`ARCHITECTURE.md`, `HTTP_API.md`, `TOOLS.md`, `COMPACTION.md`, `TUI_TESTING.md`, `WEB_DESIGN_SYSTEM.md`, `MACOS_APP.md`)

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
