// @amadeus-header
// summary: Compatibility facade re-exporting the core SDK and optional frontend adapters.
// layer: infra
// status: active
// feature_flags:
// - api
// - concurrency
// - context
// - mesh
// - supervisor
// - test-utils
// - tui
// provides:
// - module: crate
// uses:
// - module: amadeus_core
// invariants:
// - Public amadeus module paths stay compatible while implementation moves into crates.
// side_effects: none
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

//! Compatibility facade for the Amadeus workspace crates.

pub use amadeus_core::*;

#[cfg(feature = "api")]
pub use amadeus_api::api;

#[cfg(feature = "tui")]
pub use amadeus_tui::ui;
