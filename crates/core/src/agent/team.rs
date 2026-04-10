// @amadeus-header
// summary: Deprecated team shim forwarding to orchestra-compatible runtime types.
// layer: agent
// status: deprecated
// feature_flags:
// - team
// provides:
// - module: crate::agent::team
// - type: crate::agent::team::ArtifactRecord
// - type: crate::agent::team::MailboxEvent
// - type: crate::agent::team::MailboxEventKind
// - type: crate::agent::team::TeamLeader
// - type: crate::agent::team::TeamStatus
// - type: crate::agent::team::TeamTaskStatus
// - type: crate::agent::team::TeamTask
// - type: crate::agent::team::AgentTeam
// - type: crate::agent::team::TeamRegistry
// uses:
// - module: amadeus_runtime
// invariants:
// - Legacy team paths remain available as deprecated aliases over orchestra-compatible runtime types.
// side_effects: none
// tests:
// - cmd: cargo test -p core team_registry_tracks_shared_tasks --features full
// @end-amadeus-header

//! Deprecated compatibility re-exports for legacy team runtime types.

#[deprecated(
    note = "use crate::agent::orchestra::{AgentOrchestra, OrchestraLeader, OrchestraRegistry, OrchestraStatus, OrchestraTask, OrchestraTaskStatus}"
)]
pub use amadeus_runtime::{
    AgentTeam, ArtifactRecord, MailboxEvent, MailboxEventKind, TeamLeader, TeamRegistry,
    TeamStatus, TeamTask, TeamTaskStatus,
};
