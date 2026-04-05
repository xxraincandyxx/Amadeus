// @amadeus-header
// summary: Integration tests covering e2e product flow behavior.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::e2e_product_flow
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
// - cmd: cargo test e2e_product_flow --features full
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

/// A mock client that facilitates a narrative-driven E2E flow.
#[derive(Clone)]
struct StoryClient {
    role: String,
    // Using a simple state machine to return specific responses based on the "turn"
    turn: Arc<Mutex<usize>>,
}

#[async_trait]
impl LLMClient for StoryClient {
    async fn create_message(
        &self,
        _: &str,
        _: &[Message],
        _: &[serde_json::Value],
        _: u32,
    ) -> Result<(String, Vec<ContentBlock>)> {
        Ok(("end_turn".to_string(), vec![]))
    }

    async fn create_message_stream(
        &self,
        _: &str,
        _: &[Message],
        _: &[serde_json::Value],
        _: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let mut turn = self.turn.lock().await;
        *turn += 1;

        let mut events = Vec::new();

        match (self.role.as_str(), *turn) {
            // --- PRODUCT MANAGER TURN 1 ---
            ("PM", 1) => {
                events.push(Ok(StreamEvent::TextDelta("I've planned the feature. Coder, please implement a 'Calculator' class with an 'add' method.".to_string())));
            }

            // --- CODER TURNS ---
            ("Coder", 1) => {
                events.push(Ok(StreamEvent::TextDelta(
                    "Implementation started. I need to clarify the overflow behavior with the PM."
                        .to_string(),
                )));
                events.push(Ok(StreamEvent::ToolCallStart {
                    id: "pm_consult".into(),
                    name: "call_peer".into(),
                }));
                events.push(Ok(StreamEvent::ToolCallDelta {
                    arguments: json!({"task": "Should the Calculator handle integer overflow or return an error?", "capabilities": ["product"]}).to_string() 
                }));
                events.push(Ok(StreamEvent::ToolCallDone("pm_consult".into())));
                events.push(Ok(StreamEvent::StopReason("tool_use".into())));
            }
            ("Coder", 2) => {
                events.push(Ok(StreamEvent::TextDelta(
                    "Understood. PM said to handle it gracefully. I've implemented the Calculator."
                        .to_string(),
                )));
            }

            // --- PM RESPONSE TO CODER ---
            ("PM", 2) => {
                events.push(Ok(StreamEvent::TextDelta(
                    "The Calculator should return an 'Error' string on overflow.".to_string(),
                )));
            }

            // --- REVIEWER TURN 1 ---
            ("Reviewer", 1) => {
                events.push(Ok(StreamEvent::TextDelta("Reviewing code... Implementation looks clean and adheres to the PM's overflow requirements. Approved.".to_string())));
            }

            _ => {
                events.push(Ok(StreamEvent::TextDelta("Task acknowledged.".to_string())));
            }
        }

        events.push(Ok(StreamEvent::StopReason("end_turn".into())));
        Ok(Box::pin(futures::stream::iter(events)))
    }
}

fn create_test_config() -> Arc<Config> {
    Arc::new(Config {
        api_key: "e2e-test".into(),
        model: "e2e-story-model".into(),
        workdir: std::path::PathBuf::from("/tmp"),
        timeout_seconds: 10,
        ..Config::default()
    })
}

#[tokio::test]
async fn test_e2e_product_development_flow() {
    println!(
        "
🎭 --- STARTING E2E PRODUCT DEVELOPMENT FLOW --- 🎭"
    );

    let config = SupervisorConfig {
        strategy: DispatchStrategy::CapabilityMatch,
        ..Default::default()
    };

    let mut supervisor = Supervisor::new(
        StoryClient {
            role: "Base".into(),
            turn: Arc::new(Mutex::new(0)),
        },
        config,
        create_test_config(),
    );

    // 1. Setup the Team
    println!("👥 Spawning the product team...");

    let _: Vec<AgentId> = supervisor
        .spawn_with_client(
            vec![WorkerConfig::new("Alice (PM)").capability("product")],
            StoryClient {
                role: "PM".into(),
                turn: Arc::new(Mutex::new(0)),
            },
        )
        .await
        .expect("Failed to spawn PM");

    let _: Vec<AgentId> = supervisor
        .spawn_with_client(
            vec![WorkerConfig::new("Bob (Coder)").capability("code")],
            StoryClient {
                role: "Coder".into(),
                turn: Arc::new(Mutex::new(0)),
            },
        )
        .await
        .expect("Failed to spawn Coder");

    let _: Vec<AgentId> = supervisor
        .spawn_with_client(
            vec![WorkerConfig::new("Charlie (Reviewer)").capability("review")],
            StoryClient {
                role: "Reviewer".into(),
                turn: Arc::new(Mutex::new(0)),
            },
        )
        .await
        .expect("Failed to spawn Reviewer");

    let supervisor = Arc::new(supervisor);
    let supervisor_clone: Arc<Supervisor<StoryClient>> = Arc::clone(&supervisor);
    tokio::spawn(async move {
        let _ = supervisor_clone.run().await;
    });

    // 2. The Narrative

    // Turn A: PM Plans
    println!(
        "
📋 Phase 1: Planning"
    );
    let plan_task =
        Task::new("plan-1", "Design the Calculator feature").requires(vec!["product".into()]);
    let plan_res: TaskResult = supervisor
        .execute(plan_task)
        .await
        .expect("Failed to execute plan task");
    println!("   PM Response: {}", plan_res.output.as_ref().unwrap());

    // Turn B: Coder Implements (includes a P2P call back to PM)
    println!(
        "
💻 Phase 2: Implementation (Bob working...)"
    );
    let code_task = Task::new("code-1", "Implement the Calculator as per the PM's plan")
        .requires(vec!["code".into()]);
    let code_res: TaskResult = supervisor
        .execute(code_task)
        .await
        .expect("Failed to execute code task");
    println!("   Coder Result: {}", code_res.output.as_ref().unwrap());

    // Turn C: Reviewer Verifies
    println!(
        "
🔍 Phase 3: Review (Charlie checking...)"
    );
    let review_task = Task::new("review-1", "Verify the Calculator implementation")
        .requires(vec!["review".into()]);
    let review_res: TaskResult = supervisor
        .execute(review_task)
        .await
        .expect("Failed to execute review task");
    println!(
        "   Reviewer Result: {}",
        review_res.output.as_ref().unwrap()
    );

    // 3. Verification
    println!(
        "
🏁 --- E2E FLOW COMPLETE --- 🏁"
    );

    assert!(plan_res.success);
    assert!(code_res.success);
    assert!(review_res.success);

    // Verify specific story points
    assert!(code_res
        .output
        .unwrap()
        .contains("PM said to handle it gracefully"));
    assert!(review_res.output.unwrap().contains("Approved"));

    println!(
        "
✅ All phases completed successfully. Team collaboration verified."
    );
}
