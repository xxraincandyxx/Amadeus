// @amadeus-header
// summary: Compatibility wrapper re-exporting agent profiles from the profiles crate.
// layer: agent
// status: active
// feature_flags: none
// provides:
// - module: crate::agent::profile
// - type: crate::agent::profile::AgentProfile
// uses:
// - module: amadeus_profiles
// invariants:
// - Public profile paths remain stable while implementation lives outside core.
// side_effects: none
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

//! Compatibility re-exports for agent profiles.

pub use amadeus_profiles::AgentProfile;
