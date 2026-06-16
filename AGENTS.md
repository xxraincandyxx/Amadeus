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

**Critical**: Always use `--features full` for development. The crate has no default features.

---

## Feature Flags

`api` (axum HTTP adapter, implies `orchestra`), `tui` (ratatui UI, implies `concurrency`),
`concurrency` (lock primitives), `orchestra` (canonical multi-agent orchestration surface, implies `concurrency`),
`team`/`supervisor` (legacy aliases for `orchestra`), `context` (context management),
`test-utils` (test helpers and recording), `full` (all features).

Chains: `api` → `orchestra` → `concurrency`, `tui` → `concurrency`, `team`/`supervisor` → `orchestra`

---

## Code Style

### Naming & Formatting
- **Files/Modules/Functions**: `snake_case` | **Types/Traits**: `PascalCase` | **Constants**: `SCREAMING_SNAKE_CASE`
- **Indentation**: 2 spaces (Google Rust Style Guide). No `rustfmt.toml` — rely on `cargo fmt` defaults.
- **Imports**: Group as `std` → `third-party` → `crate modules`, separated by blank lines.

### Error Handling
- Use `crate::error::Result<T>` (defined via `thiserror` in `src/error.rs`).
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
Three modes: **Auto** (all automatic), **Ask** (default, dangerous ops require approval), **Strict** (all require approval).

---

## Testing

- **Mock-first**: Use `tests/mock_llm.rs` for deterministic testing. HTTP mocking via `mockito`/`wiremock`.
- **Unit tests**: Inline in `src/` modules (`#[cfg(test)] mod tests`).
- **Integration tests**: In `tests/` directory. Feature gating is mixed: some suites use `Cargo.toml` `[[test]]` `required-features`, some use `cfg(feature = "...")`, and many assume `--features full`.
- **Test naming**: Name files by behavior: `tool_approval_test.rs`, `stress_memory_test.rs`.

### Key integration test files
`agent_integration_test.rs`, `e2e_product_flow.rs`, `p2p_test.rs`, `simulation_p2p.rs`, `compaction_test.rs`, `mock_functional_test.rs`, `tool_approval_test.rs`, `streaming_scenarios_test.rs`

---

## Environment & Security

Copy `.env.example` to `.env`. Set `PROVIDER`, API keys (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`), optional base URLs, model ID, `SESSION_LOG_DIR`.
**Never** commit real API keys or modified `.env` files.

---

## Important Files

`src/lib.rs` (compatibility facade), `src/main.rs` (entry point), `crates/core/src/agent/loop_agent.rs` (core loop),
`crates/core/src/agent/orchestra.rs` (multi-agent orchestration), `crates/core/src/policy/mod.rs` (policy),
`crates/core/src/tools/tool_trait.rs` (tool trait), `crates/core/src/error.rs` (error types),
`Cargo.toml`, `verify.sh`, `CLAUDE.md`, `docs/ARCHITECTURE.md`

---

## More Documentation

- **CLAUDE.md**: Extended commands, architecture details, session management
- **GEMINI.md**: Performance mandates and defensive engineering guidelines
- **.github/copilot-instructions.md**: Quick reference for GitHub Copilot
- **docs/SOURCE_FILE_HEADERS.md**: Canonical schema and strict maintenance rules for source-file headers
- **docs/**: Design notes (REST API, TUI guide, test flow, etc.)

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
