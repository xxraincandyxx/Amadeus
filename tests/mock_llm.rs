// @amadeus-header
// summary: Integration tests covering mock llm behavior.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::mock_llm
// - type: tests::mock_llm::MockLLMClient
// - fn: tests::mock_llm::mock_tool_use_response
// - fn: tests::mock_llm::mock_text_response
// - fn: tests::mock_llm::mock_tool_result
// uses:
// - module: amadeus::agent::messages
// - module: amadeus::client
// - module: amadeus::error::Result
// - format: JSON values
// - runtime: futures streams
// invariants:
// - Assertions stay aligned with current user-visible behavior.
// side_effects: none
// tests:
// - cmd: cargo test mock_llm --features full
// @end-amadeus-header

use amadeus::agent::messages::{ContentBlock, Message};
use amadeus::client::{LLMClient, StreamEvent};
use amadeus::error::Result;
use async_trait::async_trait;
use futures::Stream;
use serde_json::json;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub struct MockLLMClient {
    pub responses: Vec<(String, Vec<ContentBlock>)>,
    pub stream_events: Vec<StreamEvent>,
    call_index: Arc<AtomicUsize>,
}

impl MockLLMClient {
    pub fn new() -> Self {
        Self {
            responses: Vec::new(),
            stream_events: Vec::new(),
            call_index: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn with_responses(mut self, responses: Vec<(String, Vec<ContentBlock>)>) -> Self {
        self.responses = responses;
        self
    }

    pub fn with_stream_events(mut self, events: Vec<StreamEvent>) -> Self {
        self.stream_events = events;
        self
    }
}

impl Clone for MockLLMClient {
    fn clone(&self) -> Self {
        Self {
            responses: self.responses.clone(),
            stream_events: self.stream_events.clone(),
            call_index: Arc::clone(&self.call_index),
        }
    }
}

impl Default for MockLLMClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LLMClient for MockLLMClient {
    async fn create_message(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)> {
        let idx = self.call_index.fetch_add(1, Ordering::SeqCst);
        let idx = idx.min(self.responses.len().saturating_sub(1));
        Ok(self.responses[idx].clone())
    }

    async fn create_message_stream(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let stream = futures::stream::iter(self.stream_events.clone().into_iter().map(Ok));
        Ok(Box::pin(stream))
    }
}

pub fn mock_tool_use_response(command: &str, tool_id: &str) -> Vec<ContentBlock> {
    vec![ContentBlock::ToolUse {
        id: tool_id.to_string(),
        name: "bash".to_string(),
        input: json!({"command": command}),
    }]
}

pub fn mock_text_response(text: &str) -> Vec<ContentBlock> {
    vec![ContentBlock::Text {
        text: text.to_string(),
    }]
}

pub fn mock_tool_result(tool_id: &str, content: &str) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: tool_id.to_string(),
        content: content.to_string(),
    }
}
