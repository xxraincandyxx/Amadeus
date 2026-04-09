// @amadeus-header
// summary: Canonical orchestra surface unifying orchestration naming across core runtime APIs.
// layer: agent
// status: active
// feature_flags:
// - orchestra
// provides:
// - module: crate::agent::orchestra
// - type: crate::agent::orchestra::AgentInfo
// - type: crate::agent::orchestra::AgentStatus
// - type: crate::agent::orchestra::AgentOrchestrator
// - type: crate::agent::orchestra::OrchestraRuntime
// - type: crate::agent::orchestra::OrchestraLeader
// - type: crate::agent::orchestra::AgentOrchestra
// - type: crate::agent::orchestra::OrchestraRegistry
// - type: crate::agent::orchestra::OrchestraConfig
// - type: crate::agent::orchestra::OrchestraStrategy
// - type: crate::agent::orchestra::Task
// - type: crate::agent::orchestra::TaskResult
// - type: crate::agent::orchestra::WorkerConfig
// - type: crate::agent::orchestra::WorkerInfo
// - type: crate::agent::orchestra::WorkerStatus
// uses:
// - module: crate::agent::manager
// - module: crate::agent::supervisor
// - module: crate::agent::worker
// - module: amadeus_runtime::orchestra
// invariants:
// - Orchestra naming remains the primary public surface while legacy modules stay deprecated.
// side_effects: none
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

pub use super::manager::{AgentInfo, AgentStatus};
pub use super::worker::{Task, TaskResult, WorkerConfig, WorkerInfo, WorkerStatus};
pub use amadeus_runtime::{
    AgentOrchestra, OrchestraConfig, OrchestraLeader, OrchestraRegistry, OrchestraStatus,
    OrchestraStrategy, OrchestraTask, OrchestraTaskStatus,
};

pub type AgentOrchestrator<C> = super::manager::AgentManager<C>;
pub type OrchestraRuntime<C> = super::supervisor::Supervisor<C>;
