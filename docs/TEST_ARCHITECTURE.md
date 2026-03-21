# Test Architecture

Amadeus uses a comprehensive testing strategy with multiple layers: unit tests in source files, integration tests, and mock-first testing.

## Overview

```
tests/
├── UNIT TESTS IN SOURCE (37 files in src/)
│   └── #[cfg(test)] modules - e.g., footer.rs, compaction.rs, events.rs
│
├── INTEGRATION TESTS (tests/*.rs)
│   │
│   ├── CORE AGENT
│   │   ├── agent_test.rs           # Agent creation/config
│   │   ├── agent_integration_test.rs # Full lifecycle
│   │   ├── compaction_test.rs       # Context compaction
│   │   └── mock_functional_test.rs   # Mock LLM functional tests
│   │
│   ├── MESSAGING & TOOLS
│   │   ├── messages_test.rs        # Message serialization
│   │   ├── todo_test.rs            # Todo tool
│   │   ├── bash_test.rs            # Bash tool behavior
│   │   └── tool_approval_test.rs   # Approval workflow
│   │
│   ├── MULTI-AGENT
│   │   ├── p2p_test.rs             # Basic delegation (supervisor)
│   │   ├── simulation_p2p.rs       # P2P collaboration simulation
│   │   ├── sub_agnet_test.rs       # Sub-agent behavior
│   │   └── e2e_product_flow.rs    # E2E product dev simulation
│   │
│   ├── STREAMING & UI
│   │   ├── streaming_scenarios_test.rs # Streaming scenarios
│   │   ├── error_recovery_test.rs  # Error handling
│   │   └── monitoring_harness_test.rs # Monitoring harness
│   │
│   ├── CONFIG & INFRA
│   │   ├── config_test.rs          # Config loading
│   │   └── mock_llm.rs             # Mock LLM utilities
│   │
│   └── [MOCKS] tests/mocks/
│       ├── scenario_client.rs       # Scripted scenario mock
│       ├── flaky_client.rs         # Failover mock
│       ├── slow_client.rs          # Delay mock
│       └── mod.rs
│
└── [SCENARIOS] tests/scenarios/
    ├── builder.rs                  # ScenarioBuilder
    ├── runner.rs                   # ScenarioRunner
    ├── timeline.rs                 # EventTimeline
    ├── assertions.rs               # Test assertions
    ├── streaming_buffer.rs         # Buffer tests
    └── cursor_positioning.rs       # Cursor tests
```

## Test Categories

| Category | Files | Purpose |
|----------|-------|---------|
| Agent Core | 4 | Agent lifecycle, config, compaction |
| Multi-Agent | 4 | P2P, supervisor, sub-agents |
| Tools | 4 | Bash, todo, messages, approval |
| Streaming | 2 | Streaming scenarios, error recovery |
| Infrastructure | 3 | Config, mocks, monitoring |
| UI/Scenarios | 6 | Buffer, cursor, assertions, timeline |

## Mock Strategy

LLM clients are mocked to avoid external API calls during testing:

```
LLMClient implementations:
├── MockLLM (mock_llm.rs)           - Simple response list
├── StatefulMockClient (mock_functional_test.rs) - Stateful with tool support
├── SimpleMockClient (p2p_test.rs)  - P2P specific
├── SimulationMockClient (simulation_p2p.rs) - P2P simulation
├── ScenarioMockClient (mocks/)     - Scripted scenarios ← PRIMARY
├── FlakyMockClient (mocks/)        - Error injection
└── SlowMockClient (mocks/)         - Delay injection
```

### Primary Mock: ScenarioMockClient

```rust
use mocks::ScenarioMockClient;
use scenarios::{ScenarioBuilder, ScenarioRunner};

let chunks = vec![
    StreamEvent::TextDelta("Hello ".to_string()),
    StreamEvent::TextDelta("world".to_string()),
    StreamEvent::StopReason("end_turn".to_string()),
];

let client = ScenarioMockClient::scripted(vec![chunks]);

let scenario = ScenarioBuilder::new("test_scenario")
    .description("Test description")
    .build();

let runner = ScenarioRunner::new(scenario);
let (events, text) = runner.execute_and_collect_text(client).await?;
```

### Specialized Mocks

| Mock | Purpose | Usage |
|------|---------|-------|
| `FlakyMockClient` | Simulate network failures | `FlakyMockClient::with_failures(vec![0])` |
| `SlowMockClient` | Test timeouts/delays | `SlowMockClient::slow()` |
| `ScenarioMockClient` | Scripted event sequences | `scripted(vec![chunks])` |

## Scenario Framework

The scenario framework (`tests/scenarios/`) provides structured test infrastructure:

### Components

- **ScenarioBuilder** - Create test scenarios with metadata
- **ScenarioRunner** - Execute scenarios and collect results
- **EventTimeline** - Track timestamped events for assertions
- **Assertions** - Custom assertion helpers

### Example

```rust
#[tokio::test]
async fn test_streaming_accumulation() {
    let chunks = vec![
        StreamEvent::TextDelta("Chunk 1".to_string()),
        StreamEvent::TextDelta("Chunk 2".to_string()),
        StreamEvent::StopReason("end_turn".to_string()),
    ];

    let client = ScenarioMockClient::scripted(vec![chunks]);

    let scenario = ScenarioBuilder::new("streaming_test")
        .description("Test text accumulation")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let (_events, text) = runner
        .execute_and_collect_text(client)
        .await
        .expect("Scenario failed");

    assert_eq!(text, "Chunk 1Chunk 2");
}
```

## Running Tests

```bash
# Run all tests
cargo test --features full

# Run specific test file
cargo test --test p2p_test --features full

# Run specific test
cargo test test_name --features full

# Run with output
cargo test --features full -- --nocapture

# Run integration tests only
cargo test --test p2p_test --features full
cargo test --test simulation_p2p --features full
cargo test --test e2e_product_flow --features full
```

## Test Organization Principles

1. **Mock-First**: All external API calls are mocked to avoid costs and ensure reproducibility
2. **Scenario-Based**: Complex behaviors use the ScenarioRunner for structured testing
3. **Layered**: Unit tests in source, integration tests in `tests/`
4. **Self-Contained**: Each test file is independent and can run in isolation

## Inline Unit Tests

Many source files contain inline tests using `#[cfg(test)]`:

```rust
// src/agent/compaction.rs
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_compaction_triggers_at_threshold() {
        // ...
    }
}
```

These are automatically compiled and run with `cargo test`.

## See Also

- [TDD Best Practices](../CLAUDE.md#testing-strategy) - Testing principles
- [TOOLS.md](TOOLS.md) - Tool system documentation
- [API_GUIDE.md](API_GUIDE.md) - API testing
