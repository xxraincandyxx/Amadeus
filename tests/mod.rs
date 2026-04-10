// @amadeus-header
// summary: Test module root for tests coverage and shared exports.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

pub mod mock_llm;

pub mod agent_test;
pub mod bash_test;
pub mod config_test;
pub mod messages_test;

// New test infrastructure
pub mod mocks;
pub mod scenarios;

// Test flows (integration tests)
// [placeholder]
// pub mod flows;

// Stress tests
// [placeholder]
// pub mod stress;
