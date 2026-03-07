# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Amadeus is a Rust SDK for building AI agents with LLM support, featuring multi-provider compatibility (Anthropic, OpenAI), streaming responses, and a powerful tool system. The project uses the ReAct (Reason + Act) pattern for agent orchestration with Tokio async runtime.

## Common Commands

### Building

```bash
# Build with all features (recommended for development)
cargo build --features full

# Build release
cargo build --release --features full

# Build with specific features only
cargo build --features tui        # Terminal UI only
cargo build --features api         # HTTP API only
cargo build --features supervisor   # Multi-agent system
```

### Running

```bash
# Run TUI (Terminal UI)
cargo run --features full

# Run HTTP API server (default port 3000)
cargo run --features full -- --server

# Run HTTP API server on custom port
cargo run --features full -- --server 8080

# Run example programs
cargo run --example tui --features tui
cargo run --example server --features api
```

### Testing

```bash
# Run all tests (including simulations)
cargo test --features full

# Run specific test
cargo test test_name --features full

# Run integration tests only
cargo test --test p2p_test --features full
cargo test --test simulation_p2p --features full
cargo test --test e2e_product_flow --features full

# Show test output
cargo test --features full -- --nocapture
```

### Linting & Checking

```bash
# Check without building
cargo check --features full

# Format code
cargo fmt

# Run clippy
cargo clippy --features full
```

## Feature Flags

Amadeus is highly modular. Use feature flags to keep builds lean:

- `tui` - Terminal UI components (ratatui-based)
- `api` - Axum-based HTTP server
- `concurrency` - Concurrency primitives (locks, coordination)
- `supervisor` - Multi-agent orchestration system (implies `concurrency`)
- `mesh` - Distributed agent coordination (implies `supervisor`)
- `context` - Context management
- `test-utils` - Test utilities
- `full` - All features enabled

## Architecture Overview

### Core Components

```
src/
├── agent/           # Agent orchestration
│   ├── loop_agent.rs    # Main Agent loop - ReAct pattern implementation
│   ├── supervisor.rs     # Multi-agent supervisor for worker coordination
│   ├── worker.rs        # Worker agent implementation
│   ├── compaction.rs    # Context compaction for long conversations
│   ├── config.rs        # Configuration loading
│   ├── events.rs        # Event types (AgentEvent, ToolCall, etc.)
│   ├── messages.rs      # Message types (ContentBlock, Message)
│   └── mesh.rs         # Distributed mesh coordination
├── client/          # LLM provider clients
│   ├── anthropic.rs     # Anthropic API implementation
│   ├── openai.rs       # OpenAI API implementation
│   └── mod.rs          # LLMClient trait definition
├── tools/           # Tool system
│   ├── tool_trait.rs    # Tool trait definition
│   ├── registry.rs      # Tool registry for dynamic tool management
│   ├── bash.rs         # Shell command execution
│   ├── file.rs         # File operations (read, write, edit)
│   ├── glob.rs         # Pattern-based file matching
│   ├── grep.rs         # Content search
│   ├── web.rs          # Web fetching
│   └── peer.rs        # Peer-to-peer communication tools
├── policy/          # Approval system
│   └── mod.rs          # Policy configuration for tool approval
├── hooks/           # Extensibility hooks
│   ├── mod.rs          # Hook registry and trait definitions
│   └── shell.rs        # Shell command hooks
├── skills/          # Skills system
│   ├── mod.rs          # Skill definitions and loading
│   └── registry.rs     # Skill registry
├── mcp/             # Model Context Protocol
│   ├── client.rs       # MCP client implementation
│   └── adapter.rs      # MCP tool adapter
├── ui/              # Terminal UI (ratatui)
│   ├── app.rs          # Main TUI application
│   ├── components/     # UI components
│   └── themes/         # Color themes
├── api/             # HTTP API (axum)
│   ├── http.rs         # HTTP server setup
│   ├── handlers/       # API endpoints
│   └── types.rs        # API types
└── core/            # Core primitives
    ├── id.rs           # ID generation
    └── event.rs        # Event types
```

### Agent Loop (ReAct Pattern)

The heart of the SDK is in `src/agent/loop_agent.rs`. It implements the ReAct pattern:

1. **Turn-based execution**: Each interaction is a "turn" with text response and tool calls
2. **Internal history**: The `Agent` struct manages its own `Arc<RwLock<Vec<Message>>>` history
3. **Streaming**: Supports real-time event streaming via `run_stream()`
4. **Approval flow**: Tools requiring approval use channels for UI communication

### Multi-Agent System

Located in `src/agent/supervisor.rs` and `src/agent/worker.rs`:

