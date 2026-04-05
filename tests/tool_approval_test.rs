// @amadeus-header
// summary: Integration tests covering tool approval test behavior.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::tool_approval_test
// uses:
// - module: amadeus::client::StreamEvent
// - format: JSON values
// invariants:
// - Assertions stay aligned with current user-visible behavior.
// side_effects: none
// tests:
// - cmd: cargo test tool_approval_test --features full
// @end-amadeus-header

use amadeus::client::StreamEvent;
use serde_json::json;

#[path = "scenarios/mod.rs"]
mod scenarios;

#[path = "mocks/mod.rs"]
mod mocks;

use mocks::ScenarioMockClient;
use scenarios::{
    assert_timeline_has_approval, assert_timeline_has_approval_decision, assert_tool_call_count,
    ScenarioBuilder, ScenarioRunner,
};

#[tokio::test]
async fn test_safe_tools_auto_approved() {
    let client = ScenarioMockClient::scripted(vec![
        vec![
            StreamEvent::ToolCallStart {
                id: "tool_1".to_string(),
                name: "read_file".to_string(),
            },
            StreamEvent::ToolCallDelta {
                arguments: json!({"path": "main.rs"}).to_string(),
            },
            StreamEvent::ToolCallDone("tool_1".to_string()),
            StreamEvent::StopReason("tool_use".to_string()),
        ],
        vec![
            StreamEvent::TextDelta("File read successfully".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
    ]);

    let scenario = ScenarioBuilder::new("auto_approve_safe")
        .description("Safe tools should be auto-approved")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let events = runner.execute(client).await.expect("Scenario failed");

    assert_tool_call_count(&events, 1);
}

#[tokio::test]
async fn test_dangerous_command_requires_approval() {
    let client = ScenarioMockClient::scripted(vec![
        vec![
            StreamEvent::ToolCallStart {
                id: "tool_1".to_string(),
                name: "bash".to_string(),
            },
            StreamEvent::ToolCallDelta {
                arguments: json!({"command": "rm -rf /"}).to_string(),
            },
            StreamEvent::ToolCallDone("tool_1".to_string()),
            StreamEvent::StopReason("tool_use".to_string()),
        ],
        vec![
            StreamEvent::TextDelta("Command was blocked.".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
    ]);

    let scenario = ScenarioBuilder::new("dangerous_command")
        .description("Dangerous commands should require approval")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let events = runner.execute(client).await.expect("Scenario failed");

    assert_tool_call_count(&events, 1);
}

#[tokio::test]
async fn test_scenario_runner_records_denied_approval_decision() {
    let client = ScenarioMockClient::scripted(vec![
        vec![
            StreamEvent::ToolCallStart {
                id: "tool_1".to_string(),
                name: "bash".to_string(),
            },
            StreamEvent::ToolCallDelta {
                arguments: json!({"command": "rm -rf /"}).to_string(),
            },
            StreamEvent::ToolCallDone("tool_1".to_string()),
            StreamEvent::StopReason("tool_use".to_string()),
        ],
        vec![
            StreamEvent::TextDelta("Blocked by approval flow.".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
    ]);

    let scenario = ScenarioBuilder::new("approval_decision_recorded")
        .description("Script an explicit deny decision")
        .user_says("Delete everything.")
        .approve_tool("bash", false)
        .build();

    let timeline = ScenarioRunner::new(scenario)
        .execute_timeline(client)
        .await
        .expect("Scenario failed");

    assert_timeline_has_approval(&timeline);
    assert_timeline_has_approval_decision(&timeline, "bash", false);
}
