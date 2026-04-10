// @amadeus-header
// summary: Compatibility wrapper re-exporting identifier primitives from the ids crate.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::core::id
// - type: crate::core::id::AgentId
// - type: crate::core::id::TeamId
// - type: crate::core::id::CommitId
// - type: crate::core::id::TxId
// - type: crate::core::id::SnapshotId
// uses:
// - module: amadeus_ids
// invariants:
// - Public identifier paths remain stable while implementation lives outside core.
// side_effects: none
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

pub use amadeus_ids::{AgentId, CommitId, SnapshotId, TeamId, TxId};
