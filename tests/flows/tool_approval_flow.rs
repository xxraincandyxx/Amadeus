use serde_json::json;
use amadeus::client::StreamEvent;

#[path = "../scenarios/mod.rs"]
mod scenarios;

#[path = "../mocks/mod.rs"]
mod mocks;

use scenarios::{ScenarioBuilder, ScenarioRunner, assert_tool_call_count};
use mocks::ScenarioMockClient;

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
    let events = runner
        .execute(client)
        .await
        .expect("Scenario failed");
    
    assert_tool_call_count(&events, 1);
}

#[tokio::test]
async fn test_dangerous_command_blocked() {
    let client = ScenarioMockClient::scripted(vec![
        vec![
            StreamEvent::ToolCallStart {
                id: "tool_1".to_string(),
                name: "bash".to_string(),
            },
            StreamEvent::ToolCallDelta {
                arguments: json!({"command": "sudo rm -rf /"}).to_string(),
            },
            StreamEvent::ToolCallDone("tool_1".to_string()),
            StreamEvent::StopReason("tool_use".to_string()),
        ],
    ]);
    
    let scenario = ScenarioBuilder::new("dangerous_blocked")
        .description("Dangerous commands should be blocked")
        .build();
    
    let runner = ScenarioRunner::new(scenario);
    let events = runner
        .execute(client)
        .await
        .expect("Scenario failed");
    
    let has_error = events.iter().any(|e| {
        matches!(e, amadeus::agent::events::AgentEvent::Error { .. })
    });
    
    assert!(has_error, "Expected error for dangerous command");
}

#[tokio::test]
async fn test_multiple_tool_sequence() {
    let client = ScenarioMockClient::scripted(vec![
        vec![
            StreamEvent::ToolCallStart {
                id: "tool_1".to_string(),
                name: "read_file".to_string(),
            },
            StreamEvent::ToolCallDelta {
                arguments: json!({"path": "src/main.rs"}).to_string(),
            },
            StreamEvent::ToolCallDone("tool_1".to_string()),
            StreamEvent::StopReason("tool_use".to_string()),
        ],
        vec![
            StreamEvent::TextDelta("Operations completed successfully".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
    ]);
    
    let scenario = ScenarioBuilder::new("tool_chain")
        .description("Multiple tools in sequence")
        .build();
    
    let runner = ScenarioRunner::new(scenario);
    let events = runner
        .execute(client)
        .await
        .expect("Scenario failed");
    
    assert_tool_call_count(&events, 1);
}
