// @amadeus-header
// summary: Shared security primitives for path resolution and command execution boundaries.
// layer: policy
// status: active
// feature_flags: none
// provides:
// - module: crate::security
// - type: crate::security::PathPolicy
// - type: crate::security::CommandRunner
// - type: crate::security::CommandRequest
// - type: crate::security::CommandResult
// - type: crate::security::SandboxProfile
// uses:
// - module: crate::error
// - module: crate::permissions
// - runtime: tokio async runtime
// - artifact: filesystem paths and files
// invariants:
// - Runtime enforcement and permission classification share the same path policy.
// side_effects:
// - May run external commands or subprocesses.
// tests:
// - cmd: cargo test -p core security --features full
// @end-amadeus-header

//! Shared security primitives for tool execution.

pub mod command_runner;
pub mod path_policy;

pub use command_runner::{CommandRequest, CommandResult, CommandRunner, SandboxProfile};
pub use path_policy::PathPolicy;
