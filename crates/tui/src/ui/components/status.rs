// @amadeus-header
// summary: TUI component implementation for status.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::components::status
// uses: none
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects: none
// tests:
// - tests/tui_snapshot_test.rs
// @end-amadeus-header

//! Status component module.
//!
//! This module previously contained AppState enum and StatusBar component.
//! Both have been removed - Footer now handles all status display.
