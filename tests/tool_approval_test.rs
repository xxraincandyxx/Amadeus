use amadeus::client::StreamEvent;
use serde_json::json;

#[path = "scenarios/mod.rs"]
mod scenarios;

#[path = "mocks/mod.rs"]
mod mocks;

use mocks::ScenarioMockClient;
use scenarios::{assert_tool_call_count, ScenarioBuilder, ScenarioRunner};

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
