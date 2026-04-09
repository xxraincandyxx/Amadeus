// @amadeus-header
// summary: Shared hook event and policy model types used across runtime surfaces.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - type: crate::HookSource
// - type: crate::HookAction
// - type: crate::HookEvent
// - type: crate::HookDescriptor
// uses:
// - format: JSON values
// invariants:
// - Hook event and policy model stay independent from runtime hook execution.
// side_effects: none
// tests:
// - cmd: cargo test -p hooks
// @end-amadeus-header

//! Shared hook event and policy model.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookSource {
    Global,
    Workspace,
    Runtime,
}

#[derive(Debug, Clone)]
pub enum HookAction {
    Continue,
    ModifyInput(Value),
    Block(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
}

impl HookEvent {
    pub fn title(&self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
        }
    }

    pub fn summary(&self) -> &'static str {
        match self {
            Self::PreToolUse => "Before tool execution",
            Self::PostToolUse => "After tool execution",
            Self::PostToolUseFailure => "After tool execution fails",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookDescriptor {
    pub name: String,
    pub event: HookEvent,
    pub command: String,
    pub tools: Vec<String>,
    pub source: HookSource,
}

#[cfg(test)]
mod tests {
    use super::HookEvent;

    #[test]
    fn hook_event_titles_are_stable() {
        assert_eq!(HookEvent::PreToolUse.title(), "PreToolUse");
        assert_eq!(HookEvent::PostToolUse.summary(), "After tool execution");
    }
}
