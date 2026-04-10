// @amadeus-header
// summary: Core primitive definitions for event.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::core::event
// - type: crate::core::event::EventEntry
// - type: crate::core::event::Event
// uses:
// - module: amadeus_events
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects: none
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

//! Event types for the SDK

pub use amadeus_events::{Event, EventEntry};
