# Amadeus SDK Development Guide

This document provides technical details, architectural insights, and contribution guidelines for the Amadeus SDK.

## Core Architecture

Amadeus is built on a modular, async architecture using **Tokio**. It follows the **ReAct (Reason + Act)** pattern for agent orchestration.

### 1. The Agent Loop (`src/agent/loop_agent.rs`)
The heart of the SDK. It manages the conversation state and orchestrates the interaction between the LLM and available tools.
- **Turn-based**: Each interaction is a "turn" that can include text response and tool calls.
- **Internal History**: The `Agent` struct manages its own `Arc<RwLock<Vec<Message>>>` history.
- **Streaming**: Supports real-time event streaming via `run_stream`.

### 2. Multi-Agent Supervisor (`src/agent/supervisor.rs`)
Manages a pool of specialized worker agents.
- **Concurrency**: Uses `tokio::task::JoinSet` for parallel task execution.
- **Queueing**: Implements a `TaskQueue` with backpressure (`max_pending_tasks`).
- **P2P Collaboration**: Routes `HelpRequest` events between workers via a central bus.

### 3. LLM Clients (`src/client/`)
Provider-agnostic abstractions for Anthropic and OpenAI.
- **Unified Interface**: Generic `LLMClient` trait ensures consistency.
- **Event-Driven**: Streaming results are normalized into `StreamEvent` tokens.

## Development Workflow

### Feature Flags
Amadeus is highly modular. Use feature flags to keep your build lean:
- `tui`: Terminal User Interface components.
- `api`: Axum-based HTTP server.
- `supervisor`: Multi-agent orchestration system.
- `full`: Enables all optional features.

### Commands
```bash
# Build with all features
cargo build --features full

# Run the TUI test harness
cargo run --example tui --features tui

# Run the HTTP API server
cargo run --example server --features api

# Run all tests (including simulations)
cargo test --features full
```

## Testing Strategy

Amadeus prioritizes **Mock-First Testing** to ensure stability without API costs.
- **Unit Tests**: Found in `src/` modules.
- **Integration Tests**: Located in `tests/`.
  - `p2p_test.rs`: Basic delegation verification.
  - `simulation_p2p.rs`: High-concurrency stress tests.
  - `e2e_product_flow.rs`: Narrative-driven product development simulation.

## Design Patterns

1. **Actor-like Workers**: Workers are spawned as persistent configurations and managed by the Supervisor.
2. **Generic Clients**: The `Agent<C>` struct is generic over the LLM provider, allowing zero-cost provider switching.
3. **Reactive UI**: The TUI consumes an `AgentEvent` stream, decoupling logic from presentation.

## Contribution Guidelines

1. **Surgical Changes**: Use surgical updates for code modifications.
2. **Defensive Programming**: Use `crate::error::Result` and avoid `unwrap()`.
3. **Google Style**: Follow the strict Google Rust Style Guide (2-space indent, snake_case).
4. **Validation**: Always run `cargo check` and relevant tests before pushing.
5. **Header Maintenance**: In-scope source files must carry and maintain the canonical header defined in `docs/SOURCE_FILE_HEADERS.md`.

---
*El Psy Kongroo*
