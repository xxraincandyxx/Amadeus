// @amadeus-header
// summary: Module root for the core subsystem and its exports.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::core
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - tests/mod.rs
// @end-amadeus-header

//! Core primitives for the SDK

pub mod event;
pub mod id;

pub use event::{Event, EventEntry};
pub use id::{AgentId, CommitId};
