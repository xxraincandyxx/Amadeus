# AGENTS.md

Rust-based AI coding agent supporting Anthropic and OpenAI APIs. This document guides agentic coding agents working in this codebase.

## Build / Lint / Test Commands

```bash
cargo build                          # Debug build
cargo build --release                # Release build
cargo check                          # Fast check without building
cargo clippy                         # Lint with clippy

cargo test                           # Run all tests
cargo test -- --nocapture            # Show test output
cargo test test_bash_echo            # Run specific test function
cargo test --test bash_test          # Run specific test file
cargo test test_bash                 # Run tests matching pattern

cargo run                            # Interactive mode (Anthropic)
cargo run -- "list files in src"     # Single-shot mode
PROVIDER=openai cargo run            # Use OpenAI
USE_STREAMING=true cargo run         # Enable streaming
```

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `PROVIDER` | No | `anthropic` | LLM provider (`anthropic` or `openai`) |
| `ANTHROPIC_API_KEY` | Yes* | - | Anthropic API key |
| `OPENAI_API_KEY` | Yes* | - | OpenAI API key |
| `MODEL_ID` | No | Provider default | Model identifier |
| `USE_STREAMING` | No | `false` | Enable streaming responses |

*Required based on selected provider. Configure via `.env` (copy from `.env.example`).

## Code Style Guidelines

### Imports
Group imports in order: std → external crates → crate modules, separated by blank lines:
```rust
use std::sync::Arc;
use std::path::PathBuf;

use tokio::sync::RwLock;
use anyhow::Result;
use async_trait::async_trait;

use crate::error::{AgentError, Result};
use crate::agent::messages::{ContentBlock, Message};
use crate::client::LLMClient;
```

### Naming Conventions
| Type | Convention | Example |
|------|------------|---------|
| Functions/Variables | `snake_case` | `execute_tool`, `api_key` |
| Types/Structs/Enums | `PascalCase` | `AgentError`, `Config` |
| Constants | `SCREAMING_SNAKE_CASE` | `API_VERSION` |
| Test functions | `test_<subject>_<scenario>` | `test_bash_timeout` |

### File Organization
```
src/
  lib.rs           # Public exports, module declarations
  main.rs          # CLI entry point only
  error.rs         # All custom error types
  agent/           # Agent domain (config, messages, loop_agent)
  client/          # LLM client domain (trait + anthropic/openai impls)
  tools/           # Tool implementations (bash, schema)
  ui/              # Terminal UI (colors, repl)
tests/             # Integration tests (bash_test, messages_test, etc.)
```

### Types
- Use `Result<T>` from `crate::error` (not `std::result::Result`)
- Prefer `String` over `&str` for struct fields (owned data)
- Use `Arc<T>` for shared ownership, `RwLock<T>` for shared mutable state
- Use `PathBuf` for file paths (not `String`)

### Error Handling
Use `thiserror` with `#[from]` for automatic conversion:
```rust
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("API request failed: {0}")]
    Api(#[from] reqwest::Error),
    #[error("Command timed out after {0}s")]
    Timeout(u64),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

### Async Patterns
```rust
// Timeout handling
match timeout(duration, operation).await {
    Ok(result) => result,
    Err(_) => Err(AgentError::Timeout(secs)),
}

// RwLock: drop locks explicitly to avoid deadlocks
let mut history = history.write().await;
history.push(Message::user("prompt"));
drop(history);
```

### Traits and Generics
```rust
#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn create_message(...) -> Result<(String, Vec<ContentBlock>)>;
}

pub struct Agent<C: LLMClient> { client: C, ... }
```

### Serde Patterns
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String, input: ToolInput },
}
```

### Testing
```rust
#[tokio::test]
async fn test_bash_echo() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let result = tool.execute(&ToolInput { command: "echo hello".into() }).await;
    assert!(result.is_ok());
}

// Use matches! for enum variant checking
assert!(matches!(result.unwrap_err(), AgentError::Timeout(_)));
```

### Struct Initialization
Use field init shorthand when variable name matches field name:
```rust
let timeout_secs = 30;
let workdir = "/tmp".to_string();
Self {
    timeout_secs,
    workdir,
}
```

### Documentation
Use `//!` for module-level docs and `///` for item-level docs.

### Concurrency
- `Arc::clone(&shared)` increments reference count (cheap)
- `RwLock::read().await` for read, `RwLock::write().await` for write
- Always drop locks explicitly or use scoped blocks to avoid deadlocks
- Use `futures::future::join_all` for concurrent execution

## Key Design Principles
- **Type safety**: `Result<T>` error handling everywhere
- **Async-first**: Tokio runtime, non-blocking I/O
- **Provider abstraction**: Trait-based LLM client switching
- **Single tool**: Bash tool handles all file/terminal operations
- **Streaming support**: Real-time output via SSE
- **Shared history**: `Arc<RwLock<Vec<Message>>>` for conversation state
