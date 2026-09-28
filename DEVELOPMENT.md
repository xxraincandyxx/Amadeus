# Amadeus Development Guide

This document provides technical details, architectural insights, and contribution guidelines for Amadeus.

## Core Architecture

Amadeus is built on a modular, async architecture using **Tokio**. It follows the **ReAct (Reason + Act)** pattern for agent orchestration.

### 1. The Agent Loop (`crates/core/src/agent/loop_agent.rs`)
The heart of the SDK. It manages the conversation state and orchestrates the interaction between the LLM and available tools.
- **Turn-based**: Each interaction is a "turn" that can include text response and tool calls.
- **Internal History**: The `Agent` struct manages its own `Arc<RwLock<Vec<Message>>>` history.
- **Streaming**: Supports real-time event streaming via `run_stream`.

### 2. Multi-Agent Orchestra (`crates/core/src/agent/orchestra.rs`)
Manages a pool of specialized worker agents.
- **Concurrency**: Uses `tokio::task::JoinSet` for parallel task execution.
- **Queueing**: Implements a `TaskQueue` with backpressure (`max_pending_tasks`).
- **P2P Collaboration**: Routes `HelpRequest` events between workers via a central bus.

### 3. LLM Clients (`crates/core/src/client/`)
Provider-agnostic abstractions for Anthropic and OpenAI.
- **Unified Interface**: Generic `LLMClient` trait ensures consistency.
- **Event-Driven**: Streaming results are normalized into `StreamEvent` tokens.

## Development Workflow

### Feature Flags
Amadeus is highly modular. Use feature flags to keep your build lean:
- `tui`: Terminal User Interface components.
- `api`: Axum-based HTTP server.
- `orchestra`: Multi-agent orchestration system.
- `full`: Enables all optional features.

### Commands
```bash
cargo check --features full        # Type-check the workspace
cargo test --features full         # Run the test suite
cargo clippy --all-features -- -D warnings   # Lint
cargo fmt --all                    # Format
./verify.sh                        # Full verification gate (CI parity)

# Client (web workspace + macOS shell) checks
cd apps/client && npm run lint && npm run test
```

The HTTP API server runs with `cargo run --features full -- --server [PORT]` (default port 3000).

### Clean generated output
```bash
make clean
```

This removes Rust, web, and desktop builds along with generated logs, benchmark results, test output, and local caches. Dependency installations and user configuration are preserved.

## Testing Strategy

Amadeus prioritizes **Mock-First Testing** to ensure stability without API costs.
- **Unit Tests**: Found in `src/` modules.
- **Integration Tests**: Located in `tests/`.
  - `p2p_test.rs`: Basic delegation verification.
  - `simulation_p2p.rs`: High-concurrency stress tests.
  - `e2e_product_flow.rs`: Narrative-driven product development simulation.

## Design Patterns

1. **Actor-like Workers**: Workers are spawned as persistent configurations and managed by the orchestra runtime.
2. **Generic Clients**: The `Agent<C>` struct is generic over the LLM provider, allowing zero-cost provider switching.
3. **Reactive UI**: The TUI consumes an `AgentEvent` stream, decoupling logic from presentation.

## Contribution Guidelines

1. **Surgical Changes**: Use surgical updates for code modifications.
2. **Defensive Programming**: Use `crate::error::Result`; no `unwrap()`/`expect()` outside `#[cfg(test)]`.
3. **Style**: `cargo fmt --all` defaults (4-space indent), `snake_case` functions, `PascalCase` types. See `CODING_STYLE.md`.
4. **Validation**: Always run `cargo check --features full` and relevant tests before pushing; `./verify.sh` before opening a PR.
5. **Header Maintenance**: In-scope source files must carry and maintain the canonical header defined in `docs/SOURCE_FILE_HEADERS.md`.

---
*El Psy Kongroo*
