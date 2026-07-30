// @amadeus-header
// summary: Integration tests covering mock llm behavior.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::mock_llm
// - type: tests::mock_llm::MockLLMClient
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
use std::pin::Pin;

pub struct MockLLMClient {
    pub stream_events: Vec<StreamEvent>,
}

impl MockLLMClient {
    pub fn new() -> Self {
        Self {
            stream_events: Vec::new(),
        }
    }

    pub fn with_stream_events(mut self, events: Vec<StreamEvent>) -> Self {
        self.stream_events = events;
        self
    }
}

impl Clone for MockLLMClient {
    fn clone(&self) -> Self {
        Self {
            stream_events: self.stream_events.clone(),
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
        Ok(("end_turn".to_string(), Vec::new()))
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
