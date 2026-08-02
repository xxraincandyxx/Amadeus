// @amadeus-header
// summary: Golden acceptance workflow covering deterministic prompts, streaming, tools, and artifact verification.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::golden_workflow_test
// uses:
// - module: tests::mocks::scenario_client
// - module: amadeus::agent
// - module: amadeus::agent::config
// - runtime: tokio async runtime
// - artifact: temporary Rust source and test binary
// invariants:
// - The scripted LLM and prompt profile remain deterministic.
// - Tool calls execute in inspect, edit, verify order.
// side_effects:
// - Writes files and executes rustc inside a temporary workspace.
// tests:
// - cmd: cargo test --test golden_workflow_test --features full
// @end-amadeus-header

#[path = "mocks/mod.rs"]
mod mocks;

use std::collections::HashMap;
use std::sync::Arc;

use amadeus::agent::config::{
    Config, PromptMergeMode, PromptProfileConfig, PromptSectionConfig, PromptSettings,
};
use amadeus::agent::events::AgentEvent;
use amadeus::agent::loop_agent::Agent;
use amadeus::agent::messages::Message;
use futures::StreamExt;
use tempfile::tempdir;

use mocks::ScenarioMockClient;

const USER_PROMPT: &str = "A calculator regression is failing. Diagnose the root cause, make the smallest safe fix, run the focused verification, and return a structured incident report.";
const WORKFLOW_PROMPT: &str = "## Golden incident workflow\n\nFollow this exact sequence: inspect evidence, state the root cause, make the smallest change, run focused verification, then report root cause, change, verification, and risk. Do not claim success without tool evidence.";

#[tokio::test]
async fn golden_incident_workflow_runs_end_to_end() {
    let workspace = tempdir().expect("temporary workspace");
    let source_path = workspace.path().join("calculator.rs");
    std::fs::write(
    &source_path,
    "fn add(a: i32, b: i32) -> i32 { a - b }\n\n#[test]\nfn adds_two_numbers() { assert_eq!(add(2, 3), 5); }\n",
  )
  .expect("seed calculator fixture");

    let mut profiles = HashMap::new();
    profiles.insert(
        "golden-incident".to_string(),
        PromptProfileConfig {
            mode: PromptMergeMode::Append,
            sections: vec![PromptSectionConfig {
                id: "golden-incident-protocol".to_string(),
                title: Some("Golden incident protocol".to_string()),
                content: WORKFLOW_PROMPT.to_string(),
            }],
            files: Vec::new(),
            include_project_context: false,
        },
    );

    let config = Arc::new(Config {
        api_key: "deterministic-test-key".to_string(),
        model: "amadeus-golden-replay-v1".to_string(),
        workdir: workspace.path().to_path_buf(),
        timeout_seconds: 30,
        prompts: PromptSettings {
            active_profile: "golden-incident".to_string(),
            profiles,
        },
        ..Config::default()
    });

    let fixture = std::fs::read_to_string("tests/fixtures/scenarios/golden_incident_workflow.json")
        .expect("golden workflow fixture");
    let client = ScenarioMockClient::from_json(&fixture).expect("valid golden workflow fixture");
    let agent = Agent::builder(client.clone(), config)
        .with_default_tools()
        .build();
    agent
        .history()
        .write()
        .await
        .push(Message::user(USER_PROMPT));

    let mut events = Vec::new();
    let mut stream = std::pin::pin!(agent.run_stream());
    while let Some(event) = stream.next().await {
        let event = event.expect("golden workflow event");
        let done = matches!(event, AgentEvent::Done { .. });
        events.push(event);
        if done {
            break;
        }
    }

    let updated = std::fs::read_to_string(source_path).expect("updated calculator");
    assert!(updated.contains("a + b"));
    assert!(workspace.path().join("calculator_test").exists());

    let tool_order: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolComplete { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(tool_order, ["read_file", "edit_file", "bash"]);
    assert!(events
        .iter()
        .any(|event| matches!(event, AgentEvent::ThinkingDelta { .. })));
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::TokenUsage {
            input_tokens: 640,
            output_tokens: 180,
            ..
        }
    )));

    let final_text: String = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::TextDelta { delta } => Some(delta.as_str()),
            _ => None,
        })
        .collect();
    assert!(final_text.contains("Incident resolved"));
    assert!(final_text.contains("Verification: the focused Rust test passed"));

    let requests = client.captured_requests();
    assert_eq!(requests.len(), 4);
    assert!(requests
        .iter()
        .all(|request| request.system.contains(WORKFLOW_PROMPT)));
    assert!(requests.iter().all(|request| request.max_tokens == 8000));
    assert!(requests.iter().all(|request| {
        ["read_file", "edit_file", "bash"].iter().all(|name| {
            request
                .tools
                .iter()
                .any(|tool| tool["name"].as_str() == Some(name))
        })
    }));
    assert!(requests[0]
        .messages
        .iter()
        .any(|message| format!("{message:?}").contains(USER_PROMPT)));
    assert_eq!(client.remaining_steps(), 0);
}
