// @amadeus-header
// summary: Module root for the scroll subsystem and its exports.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::scroll
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - tests/mod.rs
// @end-amadeus-header

mod animated_scrollbar;
mod scroll_state;

pub use animated_scrollbar::AnimatedScrollbar;
pub use scroll_state::ScrollState;
