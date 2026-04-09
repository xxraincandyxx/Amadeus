// @amadeus-header
// summary: Compatibility wrapper re-exporting team runtime types from the runtime crate.
// layer: agent
// status: active
// feature_flags:
// - team
// provides:
// - module: crate::agent::team
// - type: crate::agent::team::TeamLeader
// - type: crate::agent::team::TeamStatus
// - type: crate::agent::team::TeamTaskStatus
// - type: crate::agent::team::TeamTask
// - type: crate::agent::team::AgentTeam
// - type: crate::agent::team::TeamRegistry
// uses:
// - module: amadeus_runtime
// invariants:
// - Public team paths remain stable while implementation lives outside core.
// side_effects: none
// tests:
// - cmd: cargo test -p core team_registry_tracks_shared_tasks --features full
// @end-amadeus-header

//! Compatibility re-exports for team runtime types.

pub use amadeus_runtime::{
    AgentTeam, TeamLeader, TeamRegistry, TeamStatus, TeamTask, TeamTaskStatus,
};
