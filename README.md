# Amadeus - AI Agent SDK

A Rust SDK for building AI agents with LLM support, featuring multi-provider compatibility, streaming responses, and a powerful tool system.

## Overview

Amadeus is a production-ready AI agent framework that provides:

- **Multi-Provider Support**: Works with Anthropic (Claude) and OpenAI APIs
- **Streaming Responses**: Real-time event streaming with async/await
- **Tool System**: Extensible tool registry with bash, file operations, glob, grep, and web fetch
- **Policy-Based Safety**: Configurable approval system for dangerous operations
- **Terminal UI**: Beautiful ratatui-based TUI for interactive use
- **HTTP API**: Optional REST API server for integration
- **Session Management**: Automatic logging and session restoration
- **TUI Capture**: Optional frame snapshots for visual debugging in session recordings
- **Multi-Agent Coordination**: Supervisor/worker pattern for complex tasks

Parity progress against the `refs/claw-code-parity` reference is tracked in `docs/PARITY.md` and only advanced when covered by automated tests.

## Installation

### Prerequisites

- Rust 1.70 or later
- API key from Anthropic or OpenAI

### Setup

```bash
# Clone the repository
git clone https://github.com/xxraincandyxx/Amadeus.git
cd Amadeus

# Copy environment template
cp .env.example .env

# Add your API key to .env
# ANTHROPIC_API_KEY=sk-ant-xxx  (for Anthropic)
# or
# OPENAI_API_KEY=sk-xxx  (for OpenAI)

# Build the project
cargo build --release
```

## Usage

### Terminal UI Mode (Default)

```bash
# Run with TUI (requires 'tui' feature)
cargo run --features full

# Or with just TUI support
cargo run --features tui
```

### HTTP Server Mode

```bash
# Start HTTP API server on default port 3000
cargo run --features full -- --server

# Custom port
cargo run --features full -- --server 8080
```

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
amadeus = { git = "https://github.com/xxraincandyxx/Amadeus" }
tokio = { version = "1.39", features = ["full"] }
```

Basic usage:

```rust
use amadeus::{
    Agent, Config, Provider,
    AnthropicClient, OpenAIClient,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = Arc::new(Config::load()?);
    
    // Create LLM client
    let client = match config.provider {
        Provider::Anthropic => AnthropicClient::new(
            config.api_key.clone(),
            config.base_url.clone(),
            config.model.clone(),
        ).into(),
        Provider::OpenAI => OpenAIClient::new(
            config.api_key.clone(),
            config.base_url.clone(),
            config.model.clone(),
        ).into(),
    };
    
    // Build agent with default tools
    let agent = Agent::new(client, config);
    
    // Run a prompt
    let result = agent.run("Create a hello world program in Rust").await?;
    println!("{}", result.text);
    
    Ok(())
}
```

### Custom Tools

```rust
use amadeus::{Agent, Tool};
use async_trait::async_trait;
use serde_json::Value;

struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &'static str {
        "my_tool"
    }
    
    fn schema(&self) -> &'static Value {
        &serde_json::json!({
            "name": "my_tool",
            "description": "A custom tool",
            "input_schema": {
                "type": "object",
                "properties": {
                    "input": { "type": "string" }
                },
                "required": ["input"]
            }
        })
    }
    
    async fn execute(&self, input: Value) -> Result<String, amadeus::AgentError> {
        // Your tool logic here
        Ok(format!("Processed: {:?}", input))
    }
}

// Register the tool
let agent = Agent::builder(client, config)
    .with_default_tools()
    .register_tool(Box::new(MyTool))
    .build();
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Amadeus SDK                             │
│                                                             │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐       │
│  │ Agent   │  │ Tools   │  │ Client  │  │ Policy  │       │
│  │ Loop    │  │ Registry│  │ Trait   │  │ System  │       │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘       │
│                                                             │
│  ┌──────────────────────────────────────────────────┐      │
│  │         Streaming Event System                    │      │
│  └──────────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

### Core Components

- **Agent Loop**: Orchestrates LLM interactions and tool execution
- **LLM Client**: Trait-based abstraction for provider swapping
- **Tool Registry**: Dynamic tool registration and execution
- **Policy System**: Approval-based safety controls
- **Event Stream**: Real-time updates via async streams

## Built-in Tools

| Tool | Description | Safety |
|------|-------------|--------|
| `bash` | Execute shell commands | Requires approval for dangerous commands |
| `read_file` | Read file contents | Auto-approved |
| `write_file` | Write/create files | Requires approval for sensitive paths |
| `edit_file` | Surgical file edits | Requires approval |
| `glob` | Pattern-based file matching | Auto-approved |
| `grep` | Search file contents | Auto-approved |
| `web_fetch` | Fetch web content | Requires approval |

## Policy System

Control tool execution with three modes:

### Auto Mode
```rust
let mut policy = Policy::new();
policy.set_mode(ApprovalMode::Auto);
// All tools execute automatically
```

