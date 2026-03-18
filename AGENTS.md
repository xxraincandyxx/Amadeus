# AGENTS.md

> Guide for AI coding agents working in the Amadeus codebase.

## Project Overview

**Amadeus** is a Rust SDK for building AI agents with LLM support. It provides:
- Multi-provider compatibility (Anthropic Claude, OpenAI GPT)
- Streaming responses with real-time event streams
- Extensible tool system (bash, file ops, web, sub-agents)
- Terminal UI (ratatui) and HTTP API (axum) interfaces
- Multi-agent coordination via Supervisor/Worker pattern

The project follows the ReAct (Reason + Act) pattern for agent orchestration.

---

## Quick Reference

| Task | Command |
|------|---------|
| Build (dev) | `cargo build --features full` |
| Build (release) | `cargo build --release --features full` |
| Run TUI | `cargo run --features full` |
| Run HTTP server | `cargo run --features full -- --server [PORT]` |
| Run tests | `cargo test --features full` |
| Run tests with output | `cargo test --features full -- --nocapture` |
| Run specific test | `cargo test --test <name> --features full` |
| Format | `cargo fmt --all` |
| Lint | `cargo clippy --all-features -- -D warnings` |
| Full verification | `./verify.sh` |

**Critical**: Always use `--features full` for development. The crate has no default features.

---

## Feature Flags

| Flag | Description |
|------|-------------|
| `tui` | Terminal UI (ratatui-based) |
| `api` | HTTP server (axum-based), implies `supervisor` |
| `concurrency` | Lock management primitives |
| `supervisor` | Multi-agent orchestration, implies `concurrency` |
| `mesh` | Distributed agent coordination, implies `supervisor` |
| `context` | Context management |
| `test-utils` | Test utilities (tempfile) |
| `full` | All features enabled |

Feature flag chain: `mesh` → `supervisor` → `concurrency`

---

## Project Structure

```
src/
├── agent/           # Agent orchestration
│   ├── loop_agent.rs    # Core Agent loop (ReAct pattern)
│   ├── supervisor.rs    # Multi-agent supervisor
│   ├── worker.rs        # Worker agent implementation
│   ├── compaction.rs    # Context compaction for long conversations
│   ├── config.rs        # Configuration loading
│   ├── events.rs        # Event types (AgentEvent, ToolCall)
│   ├── messages.rs      # Message types (ContentBlock, Message)
│   └── mesh.rs          # Distributed mesh coordination
├── client/          # LLM provider clients
│   ├── mod.rs           # LLMClient trait definition
│   ├── anthropic.rs     # Anthropic API implementation
│   └── openai.rs        # OpenAI API implementation
├── tools/           # Tool system
│   ├── tool_trait.rs    # Tool trait definition
│   ├── registry.rs      # Dynamic tool registry
│   ├── bash.rs          # Shell command execution
│   ├── file.rs          # File operations (read, write, edit)
│   ├── glob.rs          # Pattern-based file matching
│   ├── grep.rs          # Content search
│   ├── web.rs           # Web fetching
│   ├── peer.rs          # Peer-to-peer communication
│   ├── sub_agent.rs     # Recursive sub-agent spawning
│   └── todo.rs          # Task management
├── policy/          # Approval system (Auto/Ask/Strict modes)
├── hooks/           # Extensibility hooks
├── skills/          # Reusable prompt templates
├── mcp/             # Model Context Protocol support
├── ui/              # Terminal UI (ratatui)
├── api/             # HTTP API (axum)
├── benchmark/       # Benchmark and evaluation pipeline
├── concurrency/     # Lock management (feature-gated)
├── core/            # Core primitives (IDs, events)
├── test_utils/      # Test utilities (feature-gated)
├── error.rs         # Error types (thiserror)
├── context.rs       # Project context loading
├── prompts.rs       # System prompts
└── lib.rs           # Public API re-exports

tests/               # Integration tests
├── mock_llm.rs          # Mock LLM client for testing
├── mocks/               # Shared mock utilities
├── scenarios/           # Reusable scenario helpers
├── unit/                # Unit tests
└── *_test.rs            # Integration test files
```

---

## Code Style

### Naming Conventions
- **Files/Modules/Functions**: `snake_case`
- **Types/Traits**: `PascalCase`
- **Constants/Statics**: `SCREAMING_SNAKE_CASE`

### Formatting
- **Indentation**: 2 spaces (Google Rust Style Guide)
- **Imports**: Grouped as `std` → `third-party` → `crate modules`, separated by blank lines
- **Line length**: Follow `cargo fmt` defaults

### Error Handling
- Use `crate::error::Result<T>` (aliased `thiserror`)
- Never use `unwrap()` in production code paths
- `expect()` and `unwrap()` allowed in tests only (enforced by clippy.toml)

### Async Patterns
- Use `tokio` runtime throughout
- Leverage `join_all` for parallel tool execution
- Prefer generic traits over dynamic dispatch for performance-critical code

