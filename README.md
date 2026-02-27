# Amadeus - AI Agent SDK

**Bash is all you need.** A high-performance, modular AI agent SDK implemented in Rust.

## Philosophy

Amadeus is an **Agent SDK**, designed to be the core engine for AI applications. It follows a minimalist "Bash-first" philosophy, where a single robust tool enables the agent to perform almost any computing task.

The SDK focuses on the **Agent Loop (ReAct)**, **LLM Orchestration**, and **High-Performance Tool Execution**, leaving platform concerns like session management and UI to the integration layer.

## Features

- **Modular SDK Architecture** - Gated by feature flags (`tui`, `api`, `supervisor`) to keep dependencies minimal.
- **Multi-Agent Orchestration** - Reactive `Supervisor` with task queuing, backpressure, and load balancing.
- **P2P Collaboration** - Agents can recursively delegate sub-tasks to peers with specific capabilities.
- **Multi-Provider Support** - Native clients for Anthropic and OpenAI with a unified interface.
- **High-Performance Bash Tool** - Surgical file operations and command execution with timeout and output management.
- **Modern Dracula TUI** - A sleek, event-driven terminal interface for rapid testing and interaction.
- **Production-Ready** - Built on Tokio with comprehensive error handling and thread-safe concurrency.

## Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/xxraincandyxx/Amadeus.git
cd amadeus

# Setup environment
cp .env.example .env
# Add your ANTHROPIC_API_KEY or OPENAI_API_KEY to .env
```

### Usage Modes

#### 1. Interactive TUI (Human-in-the-loop)
```bash
cargo run --example tui --features tui
```

#### 2. Single Task CLI
```bash
cargo run -- "List the files in the current directory and summarize the project structure"
```

#### 3. HTTP API Server
```bash
cargo run --example server --features api
```

## SDK Integration

### Basic Agent
```rust
use amadeus::{AgentBuilder, OpenAIClient, Config};

let sdk_config = Arc::new(Config::load()?);
let client = OpenAIClient::new(api_key, None, model);

let agent = AgentBuilder::new(client, sdk_config)
    .with_default_tools() // Includes Bash and File tools
    .build();

let result = agent.run("Write a rust function to calculate fibonacci").await?;
println!("Result: {}", result.text);
```

### Multi-Agent Supervisor
```rust
let mut supervisor = Supervisor::new(client, SupervisorConfig::default(), sdk_config);

// Spawn specialized workers
supervisor.spawn(vec![
    WorkerConfig::new("Coder").capability("rust").capability("bash"),
    WorkerConfig::new("Reviewer").capability("security"),
]).await?;

// Execute tasks via the supervisor
let task = Task::new("task-1", "Implement a secure API endpoint")
    .requires(vec!["rust".into()]);
let result = supervisor.execute(task).await?;
```

## Project Structure

- `src/agent/` - Core ReAct loop, Supervisor orchestration, and Worker logic.
- `src/client/` - LLM provider implementations (Anthropic, OpenAI).
- `src/tools/` - Extensible tool system (Bash, Peer delegation, File ops).
- `src/ui/` - Modern Dracula-themed TUI components.
- `src/api/` - Axum-based HTTP handlers for SDK-as-a-service.
- `tests/` - Comprehensive test suite including P2P simulations and E2E flows.

## Documentation

- **[ARCHITECTURE.md](docs/ARCHITECTURE-V3.md)** - Actor-based agents, concurrency, and backpressure.
- **[TUI_GUIDE.md](docs/TUI_GUIDE.md)** - Guide to using the modernized terminal interface.
- **[REST_API.md](docs/REST_API.md)** - Comprehensive reference for the Axum REST APIs.
- **[SDK_SCOPE.md](docs/SDK_SCOPE.md)** - Definition of SDK vs. Platform responsibilities.
- **[INTEGRATION_GUIDE.md](docs/INTEGRATION_GUIDE.md)** - How to build platforms (like NeuroCore) on top of Amadeus.
- **[TEST_FLOW.md](docs/TEST_FLOW.md)** - Guide to the internal testing architecture.

## License

MIT
