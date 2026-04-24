# Amadeus - AI Agent SDK

A Rust SDK for building AI agents with LLM support, featuring multi-provider compatibility, streaming responses, a powerful tool system, and both HTTP and terminal adapters over a shared core runtime.

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
- **Multi-Agent Coordination**: Orchestra-based local routing and delegated task execution

Parity progress against the `refs/claw-code-parity` reference is treated as a testing problem and should only be advanced when covered by automated tests in this repository.

## Installation

### Prerequisites

- Rust 1.70 or later
- API key from Anthropic or OpenAI

### Setup

```bash
git clone https://github.com/xxraincandyxx/Amadeus.git
cd Amadeus

# Copy structured settings template
mkdir -p .amadeus
cp .amadeus/settings.example.json .amadeus/settings.json

# Add your provider and API key to .amadeus/settings.json

cargo build --release --features full
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

Amadeus is a workspace-based system rather than a single large crate.

- The root `amadeus` crate is a compatibility facade.
- `crates/core` contains the agent loop, provider clients, tools, policy, and orchestration runtime.
- `crates/runtime` contains reusable coordination models and dispatch logic.
- `crates/api` is the Axum HTTP adapter.
- `crates/tui` is the ratatui terminal adapter.

Runtime ingress looks like this:

```text
CLI/library call
  -> config + provider selection
  -> core runtime
  -> TUI adapter | HTTP adapter | assessment runner
```

The main live execution path is the ReAct-style `Agent` loop in `crates/core/src/agent/loop_agent.rs`, while local multi-agent routing is handled by `AgentOrchestrator` in `crates/core/src/agent/orchestra.rs`.

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

Structured settings live in `.amadeus/settings.json`, with optional global defaults in `~/.amadeus/settings.json` and workspace overrides in `.amadeus/settings.local.json`:

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

## Features

Enable features in `Cargo.toml`:

```toml
[dependencies]
amadeus = { git = "https://github.com/xxraincandyxx/Amadeus", features = ["full"] }
```

Available features:

- `api` - HTTP adapter and API surface, implies `orchestra`
- `tui` - Terminal UI adapter, implies `concurrency`
- `concurrency` - Locking and shared coordination primitives
- `orchestra` - Canonical multi-agent orchestration surface
- `team` - Legacy alias for `orchestra`
- `supervisor` - Legacy alias for `orchestra`
- `context` - Context management support
- `test-utils` - Test helpers and recording support
- `full` - All of the above

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
cargo build --features full

# Release build
cargo build --release --features full

# Run tests
cargo test --features full

# Run with specific features
cargo run --features tui
```

### Project Structure

```text
amadeus/
├── src/                # facade crate + CLI bootstrap
├── crates/
│   ├── core/           # agent loop, tools, policy, orchestration
│   ├── runtime/        # shared orchestration/task models
│   ├── api/            # Axum adapter
│   ├── tui/            # ratatui adapter
│   ├── config/         # layered settings loading
│   ├── commands/       # slash commands and helpers
│   ├── skills/         # skill loading
│   └── ...             # supporting shared crates
├── tests/              # integration suites and harnesses
├── examples/           # example bootstraps
└── docs/               # architecture and workflow docs
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
