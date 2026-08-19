// @amadeus-header
// summary: Public facade for structured configuration models and layered settings loading.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - type: crate::Config
// - type: crate::ConfigError
// - type: crate::Language
// - type: crate::Provider
// uses:
// - module: crate::config
// invariants:
// - Existing crate-root configuration paths remain stable through re-exports.
// side_effects: none
// tests:
// - cmd: cargo test -p config
// @end-amadeus-header

//! Structured configuration loading for Amadeus runtimes.

mod config;

pub use config::*;
