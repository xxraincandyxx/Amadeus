// @amadeus-header
// summary: Integration tests covering sub agent test behavior.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::sub_agent_test
// uses:
// - module: amadeus::agent::config::Config
// - module: amadeus::agent::loop_agent::Agent
// - module: amadeus::agent::messages::ContentBlock
// - module: amadeus::client::StreamEvent
// invariants:
// - Assertions stay aligned with current user-visible behavior.
// side_effects: none
// tests:
// - cmd: cargo test sub_agent_test --features full
// @end-amadeus-header

use std::sync::Arc;

use amadeus::agent::config::Config;
use amadeus::agent::loop_agent::Agent;
use amadeus::agent::messages::ContentBlock;
use amadeus::client::StreamEvent;

#[path = "mocks/mod.rs"]
mod mocks;

use mocks::ScenarioMockClient;

fn tool_names(tools: &[serde_json::Value]) -> Vec<String> {
    tools
        .iter()
        .filter_map(|tool| tool.get("name").and_then(|name| name.as_str()))
        .map(ToString::to_string)
        .collect()
}

#[tokio::test]
async fn test_sub_agent_uses_fresh_context_and_limited_child_tools() {
    let client = ScenarioMockClient::scripted(vec![
        vec![
            StreamEvent::ToolCallStart {
                id: "sub_1".to_string(),
                name: "sub_agent".to_string(),
            },
            StreamEvent::ToolCallDelta {
                arguments: serde_json::json!({
                    "prompt": "Inspect src/lib.rs and summarize the public API.",
                    "description": "API summary"
                })
                .to_string(),
            },
            StreamEvent::ToolCallDone("sub_1".to_string()),
            StreamEvent::StopReason("tool_use".to_string()),
        ],
        vec![
            StreamEvent::TextDelta("Child summary.".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
        vec![
            StreamEvent::TextDelta("Parent complete.".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
    ]);

    let config = Arc::new(Config {
        api_key: "test-key".to_string(),
        model: "test-model".to_string(),
        workdir: std::path::PathBuf::from("/tmp"),
        timeout_seconds: 10,
        ..Config::default()
    });

    let agent = Agent::builder(client.clone(), config)
        .with_default_tools()
        .build();

    let result = agent
        .run("Use a subagent to inspect the public API.")
        .await
        .expect("agent run should succeed");

    assert_eq!(result.text, "Parent complete.");
    assert_eq!(client.request_count(), 3);

    let parent_request = client.nth_request(0).expect("missing parent request");
    let child_request = client.nth_request(1).expect("missing child request");
    let resume_request = client
        .nth_request(2)
        .expect("missing parent resume request");

    assert!(tool_names(&parent_request.tools).contains(&"sub_agent".to_string()));

    let child_tool_names = tool_names(&child_request.tools);
    assert!(
        child_tool_names.len() >= 8,
        "expected focused child tool subset, got {:?}",
        child_tool_names
    );
    assert!(child_tool_names.contains(&"bash".to_string()));
    assert!(child_tool_names.contains(&"read_file".to_string()));
    assert!(child_tool_names.contains(&"write_file".to_string()));
    assert!(child_tool_names.contains(&"edit_file".to_string()));
    assert!(child_tool_names.contains(&"glob".to_string()));
    assert!(child_tool_names.contains(&"grep".to_string()));
    assert!(child_tool_names.contains(&"web_fetch".to_string()));
    assert!(!child_tool_names.contains(&"todo".to_string()));
    assert!(!child_tool_names.contains(&"sub_agent".to_string()));
    assert_eq!(child_request.messages.len(), 1);

    match &child_request.messages[0].content[0] {
        ContentBlock::Text { text } => {
            assert_eq!(text, "Inspect src/lib.rs and summarize the public API.");
        }
        block => panic!("expected child request text block, got {:?}", block),
    }

    assert!(resume_request.messages.iter().any(|message| {
        message.content.iter().any(|block| {
            matches!(
                block,
                ContentBlock::ToolResult { content, .. } if content == "Child summary."
            )
        })
    }));
}

#[tokio::test]
async fn test_sub_agent_stops_after_turn_limit() {
    let looping_steps = (0..31)
        .map(|idx| {
            vec![
                StreamEvent::ToolCallStart {
                    id: format!("tool_{}", idx),
                    name: "bash".to_string(),
                },
                StreamEvent::ToolCallDelta {
                    arguments: serde_json::json!({"command": "printf loop"}).to_string(),
                },
                StreamEvent::ToolCallDone(format!("tool_{}", idx)),
                StreamEvent::StopReason("tool_use".to_string()),
            ]
        })
        .collect::<Vec<_>>();

    let mut steps = vec![vec![
        StreamEvent::ToolCallStart {
            id: "sub_1".to_string(),
            name: "sub_agent".to_string(),
        },
        StreamEvent::ToolCallDelta {
            arguments: serde_json::json!({
                "prompt": "Loop forever with tool calls."
            })
            .to_string(),
        },
        StreamEvent::ToolCallDone("sub_1".to_string()),
        StreamEvent::StopReason("tool_use".to_string()),
    ]];
    steps.extend(looping_steps);
    steps.push(vec![
        StreamEvent::TextDelta("Parent observed the failure.".to_string()),
        StreamEvent::StopReason("end_turn".to_string()),
    ]);

    let client = ScenarioMockClient::scripted(steps);
    let config = Arc::new(Config {
        api_key: "test-key".to_string(),
        model: "test-model".to_string(),
        workdir: std::path::PathBuf::from("/tmp"),
        timeout_seconds: 10,
        ..Config::default()
    });

    let agent = Agent::builder(client.clone(), config)
        .with_default_tools()
        .build();

    let result = agent
        .run("Delegate an impossible looping task.")
        .await
        .expect("parent run should still complete");

    assert_eq!(result.text, "Parent observed the failure.");

    let resume_request = client
        .nth_request(31)
        .expect("missing resume request after subagent failure");
    assert!(resume_request.messages.iter().any(|message| {
        message.content.iter().any(|block| {
            matches!(
                block,
                ContentBlock::ToolResult { content, .. } if content.contains("Maximum turn limit (30) reached")
            )
        })
    }));
}
