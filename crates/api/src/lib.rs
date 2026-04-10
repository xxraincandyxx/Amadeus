// @amadeus-header
// summary: HTTP adapter crate reusing the core SDK and exposing REST API modules.
// layer: api
// status: active
// feature_flags:
// - none
// provides:
// - module: crate::api
// uses:
// - module: amadeus_core
// invariants:
// - API modules adapt core runtime behavior without owning agent business logic.
// side_effects: none
// tests:
// - cmd: cargo test --features full agent_integration_test
// @end-amadeus-header

//! HTTP API adapter crate for Amadeus.

pub use amadeus_core::*;

pub mod api;
