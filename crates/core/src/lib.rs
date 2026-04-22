// @amadeus-header
// summary: Core SDK crate root exposing agent, tool, client, and runtime modules.
// layer: core
// status: active
// feature_flags:
// - api
// - concurrency
// - context
// - orchestra
// - test-utils
// - tui
// provides:
// - module: crate
// uses: none
// invariants:
// - Core modules remain frontend-agnostic and reusable across transports.
// side_effects: none
// tests:
// - cmd: cargo test -p core --features full
// @end-amadeus-header

//! Core Amadeus SDK runtime and reusable agent infrastructure.

pub mod agent;
pub mod assessment;
pub mod benchmark;
pub mod bridge;
pub mod client;
pub mod commands;
#[cfg(feature = "concurrency")]
pub mod concurrency;
pub mod context;
pub mod core;
pub mod error;
pub mod hooks;
pub mod mcp;
pub mod permissions;
pub mod policy;
pub mod prompts;
pub mod skills;
pub mod telemetry;
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;
pub mod tools;

pub use assessment::{
    default_prompt as default_assessment_prompt, AssessmentConfig, AssessmentResult,
    AssessmentRunner, ScriptedAssessmentClient,
};
pub use commands::{
    answer_side_question,
    apply_citation_candidate, build_context_report, filter_citation_candidates,
    find_active_citation_query, format_citation_markdown, normalize_pasted_path,
    parse_render_spans, scan_workspace_citation_candidates, ActiveCitationQuery,
    CitationApplyResult, CitationCandidate, CitationRenderSpan, ContextEntry, ContextReport,
    ContextSection, ContextSectionGroup, SideQuestionOptions, SlashCommand, SlashCommandSpec,
    SLASH_COMMAND_SPECS,
};
pub use error::{AgentError, Result};
pub use permissions::{PermissionDecision, PermissionEnforcer, PermissionMode};
pub use telemetry::{
    JsonlSink, MemorySink, TelemetryEntry, TelemetryError, TelemetryEvent, TelemetryRecorder,
    TelemetrySink,
};
pub use tools::{
    ComposedToolCatalog, ToolCatalogView, ToolExecutionResult, ToolExecutor, ToolLevel, ToolPack,
    ToolPolicy, ToolProfile, ToolRegistration, ToolSource, ToolSpec,
};

#[cfg(feature = "concurrency")]
pub use concurrency::{
    FileLockManager, FileLockStats, FileReadGuard, FileReadInfo, FileWriteGuard, LockEntry,
    LockError, LockManager, LockMode, LockStatus,
};

#[cfg(feature = "orchestra")]
pub use agent::{
    AgentInfo, AgentOrchestra, AgentOrchestrator, AgentStatus, ArtifactRecord, MailboxEvent,
    MailboxEventKind, OrchestraConfig, OrchestraLeader, OrchestraRegistry, OrchestraRuntime,
    OrchestraStatus, OrchestraStrategy, OrchestraTask, OrchestraTaskStatus, Task, TaskResult,
    WorkerConfig, WorkerInfo, WorkerStatus,
};

#[cfg(feature = "orchestra")]
#[deprecated(note = "use AgentOrchestrator and orchestra::* types instead")]
#[allow(deprecated)]
pub use agent::{
    AgentInfo as LegacyAgentInfo, AgentManager, AgentStatus as LegacyAgentStatus, AgentTeam,
    TeamLeader, TeamRegistry, TeamStatus, TeamTask, TeamTaskStatus,
};

#[cfg(feature = "orchestra")]
#[deprecated(note = "use OrchestraRuntime and orchestra::* types instead")]
#[allow(deprecated)]
pub use agent::{DispatchStrategy, Supervisor, SupervisorConfig};
