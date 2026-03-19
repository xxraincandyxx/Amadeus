//! Agent system for the SDK

pub mod compaction;
pub mod config;
pub mod events;
pub mod loop_agent;
pub mod mesh;
pub mod messages;

#[cfg(feature = "supervisor")]
pub mod supervisor;

#[cfg(feature = "supervisor")]
pub mod worker;

pub use compaction::{
    CompactionConfig, CompactionEvent, CompactionResult, CompressionStatus, ContextCompactor,
};
pub use config::{Config, Provider};
pub use events::{AgentEvent, ApprovalDecision, ApprovalRequest, RunResult, ToolCall};
pub use loop_agent::{Agent, SessionLog, SessionStats};
pub use messages::{ContentBlock, Message};

#[cfg(feature = "supervisor")]
pub use supervisor::{DispatchStrategy, Supervisor, SupervisorConfig};

#[cfg(feature = "supervisor")]
pub use worker::{Task, TaskResult, WorkerConfig, WorkerInfo, WorkerStatus};
