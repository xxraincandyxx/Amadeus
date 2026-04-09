// @amadeus-header
// summary: Compatibility wrapper re-exporting agent event model types from the events crate.
// layer: agent
// status: active
// feature_flags: none
// provides:
// - module: crate::agent::events
// - type: crate::agent::events::RunResult
// - type: crate::agent::events::ToolCall
// - type: crate::agent::events::ApprovalDecision
// - type: crate::agent::events::ApprovalRequest
// - type: crate::agent::events::AgentEvent
// uses:
// - module: amadeus_events
// invariants:
// - Public event model paths remain stable while implementation lives outside core.
// side_effects: none
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

pub use amadeus_events::{AgentEvent, ApprovalDecision, ApprovalRequest, RunResult, ToolCall};
