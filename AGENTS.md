# AGENTS.md

Rust-based AI coding agent supporting Anthropic and OpenAI APIs with a terminal UI and HTTP server mode.

## Build / Lint / Test

```bash
cargo build                          # Debug build
cargo build --release                # Release build
cargo check                          # Fast check without building
cargo clippy                         # Lint
cargo fmt                            # Format code

cargo test                           # Run all tests
cargo test test_bash_echo            # Run specific test function
cargo test --test bash_test          # Run specific test file
cargo test test_bash                 # Run tests matching pattern
cargo test -- --nocapture            # Show test output

cargo run                            # Interactive mode (Anthropic)
cargo run -- "your prompt"           # Single-shot mode
cargo run -- --server                # HTTP server on port 3000
PROVIDER=openai cargo run            # Use OpenAI
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PROVIDER` | `anthropic` | LLM provider (`anthropic` or `openai`) |
| `ANTHROPIC_API_KEY` | - | Anthropic API key (required if provider=anthropic) |
| `OPENAI_API_KEY` | - | OpenAI API key (required if provider=openai) |
| `MODEL_ID` | Provider default | Model identifier |
| `MAX_OUTPUT_BYTES` | `50000` | Max tool output size |
| `BLOCKED_COMMANDS` | `rm -rf /` | Comma-separated blocked commands |

Configure via `.env` (copy from `.env.example`).

## Code Style

### Code Comments
- **DO NOT ADD COMMENTS** unless explicitly requested by the user

### Imports
Group in order: std → external crates → crate modules, separated by blank lines:
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

### Types
- Use `Result<T>` from `crate::error` (not `std::result::Result`)
- Prefer `String` over `&str` for struct fields
- Use `Arc<T>` for shared ownership, `RwLock<T>` for shared mutable state
- Use `PathBuf` for file paths (not `String`)
- Use `Option<T>` for nullable values

### Error Handling
```rust
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("API request failed: {0}")]
    ApiRequest(#[from] reqwest::Error),
    #[error("Command timed out after {0}s")]
    Timeout(u64),
    #[error("Tool input validation failed for '{tool}': {reason}")]
    ToolInput { tool: String, reason: String },
    #[error("Path escapes workspace: {0}")]
    PathEscape(PathBuf),
}
```

### Async Patterns
```rust
use tokio::time::{timeout, Duration};

match timeout(Duration::from_secs(30), operation).await {
    Ok(result) => result,
    Err(_) => Err(AgentError::Timeout(30)),
}

let mut history = history.write().await;
history.push(Message::user("prompt"));
drop(history);
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

#[serde(default)]
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

assert!(matches!(result.unwrap_err(), AgentError::Timeout(_)));
```

### Tool Implementation
```rust
#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &'static str { "bash" }
    fn schema(&self) -> &'static Value { bash_tool() }
    async fn execute(&self, input: Value) -> Result<String> {
        let parsed: BashInput = serde_json::from_value(input)
            .map_err(|e| AgentError::ToolInput {
                tool: "bash".to_string(),
                reason: e.to_string(),
            })?;
        // execution logic
    }
}
```

## File Organization

```
src/
  lib.rs           # Public exports, module declarations
  main.rs          # CLI entry point
  error.rs         # Custom error types (AgentError, Result)
  core/            # Core primitives (workspace, state, event, id, branch, commit)
  agent/           # Agent system (agent, config, messages, events, supervisor, pipeline)
  client/          # LLM client trait + anthropic/openai impls
  tools/           # Tool implementations (bash, file, registry, schema)
  concurrency/     # Lock and transaction management
  storage/         # File-based persistence
  api/             # HTTP server (handlers, types)
  ui/              # Terminal UI (app, colors, components)
tests/             # Integration tests
```

## Key Design Principles

- **Type safety**: `Result<T>` error handling everywhere
- **Async-first**: Tokio runtime, non-blocking I/O
- **Provider abstraction**: Trait-based LLM client switching
- **Tool abstraction**: `Tool` trait with name, schema, execute
- **Path safety**: File tools validate paths stay within workspace
- **Shared state**: `Arc<RwLock<T>>` for thread-safe shared mutable state

## TUI Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Submit message |
| `Ctrl+Enter` | Insert newline |
| `↑` / `↓` | Navigate history |
| `PgUp` / `PgDn` | Scroll messages |
| `Ctrl+B` | Toggle file tree sidebar |
| `Alt+B` | Toggle help sidebar |
| `Esc` | Collapse tool panels / close sidebar |
| `q` / `Ctrl+D` | Exit |
