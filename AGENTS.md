# AGENTS.md - AI Agent Development Guide

## Project Overview

Amadeus is an **Agent SDK** - a Rust library providing core building blocks for building AI agents. It is NOT a platform or application.

## Core Principles

1. **SDK, not Platform** - We provide agent capabilities, not session/memory/platform features
2. **Minimal dependencies** - Only what's needed for LLM interaction and tool execution
3. **Type-safe** - Strong typing with Result-based error handling
4. **Streaming-first** - Real-time response streaming as a first-class feature

## Build/Lint/Test Commands

```bash
# Build
cargo build                    # Debug build
cargo build --release          # Release build
cargo build --features full    # Build with all optional features (api, tui)

# Lint
cargo clippy                   # Run linter (fix all warnings)
cargo clippy --fix             # Auto-fix lint warnings

# Format
cargo fmt                      # Format code (run before commits)

# Test
cargo test                     # Run all tests
cargo test --test bash_test    # Run specific integration test file
cargo test test_bash_echo      # Run specific test function
cargo test --test agent_test   # Run agent tests
cargo test -- --nocapture      # Run tests with stdout visible

# Run examples
cargo run --example tui --features tui           # TUI test harness
cargo run --example server --features api        # HTTP API server
ANTHROPIC_API_KEY=xxx cargo run --example tui --features tui

# Python bindings (optional)
cd bindings/python && maturin develop --release
```

## Code Style Guidelines

### Imports

Group imports in this order, separated by blank lines:
1. Standard library (`std::...`)
2. External crates (`use serde_json::...`)
3. Internal modules (`use crate::...`)

```rust
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;

use crate::error::{AgentError, Result};
use crate::tools::tool_trait::Tool;
```

### Formatting

- Use `cargo fmt` before committing
- Max line length: 100 characters (default rustfmt)
- Indent with 4 spaces
- Match arms: align with the match expression

### Types and Naming

- **Structs/Enums**: PascalCase (`AgentError`, `StreamEvent`)
- **Functions/Methods**: snake_case (`create_message`, `execute_with_timeout`)
- **Constants**: SCREAMING_SNAKE_CASE (`API_VERSION`, `DEFAULT_BASE_URL`)
- **Module names**: snake_case (`tool_trait`, `loop_agent`)
- **Type parameters**: Single uppercase letter or short name (`Agent<C>` where `C: LLMClient`)

### Error Handling

- Use `Result<T>` from `crate::error` (alias for `std::result::Result<T, AgentError>`)
- Use `?` operator to propagate errors
- Use `thiserror` for custom error types (see `src/error.rs`)
- Convert external errors using `#[from]` attribute:

```rust
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("API request failed: {0}")]
    ApiRequest(#[from] reqwest::Error),
}
```

### Async Patterns

- Use `#[async_trait]` for traits with async methods
- Use `tokio` as the async runtime
- Return `Pin<Box<dyn Stream<Item = Result<T>> + Send>>` for streaming APIs
- Use `Arc<RwLock<T>>` for shared mutable state

### Traits

- All trait implementations require `Send + Sync` for thread safety
- Use `&'static str` for tool names and schemas (compile-time known)

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn schema(&self) -> &'static Value;
    async fn execute(&self, input: Value) -> Result<String>;
}
```

### Testing

- Place integration tests in `tests/` directory
- Use `#[tokio::test]` for async tests
- Use `wiremock` or `mockito` for mocking HTTP responses
- Create helper functions for common test setup:

```rust
fn create_test_config() -> Config {
    Config {
        provider: Provider::Anthropic,
        api_key: "test-key".to_string(),
        // ...
    }
}
```

## Architecture

```
src/
├── lib.rs              # Public SDK exports
├── error.rs            # Error types (AgentError, Result)
├── agent/
│   ├── mod.rs          # Re-exports
│   ├── config.rs       # Config, Provider
│   ├── loop_agent.rs   # Agent<C> struct
│   ├── messages.rs     # Message, ContentBlock
│   └── events.rs       # AgentEvent, RunResult, ToolCall
├── client/
│   ├── mod.rs          # LLMClient trait, StreamEvent
│   ├── anthropic.rs    # AnthropicClient
│   └── openai.rs       # OpenAIClient
├── tools/
│   ├── mod.rs          # Re-exports
│   ├── tool_trait.rs   # Tool trait
│   ├── bash.rs         # BashTool
│   ├── file.rs         # ReadFileTool, WriteFileTool, EditFileTool
│   ├── registry.rs     # ToolRegistry
│   └── schema.rs       # JSON schemas for tools
├── core/               # Primitives (IDs, events)
├── api/                # HTTP API (optional, feature = "api")
└── ui/                 # Terminal UI (optional, feature = "tui")
```

## SDK Scope

### Belongs in SDK

- LLM clients (Anthropic, OpenAI, Gemini, etc.)
- Tool implementations (bash, file, web search, etc.)
- Agent loop logic
- Streaming infrastructure
- Error handling

### Does NOT Belong in SDK

- Session persistence
- Memory management
- HTTP server (except for testing)
- Platform adapters (Discord, Slack)
- User authentication
- Database connections

## Commit Guidelines

Use conventional commits:
```
feat(agent): add support for tool result streaming
fix(client): handle timeout errors correctly
docs(readme): update installation instructions
refactor(tools): simplify bash tool implementation
test(agent): add tests for multi-turn conversations
```

## Key Files

| File | Purpose |
|------|---------|
| `src/lib.rs` | SDK entry point, public exports |
| `src/error.rs` | All error types |
| `src/agent/loop_agent.rs` | Core Agent struct and run loop |
| `src/client/mod.rs` | LLMClient trait definition |
| `src/tools/tool_trait.rs` | Tool trait definition |
| `src/tools/registry.rs` | Tool registration and execution |

---
*Last updated: 2026-02-24*
