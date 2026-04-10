// @amadeus-header
// summary: Integration tests covering todo test behavior.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::todo_test
// uses:
// - module: amadeus::agent::loop_agent::Agent
// - module: amadeus::agent
// - module: amadeus::client::anthropic::AnthropicClient
// - module: amadeus::client::StreamEvent
// - module: amadeus::tools
// invariants:
// - Assertions stay aligned with current user-visible behavior.
// side_effects: none
// tests:
// - cmd: cargo test todo_test --features full
// @end-amadeus-header

use std::sync::Arc;

use amadeus::agent::loop_agent::Agent;
use amadeus::agent::{Config, SessionLog, SessionStats};
use amadeus::client::anthropic::AnthropicClient;
use amadeus::client::StreamEvent;
use amadeus::tools::{TodoItem, TodoStatus};
use tempfile::tempdir;

#[path = "mocks/mod.rs"]
mod mocks;

#[path = "scenarios/mod.rs"]
mod scenarios;

use mocks::ScenarioMockClient;
use scenarios::ScenarioBuilder;

fn create_test_config() -> Config {
    Config {
        api_key: "test-key".to_string(),
        model: "test-model".to_string(),
        workdir: std::path::PathBuf::from("/tmp"),
        timeout_seconds: 30,
        ..Config::default()
    }
}

#[tokio::test]
async fn default_tools_include_todo() {
    let client = AnthropicClient::new("test-key".to_string(), None, "test-model".to_string());
    let agent = Agent::new(client, Arc::new(create_test_config()));

    assert!(agent.registry().names().contains(&"todo".to_string()));
}

#[tokio::test]
async fn todo_tool_is_sent_to_model() {
    let client =
        ScenarioMockClient::scripted(vec![vec![StreamEvent::StopReason("end_turn".to_string())]]);

    let scenario = ScenarioBuilder::new("todo_schema").build();
    let runner = scenarios::ScenarioRunner::new(scenario);
    runner
        .execute(client.clone())
        .await
        .expect("Scenario failed");

    let request = client.last_request().expect("Missing captured request");
    let tool_names: Vec<String> = request
        .tools
        .into_iter()
        .filter_map(|tool| {
            tool.get("name")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect();

    assert!(tool_names.iter().any(|name| name == "todo"));
}

#[tokio::test]
async fn save_and_restore_session_preserves_todos() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let client = AnthropicClient::new("test-key".to_string(), None, "test-model".to_string());
    let mut config = create_test_config();
    config.session_log_dir = Some(temp_dir.path().to_path_buf());

    let agent = Agent::builder(client, Arc::new(config))
        .with_default_tools()
        .with_todos(vec![TodoItem {
            id: "1".to_string(),
            text: "Track todo state".to_string(),
            status: TodoStatus::InProgress,
        }])
        .build();

    let path = agent
        .save_session(SessionStats::default())
        .await
        .expect("Failed to save session")
        .expect("Expected session log path");

    let loaded: SessionLog =
        Agent::<AnthropicClient>::load_session(&path).expect("Failed to load session");
    assert_eq!(loaded.todos.len(), 1);
    assert_eq!(loaded.todos[0].text, "Track todo state");
    assert_eq!(loaded.todos[0].status, TodoStatus::InProgress);

    let restore_agent = Agent::builder(
        AnthropicClient::new("test-key".to_string(), None, "test-model".to_string()),
        Arc::new(create_test_config()),
    )
    .with_default_tools()
    .build();

    restore_agent.restore_session(&loaded).await;
    let todos = restore_agent.todos();
    let guard = todos.read().expect("Todo lock poisoned");
    assert_eq!(guard.items().len(), 1);
    assert_eq!(guard.items()[0].id, "1");
    assert_eq!(guard.items()[0].status, TodoStatus::InProgress);
}
