# Amadeus SDK Test Flow

This document describes the testing architecture and verification strategy for the Amadeus SDK.

## Test Structure

Amadeus follows a multi-tier testing strategy, emphasizing mock-based simulations for reliable, repeatable verification.

```
tests/
├── agent_test.rs           # Core agent loop unit tests
├── bash_test.rs            # Bash tool execution tests
├── config_test.rs          # Environment and configuration tests
├── messages_test.rs        # Message serialization/deserialization tests
│
├── mock_llm.rs             # Shared Mock LLM client for simulations
├── mock_functional_test.rs  # High-signal ReAct loop simulation
├── p2p_test.rs             # Multi-agent delegation integration tests
├── simulation_p2p.rs       # High-concurrency stress tests (Saturation/Deadlock)
└── e2e_product_flow.rs     # Human-readable E2E narrative flow
```

## Running Tests

Tests are gated by feature flags. The `full` feature is recommended for comprehensive verification.

```bash
# Run all tests
cargo test --features full

# Run a specific integration test
cargo test --test simulation_p2p --features supervisor

# Run with stdout capture (useful for narrative flows)
cargo test --test e2e_product_flow --features full -- --nocapture
```

## Verification Tiers

### 1. Unit Verification
Ensures individual components (bash tool, config parser, message formats) behave correctly in isolation.

### 2. Functional Simulation
Uses `mock_functional_test.rs` to simulate a complete ReAct loop turn-by-turn. This verifies that the agent correctly parses tool calls and integrates results into history without calling a real LLM.

### 3. Orchestration Stress Testing
`simulation_p2p.rs` launches high-concurrency bursts (e.g., 50+ tasks against 10 workers) to verify:
- **Deadlock Resistance**: Circular peer dependencies are broken by timeouts.
- **Backpressure**: The `TaskQueue` correctly buffers and eventually overflows under extreme load.
- **Worker Saturation**: Tasks are fairly distributed among available workers.

### 4. Narrative E2E Flow
`e2e_product_flow.rs` provides a human-readable "story" of a Product Manager, Coder, and Reviewer collaborating. This verifies the high-level developer experience and cross-agent collaboration logic.

## CI/CD Pipeline

The standard verification pipeline for a commit includes:
1. `cargo fmt --check`
2. `cargo clippy --all-features -- -D warnings`
3. `cargo test --all-features`
4. `cargo build --examples --features full`

---
*Last updated: 2026-02-27*
