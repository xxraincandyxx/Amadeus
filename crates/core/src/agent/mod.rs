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
pub mod manager; // NEW: Multi-agent manager
pub mod messages;
#[cfg(feature = "orchestra")]
pub mod orchestra;
pub mod profile; // NEW: Agent profiles
#[cfg(feature = "orchestra")]
pub mod team;

#[cfg(feature = "orchestra")]
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
pub use manager::{AgentInfo, AgentManager, AgentStatus}; // NEW
pub use messages::{ContentBlock, Message};
#[cfg(feature = "orchestra")]
pub use orchestra::{
    AgentOrchestra, AgentOrchestrator, OrchestraConfig, OrchestraLeader, OrchestraRegistry,
    OrchestraRuntime, OrchestraStatus, OrchestraStrategy, OrchestraTask, OrchestraTaskStatus,
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
pub use supervisor::{DispatchStrategy, Supervisor, SupervisorConfig};

#[cfg(feature = "orchestra")]
pub use worker::{Task, TaskResult, WorkerConfig, WorkerInfo, WorkerStatus};
