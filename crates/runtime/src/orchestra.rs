// @amadeus-header
// summary: Unified orchestra naming surface for multi-agent coordination runtime types.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::orchestra
// - type: crate::orchestra::OrchestraLeader
// - type: crate::orchestra::OrchestraStatus
// - type: crate::orchestra::OrchestraTaskStatus
// - type: crate::orchestra::OrchestraTask
// - type: crate::orchestra::AgentOrchestra
// - type: crate::orchestra::OrchestraRegistry
// - type: crate::orchestra::OrchestraStrategy
// - type: crate::orchestra::OrchestraConfig
// uses:
// - module: crate::scheduler
// - module: crate::team
// invariants:
// - Orchestra aliases remain the canonical naming surface over the shared coordination types.
// side_effects: none
// tests:
// - cmd: cargo test -p runtime
// @end-amadeus-header

pub type OrchestraLeader = crate::team::TeamLeader;
pub type OrchestraStatus = crate::team::TeamStatus;
pub type OrchestraTaskStatus = crate::team::TeamTaskStatus;
pub type OrchestraTask = crate::team::TeamTask;
pub type AgentOrchestra = crate::team::AgentTeam;
pub type OrchestraRegistry = crate::team::TeamRegistry;
pub type OrchestraStrategy = crate::scheduler::DispatchStrategy;
pub type OrchestraConfig = crate::scheduler::SupervisorConfig;
