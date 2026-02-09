use crate::agent::messages::{Message, ContentBlock};
use crate::error::Result;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

#[derive(Debug)]
pub enum StreamEvent {
    TextDelta(String),
    ToolCallStart { id: String, name: String },
    ToolCallDelta { arguments: String },
    ToolCallDone(String),
    StopReason(String),
}

#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn create_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)>;

    async fn create_message_stream(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>>;
}

pub mod anthropic;
pub mod openai;

pub use anthropic::AnthropicClient;
pub use openai::OpenAIClient;
