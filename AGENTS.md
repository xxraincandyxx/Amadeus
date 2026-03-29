# AGENTS.md

> Guide for AI coding agents working in the Amadeus codebase.

## Project Overview

**Amadeus** is a Rust SDK for building AI agents with LLM support.
- Multi-provider compatibility (Anthropic Claude, OpenAI GPT)
- Streaming responses, extensible tool system (bash, file ops, web, sub-agents)
- Terminal UI (ratatui) and HTTP API (axum) interfaces
- Multi-agent coordination via Supervisor/Worker pattern (ReAct loop)

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

`tui` (ratatui UI), `api` (axum HTTP server, implies `supervisor`), `concurrency` (lock primitives),
`supervisor` (multi-agent orchestration, implies `concurrency`), `mesh` (distributed, implies `supervisor`),
`context` (context management), `test-utils` (tempfile helpers), `full` (all features).

Chain: `mesh` → `supervisor` → `concurrency`

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

### Agent Behavior Rules
- **No comments in code** unless explicitly requested by the user.
- Do not run destructive shell commands (`sudo`, `rm -rf /`, writing to `.env`/`.pem`/`.key` are blocked).
- Always run `cargo check --features full` and relevant tests after changes.

---

## Key Architecture

### Agent Loop (ReAct Pattern) — `src/agent/loop_agent.rs`
User prompt → LLM call → parse response → if text: emit event | if tool: policy check → execute → add result → loop.

### LLM Client Trait — `src/client/mod.rs`
```rust
pub trait LLMClient: Send + Sync {
    async fn create_message(...) -> Result<(String, Vec<ContentBlock>)>;
    async fn create_message_stream(...) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>>;
}
```
Implemented for Anthropic and OpenAI. `Agent<C>` is generic over provider.

### Tool Trait — `src/tools/tool_trait.rs`
```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn schema(&self) -> &'static Value;
    async fn execute(&self, input: Value) -> Result<String>;
}
```

### Policy System — `src/policy/mod.rs`
Three modes: **Auto** (all automatic), **Ask** (default, dangerous ops require approval), **Strict** (all require approval).

---

## Testing

- **Mock-first**: Use `tests/mock_llm.rs` for deterministic testing. HTTP mocking via `mockito`/`wiremock`.
- **Unit tests**: Inline in `src/` modules (`#[cfg(test)] mod tests`).
- **Integration tests**: In `tests/` directory. Some require specific features (check `Cargo.toml` `[[test]]` `required-features`).
- **Test naming**: Name files by behavior: `tool_approval_test.rs`, `stress_memory_test.rs`.

### Key integration test files
`agent_integration_test.rs`, `e2e_product_flow.rs`, `p2p_test.rs`, `simulation_p2p.rs`, `compaction_test.rs`, `mock_functional_test.rs`, `tool_approval_test.rs`, `streaming_scenarios_test.rs`

---

## Environment & Security

Copy `.env.example` to `.env`. Set `PROVIDER`, API keys (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`), optional base URLs, model ID, `SESSION_LOG_DIR`.
**Never** commit real API keys or modified `.env` files.

---

## Important Files

`src/lib.rs` (public API), `src/main.rs` (entry point), `src/agent/loop_agent.rs` (core loop),
`src/agent/supervisor.rs` (multi-agent), `src/policy/mod.rs` (policy), `src/tools/tool_trait.rs` (tool trait),
`src/error.rs` (error types), `Cargo.toml`, `verify.sh`, `CLAUDE.md`, `docs/ARCHITECTURE.md`

---

## More Documentation

- **CLAUDE.md**: Extended commands, architecture details, session management
- **GEMINI.md**: Performance mandates and defensive engineering guidelines
- **.github/copilot-instructions.md**: Quick reference for GitHub Copilot
- **docs/**: Design notes (REST API, TUI guide, test flow, etc.)
