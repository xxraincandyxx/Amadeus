// @amadeus-header
// summary: Test module root for mocks coverage and shared exports.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::mocks
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

mod flaky_client;
mod scenario_client;
mod slow_client;

#[allow(unused_imports)]
pub use flaky_client::FlakyMockClient;
pub use scenario_client::ScenarioMockClient;
#[allow(unused_imports)]
pub use slow_client::SlowMockClient;
