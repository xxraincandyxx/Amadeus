use amadeus::agent::config::Config;
use amadeus::agent::events::AgentEvent;
use amadeus::agent::loop_agent::Agent;
use amadeus::agent::messages::{ContentBlock, Message};
use amadeus::client::{LLMClient, StreamEvent};
use amadeus::error::Result;
use async_trait::async_trait;
use futures::Stream;
use futures::StreamExt;
use serde_json::json;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// A stateful mock client for testing multi-turn agent loops.
#[derive(Clone)]
pub struct StatefulMockClient {
    pub responses: Arc<Mutex<Vec<(String, Vec<ContentBlock>)>>>,
}

impl StatefulMockClient {
    pub fn new(responses: Vec<(String, Vec<ContentBlock>)>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
        }
    }
}

#[async_trait]
impl LLMClient for StatefulMockClient {
    async fn create_message(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)> {
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            panic!("Mock client has no more responses!");
        }
        Ok(responses.remove(0))
    }

    async fn create_message_stream(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            // Return a simple end_turn if we run out of mock responses unexpectedly
            let stream =
                futures::stream::iter(vec![Ok(StreamEvent::StopReason("end_turn".to_string()))]);
            return Ok(Box::pin(stream));
        }

        let (_stop_reason, content_blocks) = responses.remove(0);
        let mut events = Vec::new();

        let mut has_tool = false;
        for block in content_blocks {
            match block {
                ContentBlock::Text { text } => {
                    events.push(StreamEvent::TextDelta(text));
                }
                ContentBlock::ToolUse { id, name, input } => {
                    has_tool = true;
                    events.push(StreamEvent::ToolCallStart {
                        id: id.clone(),
                        name,
                    });
                    events.push(StreamEvent::ToolCallDelta {
                        arguments: serde_json::to_string(&input).unwrap(),
                    });
                    events.push(StreamEvent::ToolCallDone(id));
                }
                _ => {}
            }
        }

        let stop_reason = if has_tool { "tool_use" } else { "end_turn" };
        events.push(StreamEvent::StopReason(stop_reason.to_string()));

        let stream = futures::stream::iter(events.into_iter().map(Ok));
        Ok(Box::pin(stream))
    }
}

fn create_test_config() -> Arc<Config> {
    Arc::new(Config {
        api_key: "mock-key".to_string(),
        model: "mock-model".to_string(),
        workdir: std::path::PathBuf::from("/tmp"),
        timeout_seconds: 30,
        ..Config::default()
    })
}

#[tokio::test]
async fn test_agent_functional_loop() {
    println!("\n🎭 AMADEUS AGENT - FUNCTIONAL SIMULATION 🎭");
    println!("============================================");

    let mock_responses = vec![
        (
            "tool_use".to_string(),
            vec![ContentBlock::ToolUse {
                id: "call_123".to_string(),
                name: "bash".to_string(),
                input: json!({"command": "echo 'System Check: OK'"}),
            }],
        ),
        (
            "end_turn".to_string(),
            vec![ContentBlock::Text {
                text: "The system check is complete. Everything is operational.".to_string(),
            }],
        ),
    ];

    let client = StatefulMockClient::new(mock_responses);
    let config = create_test_config();
    let agent = Agent::new(client, config);

    println!("👤 USER: Run a system check.");

    // Add to history manually as Agent.run would do
    {
        let history_arc = agent.history();
        let mut history = history_arc.write().await;
        history.push(Message::user("Run a system check."));
    }

    let mut stream = agent.run_stream();
    let mut final_text = String::new();
    let mut tool_count = 0;

    while let Some(event_result) = stream.next().await {
        let event = event_result.unwrap();
        match event {
            AgentEvent::TextDelta { delta } => {
                final_text.push_str(&delta);
                // Print delta to illustrate streaming
                print!("{}", delta);
                use std::io::{self, Write};
                io::stdout().flush().unwrap();
            }
            AgentEvent::ToolStart { name, .. } => {
                println!("\n⚙️  AGENT REQUESTS TOOL: [{}]", name);
            }
            AgentEvent::ToolComplete { output, .. } => {
                println!("📝 TOOL OUTPUT: {}", output.trim());
                tool_count += 1;
                println!("🤖 AGENT PROCESSING OUTPUT...");
            }
            AgentEvent::Done { result } => {
                println!("\n\n🏁 SIMULATION COMPLETE");
                println!("--------------------------------------------");
                println!("📊 Stats:");
                println!("   - Total Turns: {}", agent.history().read().await.len());
                println!("   - Tools Used: {}", tool_count);
                println!("   - Final Answer: {}", result.text);
            }
            _ => {}
        }
    }

    assert_eq!(tool_count, 1);
    assert!(final_text.contains("system check is complete"));

    println!("✅ Functional Simulation Passed!\n");
}
