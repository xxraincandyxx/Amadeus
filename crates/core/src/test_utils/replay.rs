// @amadeus-header
// summary: Convert a recorded SessionLog into a replayable ScenarioDefinition.
// layer: test
// status: test-only
// feature_flags:
// - test-utils
// provides:
// - fn: crate::test_utils::replay::session_log_to_scenario
// uses:
// - type: crate::test_utils::testflow::types::SessionLog
// - type: crate::test_utils::scenario::ScenarioDefinition
// invariants:
// - One step per assistant response; thinking and tool_use are preserved.
// side_effects: none
// tests:
// - cmd: cargo test -p core --features test-utils replay
// @end-amadeus-header

//! Reconstruct a scripted scenario from a recorded session timeline.

use crate::test_utils::scenario::{ScenarioDefinition, ScenarioStepDef, StreamEventDef};
use crate::test_utils::testflow::types::{AgentEventData, RecordedEvent, SessionLog};

/// Convert a recorded session into a replayable scenario.
///
/// Grouping: accumulate `AgentEvent`s into the current step. `ToolComplete`
/// closes the current step (stop reason `tool_use`) because the next agent
/// response is a fresh LLM call. `Done` closes with `end_turn`; `Error` closes
/// with a step-level error.
pub fn session_log_to_scenario(log: &SessionLog) -> ScenarioDefinition {
    let mut steps: Vec<ScenarioStepDef> = Vec::new();
    let mut current = ScenarioStepDef { delay_ms: None, events: Vec::new(), error: None };
    let mut started = false;

    let close = |step: &ScenarioStepDef, reason: &str| -> ScenarioStepDef {
        let mut s = ScenarioStepDef {
            delay_ms: None,
            events: step.events.clone(),
            error: step.error.clone(),
        };
        s.events.push(StreamEventDef::StopReason { reason: reason.to_string() });
        s
    };

    for event in &log.timeline {
        let RecordedEvent::AgentEvent { event } = &event.event_type else { continue };
        started = true;
        match event {
            AgentEventData::TextDelta { delta } => {
                current.events.push(StreamEventDef::TextDelta { text: delta.clone() });
            }
            AgentEventData::ThinkingDelta { delta } => {
                current.events.push(StreamEventDef::ThinkingDelta { text: delta.clone() });
            }
            AgentEventData::ThinkingComplete { thinking } => {
                current.events.push(StreamEventDef::ThinkingDelta { text: thinking.clone() });
            }
            AgentEventData::ToolStart { id, name, .. } => {
                current.events.push(StreamEventDef::ToolCallStart { id: id.clone(), name: name.clone() });
            }
            AgentEventData::ToolInputDelta { delta, .. } => {
                current.events.push(StreamEventDef::ToolCallDelta { arguments: delta.clone() });
            }
            AgentEventData::ToolComplete { id, input, .. } => {
                current.events.push(StreamEventDef::ToolCallDelta { arguments: input.to_string() });
                current.events.push(StreamEventDef::ToolCallDone { id: id.clone() });
                steps.push(close(&current, "tool_use"));
                current = ScenarioStepDef { delay_ms: None, events: Vec::new(), error: None };
            }
            AgentEventData::TokenUsage { input_tokens, output_tokens, .. } => {
                current.events.push(StreamEventDef::TokenUsage {
                    input_tokens: *input_tokens,
                    output_tokens: *output_tokens,
                });
            }
            AgentEventData::Done { .. } => {
                steps.push(close(&current, "end_turn"));
                current = ScenarioStepDef { delay_ms: None, events: Vec::new(), error: None };
            }
            AgentEventData::Error { message } => {
                current.error = Some(message.clone());
                steps.push(current.clone());
                current = ScenarioStepDef { delay_ms: None, events: Vec::new(), error: None };
            }
            _ => {}
        }
    }

    if started && (!current.events.is_empty() || current.error.is_some()) {
        steps.push(close(&current, "end_turn"));
    }

    ScenarioDefinition {
        name: log.metadata.session_id.clone(),
        description: format!("Converted from session {}", log.metadata.session_id),
        steps,
    }
}

#[cfg(test)]
mod tests {
    use super::session_log_to_scenario;
    use crate::test_utils::scenario::{ScenarioDefinition, StreamEventDef};
    use crate::test_utils::testflow::types::{
        AgentEventData, RecordedEvent, SessionLog, SessionMetadata, TimelineEvent,
    };

