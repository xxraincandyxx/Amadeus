// @amadeus-header
// summary: Replay scenario data types shared by the mock client and the session converter.
// layer: test
// status: test-only
// feature_flags:
// - test-utils
// provides:
// - module: crate::test_utils::scenario
// - type: crate::test_utils::scenario::ScenarioDefinition
// - type: crate::test_utils::scenario::ScenarioStepDef
// - type: crate::test_utils::scenario::StreamEventDef
// uses:
// - module: crate::client
// - protocol: serde serialization
// invariants:
// - JSON wire format stays identical to the historical tests/mocks/scenario_client.rs layout.
// side_effects: none
// tests:
// - cmd: cargo test -p core --features test-utils scenario
// @end-amadeus-header

//! Data-only scenario types. `ScenarioMockClient` (which implements `LLMClient`)
//! remains in `tests/mocks/scenario_client.rs` and re-exports these.

use serde::{Deserialize, Serialize};

use crate::client::StreamEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioDefinition {
    pub name: String,
    pub description: String,
    pub steps: Vec<ScenarioStepDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioStepDef {
    pub delay_ms: Option<u64>,
    pub events: Vec<StreamEventDef>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEventDef {
    TextDelta { text: String },
    ThinkingDelta { text: String },
    ToolCallStart { id: String, name: String },
    ToolCallDelta { arguments: String },
    ToolCallDone { id: String },
    StopReason { reason: String },
    TokenUsage { input_tokens: u32, output_tokens: u32 },
}

impl From<StreamEventDef> for StreamEvent {
    fn from(def: StreamEventDef) -> Self {
        match def {
            StreamEventDef::TextDelta { text } => StreamEvent::TextDelta(text),
            StreamEventDef::ThinkingDelta { text } => StreamEvent::ThinkingDelta(text),
            StreamEventDef::ToolCallStart { id, name } => StreamEvent::ToolCallStart { id, name },
            StreamEventDef::ToolCallDelta { arguments } => StreamEvent::ToolCallDelta { arguments },
            StreamEventDef::ToolCallDone { id } => StreamEvent::ToolCallDone(id),
            StreamEventDef::StopReason { reason } => StreamEvent::StopReason(reason),
            StreamEventDef::TokenUsage { input_tokens, output_tokens } => StreamEvent::TokenUsage {
                input_tokens,
                output_tokens,
            },
        }
    }
}

impl From<StreamEvent> for StreamEventDef {
    fn from(event: StreamEvent) -> Self {
        match event {
            StreamEvent::TextDelta(text) => StreamEventDef::TextDelta { text },
            StreamEvent::ThinkingDelta(text) => StreamEventDef::ThinkingDelta { text },
            StreamEvent::ToolCallStart { id, name } => StreamEventDef::ToolCallStart { id, name },
            StreamEvent::ToolCallDelta { arguments } => StreamEventDef::ToolCallDelta { arguments },
            StreamEvent::ToolCallDone(id) => StreamEventDef::ToolCallDone { id },
            StreamEvent::StopReason(reason) => StreamEventDef::StopReason { reason },
            StreamEvent::TokenUsage { input_tokens, output_tokens } => {
                StreamEventDef::TokenUsage { input_tokens, output_tokens }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_definition_round_trips() {
        let def = ScenarioDefinition {
            name: "t".to_string(),
            description: "d".to_string(),
            steps: vec![ScenarioStepDef {
                delay_ms: None,
                events: vec![
                    StreamEventDef::TextDelta { text: "Hi".to_string() },
                    StreamEventDef::StopReason { reason: "end_turn".to_string() },
                ],
                error: None,
            }],
        };
        let json = serde_json::to_string(&def).unwrap();
        let back: ScenarioDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(back.steps.len(), 1);
        assert_eq!(back.steps[0].events.len(), 2);
    }

    #[test]
    fn wire_format_uses_snake_case_type_tag() {
        let json = r#"{"type":"text_delta","text":"x"}"#;
        let ev: StreamEventDef = serde_json::from_str(json).unwrap();
        assert!(matches!(ev, StreamEventDef::TextDelta { text } if text == "x"));
    }
}
