// @amadeus-header
// summary: Core SDK crate root exposing agent, tool, client, and runtime modules.
// layer: core
// status: active
// feature_flags:
// - api
// - concurrency
// - context
// - mesh
// - supervisor
// - test-utils
// - tui
// provides:
// - module: crate
// uses: none
// invariants:
// - Core modules remain frontend-agnostic and reusable across transports.
// side_effects: none
// tests:
// - cmd: cargo test -p amadeus-core --features full
// @end-amadeus-header

//! Core Amadeus SDK runtime and reusable agent infrastructure.

pub mod agent;
pub mod benchmark;
pub mod client;
#[cfg(feature = "concurrency")]
pub mod concurrency;
pub mod context;
pub mod core;
pub mod error;
pub mod hooks;
pub mod mcp;
pub mod policy;
pub mod prompts;
pub mod skills;
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;
pub mod tools;

pub use error::{AgentError, Result};

#[cfg(feature = "concurrency")]
pub use concurrency::{
    FileLockManager, FileLockStats, FileReadGuard, FileReadInfo, FileWriteGuard, LockEntry,
    LockError, LockManager, LockMode, LockStatus,
};

#[cfg(feature = "supervisor")]
pub use agent::{
    DispatchStrategy, Supervisor, SupervisorConfig, Task, TaskResult, WorkerConfig, WorkerInfo,
    WorkerStatus,
};
