// @amadeus-header
// summary: TUI adapter crate reusing the core SDK and exposing terminal UI modules.
// layer: ui
// status: active
// feature_flags:
// - test-utils
// provides:
// - module: crate::ui
// uses:
// - module: amadeus_core
// invariants:
// - TUI modules depend on exported core APIs instead of owning agent business logic.
// side_effects: none
// tests:
// - cmd: cargo test --features full tui_snapshot_test
// @end-amadeus-header

//! Terminal UI adapter crate for Amadeus.

pub use amadeus_core::*;

pub mod ui;
