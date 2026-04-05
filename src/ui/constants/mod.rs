// @amadeus-header
// summary: Module root for the constants subsystem and its exports.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::constants
// - const: crate::ui::constants::PHRASE_CHANGE_INTERVAL_MS
// - const: crate::ui::constants::COLOR_CYCLE_DURATION_MS
// - const: crate::ui::constants::SPINNER_FRAME_INTERVAL_MS
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - tests/mod.rs
// @end-amadeus-header

pub mod tips;
pub mod witty_phrases;

pub use tips::INFORMATIVE_TIPS;
pub use witty_phrases::WITTY_LOADING_PHRASES;

pub const PHRASE_CHANGE_INTERVAL_MS: u64 = 15000;
pub const COLOR_CYCLE_DURATION_MS: u64 = 4000;
pub const SPINNER_FRAME_INTERVAL_MS: u64 = 80;
