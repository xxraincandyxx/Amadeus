// @amadeus-header
// summary: Integration test asserting the agent loop writes LLM I/O trace records.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::llm_trace_test
// uses:
// - type: amadeus::agent::llm_trace::LlmTraceSink
// - type: amadeus::agent::loop_agent::Agent
// - type: tests::mock_llm::MockLLMClient
// invariants:
// - A turn driven through the agent loop produces one request and one response trace record whose payloads reflect what the model saw and said.
// side_effects:
// - Writes a trace JSONL file into a tempdir.
// tests:
// - cmd: cargo test llm_trace_test --features full
// @end-amadeus-header

//! Verifies that attaching an [`LlmTraceSink`] to an agent captures the full
//! request/response of every provider call.

use std::sync::Arc;

use amadeus::agent::config::Config;
use amadeus::agent::llm_trace::LlmTraceSink;
use amadeus::agent::loop_agent::Agent;
use amadeus::client::StreamEvent;

mod mock_llm;

use mock_llm::MockLLMClient;

fn create_test_config() -> Arc<Config> {
    Arc::new(Config {
        api_key: "mock-key".to_string(),
        model: "mock-model".to_string(),
        workdir: std::path::PathBuf::from("/tmp"),
        timeout_seconds: 30,
        ..Config::default()
    })
}

fn read_trace_lines(path: &std::path::Path) -> Vec<serde_json::Value> {
    let bytes = std::fs::read_to_string(path).expect("trace file readable");
    bytes
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).expect("jsonl line"))
        .collect()
}

#[tokio::test]
async fn agent_loop_writes_request_and_response_trace() {
    let dir = tempfile::tempdir().expect("tempdir");
    let trace = Arc::new(LlmTraceSink::open(dir.path(), "test-session").expect("open sink"));
    let trace_path = trace.path().to_path_buf();

    let client = MockLLMClient::new().with_stream_events(vec![
        StreamEvent::TextDelta("hello world".to_string()),
        StreamEvent::StopReason("end_turn".to_string()),
    ]);
    let config = create_test_config();
    let agent = Agent::builder(client, config)
        .with_default_tools()
        .with_llm_trace(Some(Arc::clone(&trace)))
        .build();

    agent.run("say hello").await.expect("agent run");

    let lines = read_trace_lines(&trace_path);
    let requests: Vec<_> = lines
        .iter()
        .filter(|v| v.get("kind").and_then(|k| k.as_str()) == Some("request"))
        .collect();
    let responses: Vec<_> = lines
        .iter()
        .filter(|v| v.get("kind").and_then(|k| k.as_str()) == Some("response"))
        .collect();

    assert!(
        !requests.is_empty(),
        "expected at least one request trace record"
    );
    assert!(
        !responses.is_empty(),
        "expected at least one response trace record"
    );

    // The request captured the user's prompt in the message history.
    let first_request = &requests[0];
    let system = first_request
        .get("system")
        .and_then(|v| v.as_str())
        .expect("system present");
    assert!(!system.is_empty(), "system prompt was captured");
    let messages = first_request.get("messages").expect("messages present");
    let last_message_text = messages
        .as_array()
        .expect("messages is array")
        .last()
        .expect("at least one message")
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|b| b.get("text"))
        .and_then(|t| t.as_str())
        .expect("user text block");
    assert_eq!(last_message_text, "say hello");

    // The response captured the model's text and stop reason.
    let first_response = &responses[0];
    assert_eq!(
        first_response.get("stop_reason").and_then(|v| v.as_str()),
        Some("end_turn")
    );
    assert_eq!(
        first_response.get("text").and_then(|v| v.as_str()),
        Some("hello world")
    );
    assert_eq!(
        first_response.get("model").and_then(|v| v.as_str()),
        Some("mock-model")
    );
}
