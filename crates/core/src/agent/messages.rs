// @amadeus-header
// summary: Compatibility wrapper re-exporting message model types from the messages crate.
// layer: agent
// status: active
// feature_flags: none
// provides:
// - module: crate::agent::messages
// - type: crate::agent::messages::ContentBlock
// - type: crate::agent::messages::Message
// uses:
// - module: amadeus_messages
// invariants:
// - Public message model paths remain stable while implementation lives outside core.
// side_effects: none
// tests:
// - tests/messages_test.rs
// @end-amadeus-header

//! Compatibility re-exports for message model types.

pub use amadeus_messages::{ContentBlock, Message};
