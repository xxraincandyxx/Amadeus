// @amadeus-header
// summary: Test module root for scenarios coverage and shared exports.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::tui::scenarios
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

//! Scenario Tests
//!
//! Pre-built test scenarios for common TUI interactions.

mod scenarios_impl;

pub use scenarios_impl::*;
