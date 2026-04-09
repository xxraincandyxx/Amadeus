// @amadeus-header
// summary: Integration tests covering structured telemetry emission for agents and orchestration.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::telemetry_test
// uses:
// - module: amadeus::agent
// - module: amadeus::client
// - module: amadeus::telemetry
// - runtime: tokio async runtime
// invariants:
// - Telemetry assertions stay aligned with emitted runtime event ordering.
// side_effects: none
// tests:
// - cmd: cargo test --test telemetry_test --features full
// @end-amadeus-header

use std::pin::Pin;
use std::sync::Arc;

use amadeus::agent::{Agent, AgentOrchestrator, AgentProfile, Config, ContentBlock, Message, Task};
use amadeus::client::{LLMClient, StreamEvent};
use amadeus::error::Result;
use amadeus::telemetry::{MemorySink, TelemetryEvent, TelemetryRecorder};
use async_trait::async_trait;
use futures::Stream;
use serde_json::json;
use tokio::sync::Mutex;

#[derive(Clone)]
struct TelemetryMockClient {
    responses: Arc<Mutex<Vec<Vec<ContentBlock>>>>,
}

impl TelemetryMockClient {
    fn new(responses: Vec<Vec<ContentBlock>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
        }
    }
}

#[async_trait]
impl LLMClient for TelemetryMockClient {
    async fn create_message(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)> {
        unreachable!("streaming path is used in telemetry tests")
    }

    async fn create_message_stream(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let mut responses = self.responses.lock().await;
        let blocks = responses.remove(0);
        let mut events = Vec::new();
        let mut has_tool = false;

        for block in blocks {
            match block {
                ContentBlock::Text { text } => events.push(StreamEvent::TextDelta(text)),
                ContentBlock::ToolUse { id, name, input } => {
                    has_tool = true;
                    events.push(StreamEvent::ToolCallStart {
                        id: id.clone(),
                        name,
                    });
                    events.push(StreamEvent::ToolCallDelta {
                        arguments: serde_json::to_string(&input).expect("serialize tool input"),
                    });
                    events.push(StreamEvent::ToolCallDone(id));
                }
                _ => {}
            }
        }

        events.push(StreamEvent::StopReason(
            if has_tool { "tool_use" } else { "end_turn" }.to_string(),
        ));

        Ok(Box::pin(futures::stream::iter(events.into_iter().map(Ok))))
    }
}

fn create_test_config() -> Arc<Config> {
    Arc::new(Config {
        api_key: "mock-key".to_string(),
        model: "mock-model".to_string(),
        workdir: std::env::temp_dir(),
        timeout_seconds: 30,
        ..Config::default()
    })
}

#[tokio::test]
async fn agent_run_records_session_and_tool_events() {
    let sink = Arc::new(MemorySink::new());
    let telemetry = Arc::new(TelemetryRecorder::new().with_sink(sink.clone()));
    let client = TelemetryMockClient::new(vec![
        vec![ContentBlock::ToolUse {
            id: "todo-call".to_string(),
            name: "todo".to_string(),
            input: json!({
                "items": [
                    {"text": "Plan telemetry coverage", "status": "in_progress"}
                ]
            }),
        }],
        vec![ContentBlock::Text {
            text: "Telemetry run complete.".to_string(),
        }],
    ]);

    let agent = Agent::builder(client, create_test_config())
        .with_default_tools()
        .with_telemetry(telemetry)
        .build();

    let result = agent
        .run("Track this task and then summarize it.")
        .await
        .unwrap();
    assert_eq!(result.text, "Telemetry run complete.");

    let entries = sink.entries().unwrap();
    assert!(entries
        .iter()
        .any(|entry| matches!(&entry.event, TelemetryEvent::SessionStarted { .. })));
    assert!(entries
        .iter()
        .any(|entry| matches!(&entry.event, TelemetryEvent::PromptSubmitted { .. })));
    assert!(entries.iter().any(|entry| matches!(
        &entry.event,
        TelemetryEvent::ToolStarted { tool, .. } if tool == "todo"
    )));
    assert!(entries.iter().any(|entry| matches!(
        &entry.event,
        TelemetryEvent::ToolCompleted { tool, is_error, .. } if tool == "todo" && !is_error
    )));
    assert!(entries
        .iter()
        .any(|entry| matches!(&entry.event, TelemetryEvent::SessionCompleted { .. })));
}

#[tokio::test]
async fn orchestrator_records_spawn_and_task_events() {
    let sink = Arc::new(MemorySink::new());
    let telemetry = Arc::new(TelemetryRecorder::new().with_sink(sink.clone()));
    let client = TelemetryMockClient::new(vec![vec![ContentBlock::Text {
        text: "orchestra done".to_string(),
    }]]);

    let mut orchestrator =
        AgentOrchestrator::new(client, create_test_config()).with_telemetry(telemetry);
    let agent_id = orchestrator
        .create_agent(Some("runtime-agent".to_string()), AgentProfile::Default)
        .await
        .unwrap();

    let task = Task::new("task-1", "Say that the orchestra finished.");
    let result = orchestrator.execute_task(None, task).await.unwrap();
    assert_eq!(result.worker_id, agent_id);
    assert_eq!(result.output.as_deref(), Some("orchestra done"));

    let entries = sink.entries().unwrap();
    assert!(entries.iter().any(|entry| matches!(
        &entry.event,
        TelemetryEvent::WorkerSpawned { worker_id, name, .. } if *worker_id == agent_id && name == "runtime-agent"
    )));
    assert!(entries.iter().any(|entry| matches!(
        &entry.event,
        TelemetryEvent::TaskDispatched { task_id, worker_id, .. } if task_id == "task-1" && *worker_id == agent_id
    )));
    assert!(entries.iter().any(|entry| matches!(
        &entry.event,
        TelemetryEvent::TaskCompleted { task_id, worker_id, success, .. } if task_id == "task-1" && *worker_id == agent_id && *success
    )));
}
