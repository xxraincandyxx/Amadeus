// @amadeus-header
// summary: Integration tests covering simulation p2p behavior.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::simulation_p2p
// uses:
// - module: amadeus::agent::config::Config
// - module: amadeus::agent::messages
// - module: amadeus::agent::orchestra
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
// - cmd: cargo test simulation_p2p --features full
// @end-amadeus-header

use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use amadeus::agent::config::Config;
use amadeus::agent::messages::{ContentBlock, Message};
use amadeus::agent::orchestra::{
    OrchestraConfig, OrchestraRuntime, OrchestraStrategy, Task, TaskResult, WorkerConfig,
};
use amadeus::client::{LLMClient, StreamEvent};
use amadeus::core::AgentId;
use amadeus::error::Result;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

#[derive(Clone)]
struct SimulationMockClient {
    name: String,
    // Using a counter to decide behavior: first call might be peer call, second is answer
    call_count: Arc<Mutex<u32>>,
    peer_chance: f64,
}

#[async_trait]
impl LLMClient for SimulationMockClient {
    async fn create_message(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)> {
        Ok(("end_turn".to_string(), vec![]))
    }

    async fn create_message_stream(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let mut count = self.call_count.lock().await;
        *count += 1;

        let mut events = Vec::new();

        // Logic:
        // If it's the first time and we roll the peer_chance, call a peer.
        // Otherwise, just return a result.
        let should_call_peer = *count == 1 && rand::random::<f64>() < self.peer_chance;

        if should_call_peer {
            let id = format!("call_{}", *count);
            events.push(Ok(StreamEvent::ToolCallStart {
                id: id.clone(),
                name: "call_peer".to_string(),
            }));
            events.push(Ok(StreamEvent::ToolCallDelta {
                arguments: json!({"task": "Sub-task for peer", "capabilities": ["worker"]})
                    .to_string(),
            }));
            events.push(Ok(StreamEvent::ToolCallDone(id)));
            events.push(Ok(StreamEvent::StopReason("tool_use".to_string())));
        } else {
            events.push(Ok(StreamEvent::TextDelta(format!(
                "Processed by {}",
                self.name
            ))));
            events.push(Ok(StreamEvent::StopReason("end_turn".to_string())));
        }

        Ok(Box::pin(futures::stream::iter(events)))
    }
}

fn create_test_config() -> Arc<Config> {
    Arc::new(Config {
        api_key: "mock".to_string(),
        model: "mock".to_string(),
        workdir: std::path::PathBuf::from("/tmp"),
        timeout_seconds: 2,
        ..Config::default()
    })
}

#[tokio::test]
async fn test_high_concurrency_p2p() {
    let num_workers = 5;
    let num_tasks = 120; // More than the default queue limit (100)

    let config = OrchestraConfig {
        strategy: OrchestraStrategy::LeastLoaded,
        task_timeout: Duration::from_secs(10),
        max_pending_tasks: 50, // Set a low limit to trigger overflow
        ..Default::default()
    };

    // We'll use a single client type but different instances if needed
    let base_client = SimulationMockClient {
        name: "Base".to_string(),
        call_count: Arc::new(Mutex::new(0)),
        peer_chance: 0.5,
    };

    let mut orchestra = OrchestraRuntime::new(base_client.clone(), config, create_test_config());

    // Spawn workers
    for i in 0..num_workers {
        let worker_client = SimulationMockClient {
            name: format!("Worker-{}", i),
            call_count: Arc::new(Mutex::new(0)),
            peer_chance: 0.5,
        };
        let _: Vec<AgentId> = orchestra
            .spawn_with_client(
                vec![WorkerConfig::new(format!("Worker-{}", i)).capability("worker")],
                worker_client,
            )
            .await
            .expect("Failed to spawn worker");
    }

    let orchestra = Arc::new(orchestra);
    let orchestra_clone: Arc<OrchestraRuntime<SimulationMockClient>> = Arc::clone(&orchestra);
    tokio::spawn(async move {
        if let Err(e) = orchestra_clone.run().await {
            eprintln!("Orchestra runtime error: {}", e);
        }
    });

    println!(
        "🚀 Launching {} concurrent tasks (Buffering Test)...",
        num_tasks
    );

    let mut handles: Vec<tokio::task::JoinHandle<amadeus::Result<TaskResult>>> = Vec::new();
    for i in 0..num_tasks {
        let s: Arc<OrchestraRuntime<SimulationMockClient>> = Arc::clone(&orchestra);
        handles.push(tokio::spawn(async move {
            let task = Task::new(format!("task-{}", i), "Execute simulation task")
                .requires(vec!["worker".to_string()]);
            s.execute(task).await
        }));
    }

    let mut success_count = 0;
    let mut error_count = 0;
    let mut queue_full_count = 0;

    for handle in handles {
        match handle.await.unwrap() {
            Ok(res) => {
                if res.success {
                    success_count += 1;
                } else {
                    error_count += 1;
                }
            }
            Err(e) => {
                error_count += 1;
                if e.to_string().contains("Task queue is full") {
                    queue_full_count += 1;
                }
            }
        }
    }

    println!("🏁 Simulation Results (Buffering Test):");
    println!("   - Total Tasks:       {}", num_tasks);
    println!("   - Successes:         {}", success_count);
    println!("   - Total Errors:      {}", error_count);
    println!("   - Queue Full Errors: {}", queue_full_count);

    assert!(
        success_count > 0,
        "Should have successes thanks to buffering"
    );
    assert!(
        queue_full_count > 0,
        "Should have queue full errors due to high load"
    );
    println!("✅ Buffering and overflow protection verified.");
}
