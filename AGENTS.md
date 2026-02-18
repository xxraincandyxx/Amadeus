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
cargo run -- --server                # Start HTTP server on port 3000
cargo run -- --server 8080           # Start HTTP server on custom port
PROVIDER=openai cargo run            # Use OpenAI
USE_STREAMING=true cargo run         # Enable streaming
```

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `PROVIDER` | No | `anthropic` | LLM provider (`anthropic` or `openai`) |
| `ANTHROPIC_API_KEY` | Yes* | - | Anthropic API key |
| `ANTHROPIC_BASE_URL` | No | - | Custom Anthropic endpoint |
| `OPENAI_API_KEY` | Yes* | - | OpenAI API key |
| `OPENAI_BASE_URL` | No | - | Custom OpenAI endpoint |
| `MODEL_ID` | No | Provider default | Model identifier |
| `USE_STREAMING` | No | `false` | Enable streaming responses |
| `MAX_OUTPUT_BYTES` | No | `50000` | Max tool output size |
| `BLOCKED_COMMANDS` | No | `rm -rf /` | Comma-separated blocked commands |

*Required based on selected provider. Configure via `.env` (copy from `.env.example`).

## Code Style Guidelines

### Imports
Group imports in order: std → external crates → crate modules, separated by blank lines:
```rust
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;

use crate::error::{AgentError, Result};
use crate::tools::tool_trait::Tool;
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
  tools/           # Tool implementations (bash, file, schema, tool_trait)
  ui/              # Terminal UI (app, colors, event, components)
    app.rs         # Main TUI application state machine
    event.rs       # Keyboard/mouse event handling
    colors.rs      # Dracula theme and Palette
    components/    # UI widgets (input, messages, sidebar, status, tools)
  api/             # HTTP server (handlers, types, http)
tests/             # Integration tests (bash_test, messages_test, etc.)
```

### Types
- Use `Result<T>` from `crate::error` (not `std::result::Result`)
- Prefer `String` over `&str` for struct fields (owned data)
- Use `Arc<T>` for shared ownership, `RwLock<T>` for shared mutable state
- Use `PathBuf` for file paths (not `String`)
- Use `Option<T>` for nullable values (no null/None in Rust)

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
    ToolUse { id: String, name: String, input: Value },
}

#[serde(default)]  // For optional fields with defaults
pub replace_all: bool,
```

### Testing
```rust
#[tokio::test]
async fn test_bash_echo() {
    let tool = BashTool::new(30, "/tmp".to_string(), vec![], 50_000);
    let input = json!({"command": "echo hello"});
    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("hello"));
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
- Use `//!` for module-level docs
- Use `///` for item-level docs
- Include examples in doc comments with ` ```rust,ignore `

### Concurrency
- `Arc::clone(&shared)` increments reference count (cheap)
- `RwLock::read().await` for read, `RwLock::write().await` for write
- Always drop locks explicitly or use scoped blocks to avoid deadlocks
- Use `futures::future::join_all` for concurrent execution

### Tool Implementation Pattern
```rust
#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &'static str { "bash" }
    fn schema(&self) -> &'static Value { bash_tool() }
    async fn execute(&self, input: Value) -> Result<String> {
        let parsed: BashInput = serde_json::from_value(input)
            .map_err(|e| AgentError::Json(e.to_string()))?;
        // ... execution logic
    }
}
```

## Key Design Principles
- **Type safety**: `Result<T>` error handling everywhere
- **Async-first**: Tokio runtime, non-blocking I/O
- **Provider abstraction**: Trait-based LLM client switching
- **Tool abstraction**: `Tool` trait with name, schema, execute
- **Path safety**: File tools validate paths stay within workspace
- **Streaming support**: Real-time output via SSE
- **Shared history**: `Arc<RwLock<Vec<Message>>>` for conversation state

## TUI Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Submit message |
| `Ctrl+Enter` | Insert newline |
| `↑` / `↓` | Navigate history |
| `PgUp` / `PgDn` | Scroll messages |
| `Ctrl+B` / `⌘B` | Toggle file tree sidebar |
| `Alt+B` / `⌥B` | Toggle help sidebar |
| `Esc` | Collapse tool panels / close sidebar |
| `q` / `Ctrl+D` | Exit |

## UI Components

The TUI uses ratatui with a Dracula-inspired color theme:
- **Status bar**: Shows processing state, timing, token count, model name
- **Messages area**: Scrollable conversation history with markdown rendering
- **Tool panels**: Collapsible output panels for bash/file operations
- **Input area**: Multiline textarea with command history
- **Sidebars**: Toggleable file tree (Ctrl+B) and help panel (Alt+B)
