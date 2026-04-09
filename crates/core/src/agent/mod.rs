// @amadeus-header
// summary: Module root for the agent subsystem and its exports.
// layer: agent
// status: active
// feature_flags:
// - orchestra
// provides:
// - module: crate::agent
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - tests/mod.rs
// @end-amadeus-header

//! Agent system for the SDK

pub mod compaction;
pub mod config;
pub mod events;
pub mod loop_agent;
#[cfg(feature = "orchestra")]
#[deprecated(note = "use crate::agent::orchestra::AgentOrchestrator")]
pub mod manager;
pub mod messages;
#[cfg(feature = "orchestra")]
pub mod orchestra;
pub mod profile; // NEW: Agent profiles
#[cfg(feature = "orchestra")]
#[deprecated(
    note = "use crate::agent::orchestra::{AgentOrchestra, OrchestraLeader, OrchestraRegistry, OrchestraStatus, OrchestraTask, OrchestraTaskStatus}"
)]
pub mod team;

#[cfg(feature = "orchestra")]
#[deprecated(note = "use crate::agent::orchestra::OrchestraRuntime")]
pub mod supervisor;

#[cfg(feature = "orchestra")]
pub mod worker;

pub use compaction::{
    CompactionConfig, CompactionEvent, CompactionResult, CompressionStatus, ContextCompactor,
};
pub use config::{Config, Provider};
pub use events::{AgentEvent, ApprovalDecision, ApprovalRequest, RunResult, ToolCall};
pub use loop_agent::{Agent, SessionCheckpoint, SessionLog, SessionStats};
#[cfg(feature = "orchestra")]
#[deprecated(
    note = "use crate::agent::orchestra::{AgentInfo, AgentOrchestrator, AgentStatus}"
)]
#[allow(deprecated)]
pub use manager::AgentManager;
pub use messages::{ContentBlock, Message};
#[cfg(feature = "orchestra")]
pub use orchestra::{
    AgentInfo, AgentOrchestra, AgentOrchestrator, AgentStatus, OrchestraConfig, OrchestraLeader,
    OrchestraRegistry, OrchestraRuntime, OrchestraStatus, OrchestraStrategy, OrchestraTask,
    OrchestraTaskStatus,
};
pub use profile::AgentProfile; // NEW
#[cfg(feature = "orchestra")]
#[deprecated(
    note = "use crate::agent::orchestra::{AgentOrchestra, OrchestraLeader, OrchestraRegistry, OrchestraStatus, OrchestraTask, OrchestraTaskStatus}"
)]
pub use team::{AgentTeam, TeamLeader, TeamRegistry, TeamStatus, TeamTask, TeamTaskStatus};

#[cfg(feature = "orchestra")]
#[deprecated(
    note = "use crate::agent::orchestra::{OrchestraRuntime, OrchestraStrategy, OrchestraConfig}"
)]
#[allow(deprecated)]
pub use supervisor::{DispatchStrategy, Supervisor, SupervisorConfig};

#[cfg(feature = "orchestra")]
pub use worker::{Task, TaskResult, WorkerConfig, WorkerInfo, WorkerStatus};