    fn agent(ev: AgentEventData) -> TimelineEvent {
        TimelineEvent {
            seq: 0,
            timestamp_ms: 0,
            event_type: RecordedEvent::AgentEvent { event: ev },
        }
    }

    fn empty_log() -> SessionLog {
        SessionLog {
            version: "1".to_string(),
            metadata: SessionMetadata { session_id: "sess_x".to_string(), ..Default::default() },
            timeline: Vec::new(),
            summaries: Default::default(),
            snapshots: Default::default(),
        }
    }

    #[test]
    fn single_text_turn_becomes_one_step() {
        let mut log = empty_log();
        log.timeline.push(agent(AgentEventData::TextDelta { delta: "Hi".to_string() }));
        log.timeline.push(agent(AgentEventData::Done { text: "Hi".to_string(), tool_call_count: 0 }));
        let def = session_log_to_scenario(&log);
        assert_eq!(def.steps.len(), 1);
        assert!(def.steps[0].events.iter().any(|e| matches!(e, StreamEventDef::TextDelta { text } if text == "Hi")));
        assert!(def.steps[0].events.iter().any(|e| matches!(e, StreamEventDef::StopReason { reason } if reason == "end_turn")));
    }

    #[test]
    fn tool_turn_splits_into_two_steps() {
        let mut log = empty_log();
        log.timeline.push(agent(AgentEventData::TextDelta { delta: "running".to_string() }));
        log.timeline.push(agent(AgentEventData::ToolStart {
            id: "t1".to_string(),
            name: "bash".to_string(),
            command: None,
            parent_id: None,
        }));
        log.timeline.push(agent(AgentEventData::ToolComplete {
            id: "t1".to_string(),
            name: "bash".to_string(),
            input: serde_json::json!({"cmd":"ls"}),
            output: "out".to_string(),
            is_error: false,
            parent_id: None,
        }));
        log.timeline.push(agent(AgentEventData::TextDelta { delta: "done".to_string() }));
        log.timeline.push(agent(AgentEventData::Done { text: "done".to_string(), tool_call_count: 1 }));
        let def = session_log_to_scenario(&log);
        assert_eq!(def.steps.len(), 2, "tool_use turn then follow-up");
        assert!(def.steps[0].events.iter().any(|e| matches!(e, StreamEventDef::ToolCallStart { name, .. } if name == "bash")));
    }

    #[test]
    fn thinking_is_preserved() {
        let mut log = empty_log();
        log.timeline.push(agent(AgentEventData::ThinkingDelta { delta: "hmm".to_string() }));
        log.timeline.push(agent(AgentEventData::TextDelta { delta: "ok".to_string() }));
        log.timeline.push(agent(AgentEventData::Done { text: "ok".to_string(), tool_call_count: 0 }));
        let def = session_log_to_scenario(&log);
        assert!(def.steps[0].events.iter().any(|e| matches!(e, StreamEventDef::ThinkingDelta { text } if text == "hmm")));
    }

    /// Regression guard: the on-disk `session_*.json` fixture produced by the
    /// recorder must (a) deserialize via `load_session`, (b) convert cleanly
    /// into a `ScenarioDefinition`, and (c) round-trip back through
    /// `serde_json` so the `convert_session` CLI example's stdout is consumable
    /// by `ScenarioMockClient::from_json`. Without this, the convert -> replay
    /// pipeline silently breaks the moment the recorder's JSON schema drifts.
    #[test]
    fn on_disk_fixture_round_trips_through_convert_session() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        // CARGO_MANIFEST_DIR for this crate is <repo>/crates/core, so the
        // workspace-level fixture lives two levels up.
        let fixture = manifest_dir.join("../../tests/testflow/fixtures/sample_session.json");
        let log = crate::test_utils::testflow::recorder::load_session(&fixture)
            .expect("sample_session.json must load");
        let def = session_log_to_scenario(&log);
        assert!(!def.steps.is_empty(), "fixture should yield at least one step");

        // Round-trip through JSON, which is what the CLI example produces and
        // what ScenarioMockClient::from_json consumes.
        let json = serde_json::to_string(&def).expect("serialize scenario");
        let parsed: ScenarioDefinition =
            serde_json::from_str(&json).expect("scenario JSON round-trips");
        assert_eq!(parsed.steps.len(), def.steps.len());
    }
}