- **Supervisor**: Manages a pool of specialized worker agents
- **Concurrency**: Uses `tokio::task::JoinSet` for parallel task execution
- **Queueing**: Implements `TaskQueue` with backpressure (`max_pending_tasks`)
- **P2P Collaboration**: Routes `HelpRequest` events between workers via a central bus

### Context Compaction

When conversations grow long, `src/agent/compaction.rs` provides automatic compaction:

- Monitors token usage in conversation history
- Triggers summarization when approaching context limits (default: 75% threshold)
- Preserves recent messages and important context
- Uses LLM to generate meaningful summaries

### LLM Client Trait

Provider-agnostic abstraction defined in `src/client/mod.rs`:

```rust
#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn create_message(...) -> Result<(String, Vec<ContentBlock>)>;
    async fn create_message_stream(...) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>>;
}
```

Implemented for Anthropic and OpenAI. The `Agent<C>` struct is generic over the LLM provider, allowing zero-cost provider switching.

### Tool System

Tools implement the `Tool` trait from `src/tools/tool_trait.rs`:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn schema(&self) -> &'static Value;
    async fn execute(&self, input: Value) -> Result<String>;
}
```

Built-in tools are registered in `ToolRegistry` (src/tools/registry.rs):
- `bash` - Execute shell commands
- `read_file` - Read file contents
- `write_file` - Write/create files
- `edit_file` - Surgical file edits
- `glob` - Pattern-based file matching
- `grep` - Search file contents
- `web_fetch` - Fetch web content

### Policy System

Located in `src/policy/mod.rs`, controls tool execution with three modes:

- **Auto**: Execute all tools automatically
- **Ask** (default): Ask for dangerous operations only
- **Strict**: Ask for all tool executions

Dangerous patterns are automatically blocked:
- `sudo` commands
- `chmod 777`
- `rm -rf /`
- Writing to `.env`, `.pem`, `.key` files
- Shell pipe to bash/sh

## Testing Strategy

Amadeus prioritizes **Mock-First Testing** to ensure stability without API costs.

### Unit Tests
Found in `src/` modules alongside the code they test.

### Integration Tests
Located in `tests/` directory:
- `p2p_test.rs` - Basic delegation verification
- `simulation_p2p.rs` - High-concurrency stress tests
- `e2e_product_flow.rs` - Narrative-driven product development simulation
- `agent_integration_test.rs` - Full agent lifecycle tests
- `compaction_test.rs` - Context compaction behavior
- `agent_test.rs` - Agent creation and configuration
- `bash_test.rs` - Bash tool behavior
- `config_test.rs` - Configuration loading
- `messages_test.rs` - Message serialization/deserialization
- `mock_functional_test.rs` - Mock LLM functional tests
- `monitoring_harness_test.rs` - Monitoring-first scenario harness coverage

### Mock Utilities
- `mockito` - HTTP mocking for LLM client tests
- `wiremock` - Alternative HTTP mocking
- `tests/mock_llm.rs` - Mock LLM client for integration tests
- `tests/mocks/scenario_client.rs` - Scenario-driven mock client with captured request snapshots
- `tests/scenarios/timeline.rs` - Timestamped event timeline for observability-focused assertions

## Code Style

- **Indentation**: 2 spaces (Google Rust Style Guide)
- **Naming**: `snake_case` for variables/functions, `PascalCase` for types
- **Error Handling**: Use `crate::error::Result` and avoid `unwrap()`
- **Async/await**: Use Tokio runtime throughout
- **Documentation**: Document public APIs with rustdoc comments

## Environment Configuration

Create `.env` file from `.env.example`:

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

Session files are stored in JSON or compressed JSON.gz format in `SESSION_LOG_DIR`.

## Key Design Patterns

1. **Actor-like Workers**: Workers are spawned as persistent configurations and managed by the Supervisor
2. **Generic Clients**: The `Agent<C>` struct is generic over the LLM provider, allowing zero-cost provider switching
3. **Reactive UI**: The TUI consumes an `AgentEvent` stream, decoupling logic from presentation
4. **Builder Pattern**: Use `Agent::builder()` for custom configuration with tools, policy, hooks, etc.
5. **Stream-based**: All major operations support streaming events for real-time monitoring

## Important File Paths

- `src/lib.rs` - Library entry point and public API re-exports
- `src/main.rs` - Binary entry point (TUI and server modes)
- `src/agent/loop_agent.rs` - Core agent loop implementation
- `src/agent/supervisor.rs` - Multi-agent supervisor
- `src/policy/mod.rs` - Approval/policy system
- `src/agent/compaction.rs` - Context compaction
- `tests/` - Integration tests directory
- `Cargo.toml` - Dependencies and feature flags
