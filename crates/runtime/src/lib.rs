// @amadeus-header
// summary: Runtime coordination primitives reused across core transport surfaces.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
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

pub use scheduler::{select_worker, DispatchStrategy, SupervisorConfig};
pub use team::{AgentTeam, TeamLeader, TeamRegistry, TeamStatus, TeamTask, TeamTaskStatus};
pub use worker::{HelpRequest, Task, TaskResult, WorkerConfig, WorkerInfo, WorkerStatus};
