// @amadeus-header
// summary: Compatibility layer re-exporting shared project context loading.
// layer: infra
// status: active
// feature_flags: none
// provides:
// - module: crate::context
// - type: crate::context::ProjectContext
// - fn: crate::context::load_context_prompt
// uses:
// - module: amadeus_context
// invariants:
// - Core context call sites stay compatible while context loading lives in a dedicated crate.
// side_effects: none
// tests:
// - cmd: cargo test -p context
// @end-amadeus-header

//! Compatibility layer for shared project context loading.

pub use amadeus_context::{load_context_prompt, ProjectContext};
