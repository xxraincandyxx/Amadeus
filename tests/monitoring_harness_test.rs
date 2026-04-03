use amadeus::client::StreamEvent;
use serde_json::json;

#[path = "scenarios/mod.rs"]
mod scenarios;

#[path = "mocks/mod.rs"]
mod mocks;

use mocks::ScenarioMockClient;
use scenarios::{
    assert_timeline_event_labels, assert_timeline_has_thinking, assert_timeline_has_token_usage,
    assert_timeline_has_approval, assert_timeline_has_approval_decision,
    assert_timeline_history_len, assert_timeline_history_roles, assert_timeline_is_done,
    assert_timeline_text_contains, assert_timeline_thinking_contains, assert_timeline_tool_count,
    assert_timeline_tool_names, ScenarioBuilder, ScenarioRunner,
};

#[tokio::test]
async fn test_monitoring_timeline_exposes_request_and_history_details() {
    let client = ScenarioMockClient::scripted(vec![
        vec![
            StreamEvent::TokenUsage {
                input_tokens: 12,
                output_tokens: 4,
            },
            StreamEvent::ThinkingDelta("Need to inspect a file. ".to_string()),
            StreamEvent::TextDelta("Checking file... ".to_string()),
            StreamEvent::ToolCallStart {
                id: "tool_1".to_string(),
                name: "read_file".to_string(),
            },
            StreamEvent::ToolCallDelta {
                arguments: json!({"path": "Cargo.toml"}).to_string(),
            },
            StreamEvent::ToolCallDone("tool_1".to_string()),
            StreamEvent::StopReason("tool_use".to_string()),
        ],
        vec![
            StreamEvent::TokenUsage {
                input_tokens: 20,
                output_tokens: 8,
            },
            StreamEvent::TextDelta("Done reading.".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
    ]);

    let scenario = ScenarioBuilder::new("monitoring_request_capture")
        .description("Expose timeline, request capture, and final history")
        .user_says("Please inspect Cargo.toml and summarize it.")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let timeline = runner
        .execute_timeline(client.clone())
        .await
        .expect("timeline execution failed");

    assert_timeline_is_done(&timeline);
    assert_timeline_has_thinking(&timeline);
    assert_timeline_thinking_contains(&timeline, "inspect a file");
    assert_timeline_text_contains(&timeline, "Checking file...");
    assert_timeline_text_contains(&timeline, "Done reading.");
    assert_timeline_tool_count(&timeline, 1);
    assert_timeline_tool_names(&timeline, &["read_file"]);
    assert_timeline_has_token_usage(&timeline);
    assert_timeline_event_labels(
        &timeline,
        &[
            "token_usage",
            "thinking",
            "text",
            "tool_start:read_file",
            "tool_input",
            "tool_complete:read_file",
            "token_usage",
            "text",
            "done",
        ],
    );

    assert_eq!(timeline.total_tokens(), 44);
    assert_eq!(
        timeline.tool_input_for("tool_1"),
        json!({"path": "Cargo.toml"}).to_string()
    );
    assert_timeline_history_len(&timeline, 4);
    assert_timeline_history_roles(&timeline, &["user", "assistant", "user", "assistant"]);

    let requests = client.captured_requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].messages.len(), 1);
    assert_eq!(requests[0].messages[0].role, "user");
    assert_eq!(requests[0].messages[0].content.len(), 1);
    assert_eq!(requests[1].messages.len(), 3);
    assert_eq!(requests[0].max_tokens, 8000);
    assert!(!requests[0].tools.is_empty());
    assert!(requests[0].system.contains("You are"));
}

#[tokio::test]
async fn test_monitoring_timeline_exposes_missing_default_policy_monitoring_gap() {
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
            StreamEvent::TextDelta("I could not run that command.".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
    ]);

    let scenario = ScenarioBuilder::new("monitoring_approval")
        .description("Expose approval-required tool calls")
        .user_says("Delete the root filesystem.")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let timeline = runner
        .execute_timeline(client)
        .await
        .expect("timeline execution failed");

    assert!(
        timeline.has_approval_requests(),
        "default test harness policy did not emit approval"
    );
    assert_timeline_text_contains(&timeline, "I could not run that command.");

    let tool_errors = timeline.tool_errors();
    assert_eq!(tool_errors.len(), 1);
    assert!(tool_errors[0].output.contains("No such file or directory") || tool_errors[0].is_error);
}

#[tokio::test]
async fn scenario_runner_can_script_approval_decisions() {
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
            StreamEvent::TextDelta("Denied as expected.".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
    ]);

    let scenario = ScenarioBuilder::new("approval_roundtrip")
        .description("Drive approval decisions through the scenario runner")
        .user_says("Delete the root filesystem.")
        .approve_tool("bash", false)
        .build();

    let timeline = ScenarioRunner::new(scenario)
        .execute_timeline(client)
        .await
        .expect("timeline execution failed");

    assert_timeline_has_approval(&timeline);
    assert_timeline_has_approval_decision(&timeline, "bash", false);
}

#[tokio::test]
async fn scenario_runner_supports_fixture_backed_approval_scenarios() {
    let client = ScenarioMockClient::from_json(include_str!("fixtures/scenarios/approval_roundtrip.json"))
        .expect("fixture should deserialize");

    let scenario = ScenarioBuilder::new("approval_roundtrip_fixture")
        .description("Drive approval decisions with fixture-backed LLM output")
        .user_says("Delete the root filesystem.")
        .approve_tool("bash", false)
        .build();

    let timeline = ScenarioRunner::new(scenario)
        .execute_timeline(client)
        .await
        .expect("timeline execution failed");

    assert_timeline_has_approval(&timeline);
    assert_timeline_has_approval_decision(&timeline, "bash", false);
    assert_timeline_text_contains(&timeline, "Denied as expected.");
}
