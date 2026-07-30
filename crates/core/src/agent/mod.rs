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
pub mod llm_trace;
pub mod loop_agent;
pub mod messages;
#[cfg(feature = "orchestra")]
pub mod orchestra;
pub mod profile;

#[cfg(feature = "orchestra")]
pub mod worker;

pub use compaction::{
    CompactionConfig, CompactionEvent, CompactionResult, CompressionStatus, ContextCompactor,
};
pub use config::{
    Config, PromptMergeMode, PromptProfileConfig, PromptSectionConfig, PromptSettings, Provider,
    ToolOverrideConfig, ToolProfileConfig, ToolSettings,
};
pub use events::{AgentEvent, ApprovalDecision, ApprovalRequest, RunResult, ToolCall};
pub use llm_trace::{LlmTraceRequest, LlmTraceResponse, LlmTraceSink, LlmTraceToolCall};
pub use loop_agent::{Agent, SessionCheckpoint, SessionLog, SessionStats};
pub use messages::{ContentBlock, Message};
#[cfg(feature = "orchestra")]
pub use orchestra::{
    AgentInfo, AgentOrchestra, AgentOrchestrator, AgentStatus, ArtifactRecord, MailboxEvent,
    MailboxEventKind, OrchestraConfig, OrchestraLeader, OrchestraRegistry, OrchestraRuntime,
    OrchestraStatus, OrchestraStrategy, OrchestraTask, OrchestraTaskStatus,
};
pub use profile::AgentProfile;

#[cfg(feature = "orchestra")]
pub use worker::{Task, TaskResult, WorkerConfig, WorkerInfo, WorkerStatus};
