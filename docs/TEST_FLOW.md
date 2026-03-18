# Amadeus SDK Test Flow

This document describes the testing architecture and verification strategy for the Amadeus SDK.

## Test Structure

Amadeus follows a multi-tier testing strategy, emphasizing mock-based simulations for reliable, repeatable verification.

```
tests/
├── mod.rs                  # Test module organization
├── agent_test.rs           # Core agent loop unit tests
├── agent_integration_test.rs # Full agent lifecycle tests
├── bash_test.rs            # Bash tool execution tests
├── config_test.rs          # Environment and configuration tests
├── messages_test.rs        # Message serialization/deserialization tests
├── compaction_test.rs      # Context compaction behavior tests
│
├── mock_llm.rs             # Shared Mock LLM client for simulations
├── mock_functional_test.rs # High-signal ReAct loop simulation
├── p2p_test.rs             # Multi-agent delegation integration tests
├── simulation_p2p.rs       # High-concurrency stress tests
├── e2e_product_flow.rs     # Human-readable E2E narrative flow
│
├── error_recovery_test.rs  # Error handling and recovery tests
├── monitoring_harness_test.rs # Monitoring-first scenario harness tests
├── streaming_scenarios_test.rs # Streaming behavior tests
├── tool_approval_test.rs   # Policy/approval system tests
│
├── stress_*.rs             # Stress tests (concurrent, memory, race, rapid streaming)
├── scenarios_test.rs       # Scenario-based test runner
│
├── fixtures/               # Test data fixtures
├── mocks/                  # Mock implementations
├── scenarios/              # Test scenario definitions
└── unit/                   # Unit test utilities
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

### 2.5. Monitoring-First Scenario Harness
The scenario test harness is designed to observe the agent architecture in detail, not just assert final text output.

Key building blocks:
- `tests/mocks/scenario_client.rs` captures every request sent to the mock LLM, including `system`, `messages`, `tools`, and `max_tokens`
- `tests/scenarios/runner.rs` can execute a scenario and return an `EventTimeline`
- `tests/scenarios/timeline.rs` records timestamped `AgentEvent`s and the final agent history snapshot
- `tests/scenarios/assertions.rs` provides timeline-aware assertions for text, thinking, tools, approvals, token usage, compaction, and history shape

This makes it possible to verify:
- Exact event ordering and timing
- Tool input streaming and tool completion outputs
- Final conversation history shape after the run
- The content of each request the agent sent to the LLM mock
- Monitoring gaps in the current architecture when expected events are not emitted

### 2.6. TUI Frame Capture
When `SessionRecorder` is enabled, the TUI now writes frame snapshots to `tui_capture.log` alongside the structured session JSON.

Captured frame data includes:
- frame size and timestamp
- per-cell symbol content
- foreground/background colors
- modifiers and underline color

This is useful when debugging visual regressions in the live terminal UI.

### 3. Integration Tests
- `agent_integration_test.rs` - Full agent lifecycle verification
- `compaction_test.rs` - Context compaction behavior
- `monitoring_harness_test.rs` - End-to-end observability coverage for the scenario harness
- `streaming_scenarios_test.rs` - Streaming response handling
- `tool_approval_test.rs` - Policy/approval system behavior
- `error_recovery_test.rs` - Error handling and recovery

### 4. Orchestration Stress Testing
`simulation_p2p.rs` and `stress_*.rs` files launch high-concurrency bursts to verify:
- **Deadlock Resistance**: Circular peer dependencies are broken by timeouts.
- **Backpressure**: The `TaskQueue` correctly buffers and eventually overflows under extreme load.
- **Worker Saturation**: Tasks are fairly distributed among available workers.

### 5. Narrative E2E Flow
`e2e_product_flow.rs` provides a human-readable "story" of a Product Manager, Coder, and Reviewer collaborating. This verifies the high-level developer experience and cross-agent collaboration logic.

## CI/CD Pipeline

The standard verification pipeline for a commit includes:
1. `cargo fmt --check`
2. `cargo clippy --all-features -- -D warnings`
3. `cargo test --all-features`
4. `cargo build --examples --features full`

---
*Last updated: 2026-03-07*
