use amadeus::agent::messages::{ContentBlock, Message};
use amadeus::client::{LLMClient, StreamEvent};
use amadeus::error::Result;
use async_trait::async_trait;
use futures::Stream;
use serde_json::json;
use std::pin::Pin;

pub struct MockLLMClient {
    pub responses: Vec<(String, Vec<ContentBlock>)>,
    pub stream_events: Vec<StreamEvent>,
}

impl MockLLMClient {
    pub fn new() -> Self {
        Self {
            responses: Vec::new(),
            stream_events: Vec::new(),
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
        Ok(self.responses[0].clone())
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
