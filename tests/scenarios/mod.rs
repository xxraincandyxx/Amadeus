// @amadeus-header
// summary: Test module root for scenarios coverage and shared exports.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::scenarios
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

mod assertions;
mod builder;
mod cursor_positioning;
mod runner;
mod streaming_buffer;
pub mod timeline;

#[allow(unused_imports)]
pub use assertions::*;
#[allow(unused_imports)]
pub use builder::{Scenario, ScenarioBuilder};
#[allow(unused_imports)]
pub use runner::ScenarioRunner;
#[allow(unused_imports)]
pub use timeline::EventTimeline;
