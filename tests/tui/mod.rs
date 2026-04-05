#![allow(dead_code, unused_imports, clippy::result_large_err)]
// @amadeus-header
// summary: Test module root for tui coverage and shared exports.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::tui
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

//! TUI Snapshot Testing Infrastructure
//!
//! Complete visual regression testing for the terminal UI.

pub mod capture;
pub mod comparison;
pub mod harness;
pub mod scenarios;

// Re-exports for convenience
pub use capture::{TuiCapture, TuiFrameSnapshot};
pub use comparison::{compare, format_diff, FrameDiff};
pub use harness::{run_scenario, InputSequence, TuiTestHarness};