---

## Key Architecture Patterns

### 1. Agent Loop (ReAct Pattern)
Located in `src/agent/loop_agent.rs`:
1. User prompt → Add to history
2. Call LLM → Parse response
3. If text: emit event, continue
4. If tool call: policy check → execute/deny → add result to history
5. Loop until stop reason

### 2. Generic Client Pattern
```rust
pub struct Agent<C: LLMClient> {
    client: Arc<C>,
    // ...
}
```
The agent is generic over LLM provider, enabling zero-cost provider switching.

### 3. Tool Trait
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn schema(&self) -> &'static Value;
    async fn execute(&self, input: Value) -> Result<String>;
}
```

### 4. Policy System
Three modes in `src/policy/mod.rs`:
- **Auto**: Execute all tools automatically
- **Ask** (default): Dangerous operations require approval
- **Strict**: All tools require approval

Blocked patterns: `sudo`, `chmod 777`, `rm -rf /`, writing to `.env`/`.pem`/`.key`

### 5. Builder Pattern
```rust
let agent = Agent::builder(client, config)
    .with_tools(registry)
    .with_policy(policy)
    .build()?;
```

---

## Testing Strategy

### Mock-First Testing
- Use `tests/mock_llm.rs` for deterministic testing without API calls
- HTTP mocking via `mockito` or `wiremock` (dev-dependencies)

### Test Categories
| Type | Location | Pattern |
|------|----------|---------|
| Unit | Inline in `src/` modules | `#[cfg(test)] mod tests` |
| Integration | `tests/` directory | Separate `[[test]]` targets in Cargo.toml |
| Feature-gated | Both | `#[cfg(feature = "...")]` |

### Running Tests
```bash
# All tests
cargo test --features full

# Specific integration test
cargo test --test agent_integration_test --features full

# With output
cargo test --features full -- --nocapture
```

### Test Naming
Name test files by behavior or subsystem: `tool_approval_test.rs`, `stress_memory_test.rs`

---

## Environment Configuration

Copy `.env.example` to `.env`:

```bash
# Provider selection
PROVIDER=anthropic  # or "openai"

# API Keys
ANTHROPIC_API_KEY=sk-ant-xxx
OPENAI_API_KEY=sk-xxx

# Optional: API proxies
ANTHROPIC_BASE_URL=https://api.anthropic.com
OPENAI_BASE_URL=https://api.openai.com/v1

# Model selection
MODEL_ID=claude-sonnet-4-5-20250929

# Session logging
SESSION_LOG_DIR=logs/sessions
SESSION_LOG_COMPRESS=true
```

**Security**: Never commit real API keys or modified `.env` files.

---

## Commit Guidelines

Recent history uses Conventional Commit style with scopes:
- `feat(ui): add multi-session support`
- `fix(agent): handle stream timeout correctly`
- `style(agent): format imports`
- `docs(arch): init ARCH.md`

Keep commits small and reviewable.

---

## Important Files

| File | Purpose |
|------|---------|
| `src/lib.rs` | Library entry point, public API re-exports |
| `src/main.rs` | Binary entry (TUI and server modes) |
| `src/agent/loop_agent.rs` | Core agent loop implementation |
| `src/agent/supervisor.rs` | Multi-agent supervisor |
| `src/policy/mod.rs` | Approval/policy system |
| `src/tools/tool_trait.rs` | Tool trait definition |
| `src/error.rs` | Error types |
| `Cargo.toml` | Dependencies and feature flags |
| `verify.sh` | CI verification script |
| `CLAUDE.md` | Extended project documentation |
| `ARCH.md` | Architecture diagrams and details |

---

## Common Gotchas

1. **Feature flags are required**: Running `cargo build` without features produces minimal output. Always use `--features full` for development.

2. **Test isolation**: Some tests require specific features. Check `Cargo.toml` for `required-features` on test targets.

3. **Path safety**: File tools validate paths don't escape workspace. Tests should use `tempfile` for isolated paths.

4. **Stream handling**: LLM responses can be streamed or non-streamed. Both code paths must be tested.

5. **Clippy strictness**: CI runs `cargo clippy --all-features -- -D warnings`. All warnings are errors.

---

## Example Prompts for Agents

- "Run the full test suite and summarize any failures"
- "Add a unit test for `src/agent/compaction.rs` covering token threshold behavior"
- "Run `./verify.sh` and fix any issues found"
- "Implement a new tool following the pattern in `src/tools/bash.rs`"
- "Add error handling for the new API endpoint in `src/api/handlers/`"

---

## Where to Find More

- **CLAUDE.md**: Extended commands, architecture details, session management
- **ARCH.md**: Architecture diagrams and data flow
- **GEMINI.md**: Performance mandates and defensive engineering guidelines
- **.github/copilot-instructions.md**: Quick reference for GitHub Copilot
- **docs/**: Design notes (REST API, TUI guide, test flow, etc.)
