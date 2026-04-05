#![cfg(feature = "supervisor")]
// @amadeus-header
// summary: Integration tests covering p2p test behavior.
// layer: test
// status: test-only
// feature_flags:
// - full
// - supervisor
// provides:
// - module: tests::p2p_test
// uses:
// - module: amadeus::agent::config::Config
// - module: amadeus::agent::messages
// - module: amadeus::agent::supervisor
// - module: amadeus::agent::worker
// - module: amadeus::client
// - module: amadeus::core::AgentId
// - module: amadeus::error::Result
// - runtime: tokio async runtime
// invariants:
// - Assertions stay aligned with current user-visible behavior.
// side_effects:
// - Spawns asynchronous tasks.
// - Writes output to stdout or stderr.
// tests:
// - cmd: cargo test p2p_test --features full
// @end-amadeus-header


use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use amadeus::agent::config::Config;
use amadeus::agent::messages::{ContentBlock, Message};
use amadeus::agent::supervisor::{DispatchStrategy, Supervisor, SupervisorConfig};
use amadeus::agent::worker::{Task, TaskResult, WorkerConfig};
use amadeus::client::{LLMClient, StreamEvent};
use amadeus::core::AgentId;
use amadeus::error::Result;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

/// A mock client that returns a fixed sequence of responses.
#[derive(Clone)]
struct SimpleMockClient {
    name: String,
    responses: Arc<Mutex<Vec<Vec<ContentBlock>>>>,
}

#[async_trait]
impl LLMClient for SimpleMockClient {
    async fn create_message(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)> {
        let mut resps = self.responses.lock().await;
        if resps.is_empty() {
            println!("[{}] create_message: No more mock responses", self.name);
            return Ok(("end_turn".to_string(), vec![]));
        }
        let b = resps.remove(0);
        println!(
            "[{}] create_message: Returning {} blocks",
            self.name,
            b.len()
        );
        Ok(("end_turn".to_string(), b))
    }

    async fn create_message_stream(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let mut resps = self.responses.lock().await;
        let blocks = if !resps.is_empty() {
            let b = resps.remove(0);
            println!(
                "[{}] create_message_stream: Returning turn with {} blocks",
                self.name,
                b.len()
            );
            b
        } else {
            println!(
                "[{}] create_message_stream: No more mock responses, returning empty",
                self.name
            );
            vec![]
        };

        let mut events = Vec::new();
        for b in blocks {
            match b {
                ContentBlock::Text { text } => {
                    println!("[{}]   - Text: {}", self.name, text);
                    events.push(Ok(StreamEvent::TextDelta(text)));
                }
                ContentBlock::ToolUse { id, name, input } => {
                    println!("[{}]   - Tool: {} ({})", self.name, name, id);
                    events.push(Ok(StreamEvent::ToolCallStart {
                        id: id.clone(),
                        name,
                    }));
                    events.push(Ok(StreamEvent::ToolCallDelta {
                        arguments: serde_json::to_string(&input).unwrap(),
                    }));
                    events.push(Ok(StreamEvent::ToolCallDone(id)));
                }
                _ => {}
            }
        }
        events.push(Ok(StreamEvent::StopReason("end_turn".to_string())));
        Ok(Box::pin(futures::stream::iter(events)))
    }
}

fn create_test_config() -> Arc<Config> {
    Arc::new(Config {
        api_key: "mock".to_string(),
        model: "mock".to_string(),
        workdir: std::path::PathBuf::from("/tmp"),
        ..Config::default()
    })
}

#[tokio::test]
async fn test_p2p_delegation() {
    // 1. Setup responses
    let requester_responses = vec![
        vec![ContentBlock::ToolUse {
            id: "call_peer_1".to_string(),
            name: "call_peer".to_string(),
            input: json!({
                "task": "What is 2+2?",
                "capabilities": ["math-capability"]
            }),
        }],
        vec![ContentBlock::Text {
            text: "The peer said the answer is 4.".to_string(),
        }],
    ];

    let calculator_responses = vec![vec![ContentBlock::Text {
        text: "4".to_string(),
    }]];

    let requester_client = SimpleMockClient {
        name: "Requester".to_string(),
        responses: Arc::new(Mutex::new(requester_responses)),
    };
    let calculator_client = SimpleMockClient {
        name: "Calculator".to_string(),
        responses: Arc::new(Mutex::new(calculator_responses)),
    };

    // 2. Setup Supervisor
    let config = SupervisorConfig {
        strategy: DispatchStrategy::CapabilityMatch,
        ..Default::default()
    };

    let mut supervisor = Supervisor::new(requester_client.clone(), config, create_test_config());

    let _: Vec<AgentId> = supervisor
        .spawn_with_client(
            vec![WorkerConfig::new("Requester").capability("logic-capability")],
            requester_client,
        )
        .await
        .expect("Failed to spawn requester");

    let calc_ids: Vec<AgentId> = supervisor
        .spawn_with_client(
            vec![WorkerConfig::new("Calculator").capability("math-capability")],
            calculator_client,
        )
        .await
        .expect("Failed to spawn calculator");
    let calculator_id = &calc_ids[0];

    // 3. Start supervisor loop in background
    let supervisor_arc: Arc<Supervisor<SimpleMockClient>> = Arc::new(supervisor);
    let supervisor_clone: Arc<Supervisor<SimpleMockClient>> = Arc::clone(&supervisor_arc);
    tokio::spawn(async move {
        supervisor_clone.run().await.unwrap();
    });

    // 4. Execute task
    let task =
        Task::new("main-task", "Start logical flow").requires(vec!["logic-capability".to_string()]);

    println!("Executing main task...");
    let res: TaskResult = supervisor_arc
        .execute(task)
        .await
        .expect("Failed to execute task");
    let result = &res;

    println!(
        "Task Result: Success={}, Output='{:?}', Error='{:?}'",
        result.success, result.output, result.error
    );

    assert!(
        result.success,
        "Task should succeed, but got error: {:?}",
        result.error
    );
    assert!(result.output.is_some(), "Output should not be None");
    let out = result.output.as_ref().unwrap();
    assert!(out.contains("answer is 4"), "Output was: '{}'", out);

    // Verify math worker was actually called
    let calculator_info = supervisor_arc.worker(*calculator_id).await.unwrap();
    assert_eq!(calculator_info.completed_tasks, 1);
}
