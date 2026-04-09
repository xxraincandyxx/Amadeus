// @amadeus-header
// summary: Runtime coordination primitives reused across core transport surfaces.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - module: crate::agent
// - module: crate::orchestra
// - module: crate::scheduler
// - module: crate::team
// - module: crate::worker
// - type: crate::RuntimeError
// - type: crate::Result
// uses:
// - module: amadeus_events
// - module: amadeus_ids
// - runtime: tokio sync primitives
// invariants:
// - Runtime coordination types stay transport-agnostic and reusable across frontends.
// side_effects: none
// tests:
// - cmd: cargo test -p runtime
// @end-amadeus-header

//! Runtime coordination primitives for Amadeus.

pub mod agent;
pub mod orchestra;
pub mod scheduler;
pub mod team;
pub mod worker;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("Command execution failed: {0}")]
    Command(String),
}

pub type Result<T> = std::result::Result<T, RuntimeError>;

pub use agent::{
    find_agent_index, get_agent_info, list_agent_info, next_agent_index,
    normalize_active_index_after_removal, previous_agent_index, select_agent, AgentInfo,
    AgentRouteCandidate, AgentStatus,
};
pub use orchestra::{
    AgentOrchestra, OrchestraConfig, OrchestraLeader, OrchestraRegistry, OrchestraStatus,
    OrchestraStrategy, OrchestraTask, OrchestraTaskStatus,
};
pub use scheduler::{select_worker, DispatchStrategy, SupervisorConfig};
pub use team::{AgentTeam, TeamLeader, TeamRegistry, TeamStatus, TeamTask, TeamTaskStatus};
pub use worker::{
    finalize_worker_task, mark_worker_task_started, HelpRequest, RunOutcome, Task, TaskResult,
    WorkerConfig, WorkerInfo, WorkerStatus,
};
