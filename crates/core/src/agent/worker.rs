// @amadeus-header
// summary: Compatibility wrapper re-exporting worker runtime types from the runtime crate.
// layer: agent
// status: active
// feature_flags: none
// provides:
// - module: crate::agent::worker
// - type: crate::agent::worker::WorkerConfig
// - type: crate::agent::worker::Task
// - type: crate::agent::worker::HelpRequest
// - type: crate::agent::worker::TaskResult
// - type: crate::agent::worker::WorkerStatus
// - type: crate::agent::worker::WorkerInfo
// - type: crate::agent::worker::RunOutcome
// - function: crate::agent::worker::mark_worker_task_started
// - function: crate::agent::worker::finalize_worker_task
// uses:
// - module: amadeus_runtime
// invariants:
// - Public worker paths remain stable while implementation lives outside core.
// side_effects: none
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

//! Compatibility re-exports for worker runtime types.

pub use amadeus_runtime::{
    finalize_worker_task, mark_worker_task_started, HelpRequest, RunOutcome, Task, TaskResult,
    WorkerConfig, WorkerInfo, WorkerStatus,
};
