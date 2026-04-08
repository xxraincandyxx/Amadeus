// @amadeus-header
// summary: Module root for the agent subsystem and its exports.
// layer: agent
// status: active
// feature_flags:
// - supervisor
// - team
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
#[cfg(feature = "team")]
pub mod manager; // NEW: Multi-agent manager
pub mod mesh;
pub mod messages;
pub mod profile; // NEW: Agent profiles
#[cfg(feature = "team")]
pub mod team;

#[cfg(feature = "supervisor")]
pub mod supervisor;

#[cfg(any(feature = "team", feature = "supervisor"))]
pub mod worker;

pub use compaction::{
    CompactionConfig, CompactionEvent, CompactionResult, CompressionStatus, ContextCompactor,
};
pub use config::{Config, Provider};
pub use events::{AgentEvent, ApprovalDecision, ApprovalRequest, RunResult, ToolCall};
pub use loop_agent::{Agent, SessionCheckpoint, SessionLog, SessionStats};
#[cfg(feature = "team")]
pub use manager::{AgentInfo, AgentManager, AgentStatus}; // NEW
pub use messages::{ContentBlock, Message};
pub use profile::AgentProfile; // NEW
#[cfg(feature = "team")]
pub use team::{AgentTeam, TeamLeader, TeamRegistry, TeamStatus, TeamTask, TeamTaskStatus};

#[cfg(feature = "supervisor")]
pub use supervisor::{DispatchStrategy, Supervisor, SupervisorConfig};

#[cfg(any(feature = "team", feature = "supervisor"))]
pub use worker::{Task, TaskResult, WorkerConfig, WorkerInfo, WorkerStatus};