### Ask Mode (Default)
```rust
let mut policy = Policy::new();
policy.set_mode(ApprovalMode::Ask);
// Only dangerous operations require approval
policy.add_auto_approve("read_file");
policy.add_auto_approve("glob");
```

### Strict Mode
```rust
let mut policy = Policy::new();
policy.set_mode(ApprovalMode::Strict);
// All tools require approval except those in auto_approve list
```

### Dangerous Patterns

The policy system blocks:
- `sudo` commands
- `chmod 777`
- `rm -rf /`
- Writing to `.env`, `.pem`, `.key` files
- Shell pipe to bash/sh

## Configuration

Environment variables (`.env`):

```bash
# Provider selection
PROVIDER=anthropic  # or "openai"

# Anthropic
ANTHROPIC_API_KEY=sk-ant-xxx
ANTHROPIC_BASE_URL=https://api.anthropic.com  # optional
ANTHROPIC_MODEL=claude-sonnet-4-5-20250929

# OpenAI
OPENAI_API_KEY=sk-xxx
OPENAI_BASE_URL=https://api.openai.com/v1  # optional
OPENAI_MODEL=gpt-4

# Agent settings
TIMEOUT_SECONDS=120
MAX_OUTPUT_BYTES=50000
WORKDIR=/path/to/project
SESSION_LOG_DIR=./logs
SESSION_LOG_COMPRESS=true

# Blocked commands (comma-separated)
BLOCKED_COMMANDS=rm -rf /,sudo
```

## Features

Enable features in `Cargo.toml`:

```toml
[dependencies]
amadeus = { git = "https://github.com/xxraincandyxx/Amadeus", features = ["full"] }
```

Available features:

- `tui` - Terminal UI (ratatui-based)
- `api` - HTTP API server (axum-based)
- `concurrency` - Concurrency primitives
- `supervisor` - Multi-agent supervisor
- `mesh` - Distributed agent coordination
- `context` - Context management
- `full` - All features

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

Session files are stored in JSON or compressed JSON.gz format.

## Event Streaming

Monitor agent execution in real-time:

```rust
let mut stream = agent.run_stream();

while let Some(event) = stream.next().await {
    match event? {
        AgentEvent::TextDelta { delta } => print!("{}", delta),
        AgentEvent::ToolStart { id, name } => {
            println!("\n[Tool: {}]", name);
        }
        AgentEvent::ToolComplete { name, output, .. } => {
            println!("Output: {}", output);
        }
        AgentEvent::TokenUsage { total_tokens, .. } => {
            println!("\nTokens: {}", total_tokens);
        }
        AgentEvent::Done { result } => {
            println!("\nComplete!");
        }
        _ => {}
    }
}
```

## Development

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with specific features
cargo run --features tui
```

### Project Structure

```
amadeus/
├── src/
│   ├── agent/         # Agent loop, events, messages
│   ├── client/        # LLM client implementations
│   ├── tools/         # Tool registry and tools
│   ├── policy/        # Approval system
│   ├── ui/            # Terminal UI components
│   ├── api/           # HTTP API handlers
│   ├── hooks/         # Hook system
│   ├── skills/        # Skill templates
│   ├── mcp/           # Model Context Protocol
│   └── error.rs       # Error types
├── tests/             # Integration tests
├── examples/          # Example programs
└── docs/              # Documentation
```

### Code Count

```bash
./count-code.sh
```

## API Reference

### Agent

```rust
// Create agent
let agent = Agent::new(client, config);

// Builder pattern
let agent = Agent::builder(client, config)
    .with_default_tools()
    .with_policy(policy)
    .with_hooks(hooks)
    .build();

// Run
let result = agent.run("prompt").await?;

// Stream
let stream = agent.run_stream();
```

### LLMClient Trait

```rust
#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn create_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)>;
    
    async fn create_message_stream(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>>;
}
```

## Error Handling

```rust
use amadeus::{AgentError, Result};

match agent.run("prompt").await {
    Ok(result) => println!("{}", result.text),
    Err(AgentError::Timeout(secs)) => eprintln!("Timed out after {}s", secs),
    Err(AgentError::ToolNotFound(tool)) => eprintln!("Tool not found: {}", tool),
    Err(AgentError::CommandBlocked(cmd)) => eprintln!("Command blocked: {}", cmd),
    Err(e) => eprintln!("Error: {}", e),
}

// Check if error is retryable
if error.is_retryable() {
    // Retry the operation
}
```

## Examples

See the `examples/` directory for more:

- `examples/tui/` - Terminal UI example
- `examples/server/` - HTTP server example

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `cargo test`
5. Submit a pull request

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Acknowledgments

Built with:
- [tokio](https://tokio.rs/) - Async runtime
- [reqwest](https://docs.rs/reqwest/) - HTTP client
- [ratatui](https://ratatui.rs/) - Terminal UI
- [serde](https://serde.rs/) - Serialization
- [tracing](https://docs.rs/tracing/) - Logging

## Contact

- Repository: https://github.com/xxraincandyxx/Amadeus
- Issues: https://github.com/xxraincandyxx/Amadeus/issues
