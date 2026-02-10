use reqwest::{Client, StatusCode};
use serde_json::Value;
use crate::agent::messages::{Message, ContentBlock};
use crate::client::LLMClient;
use crate::client::StreamEvent;
use crate::error::{Result, AgentError};
use async_trait::async_trait;
use bytes::Bytes;
use std::pin::Pin;
use futures::{Stream, StreamExt};

const API_VERSION: &str = "2023-06-01";

pub struct AnthropicClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl AnthropicClient {
    pub fn new(api_key: String, base_url: Option<String>, model: String) -> Self {
        let base_url = base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string());
        Self {
            client: Client::new(),
            api_key,
            base_url,
            model,
        }
    }
}

#[async_trait]
impl LLMClient for AnthropicClient {
    async fn create_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)> {
        let url = format!("{}/v1/messages", self.base_url);

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "system": system,
            "messages": messages,
            "tools": tools,
        });

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            let status_code = response.status().as_u16();
            let error_text = response.text().await?;
            return Err(AgentError::InvalidResponse(format!(
                "API error {}: {}",
                status_code, error_text
            )));
        }

        let json: Value = response.json().await?;
        let stop_reason = json["stop_reason"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let content: Vec<ContentBlock> = serde_json::from_value(json["content"].clone())?;

        Ok((stop_reason, content))
    }

    async fn create_message_stream(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let url = format!("{}/v1/messages", self.base_url);

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "system": system,
            "messages": messages,
            "tools": tools,
            "stream": true,
        });

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            let status_code = response.status().as_u16();
            let error_text = response.text().await?;
            return Err(AgentError::InvalidResponse(format!(
                "API error {}: {}",
                status_code, error_text
            )));
        }

        let byte_stream = response.bytes_stream();
        Ok(Box::pin(Self::parse_sse_stream(byte_stream)))
    }
}

impl AnthropicClient {
    fn parse_sse_stream(
        stream: impl Stream<Item = reqwest::Result<bytes::Bytes>> + Unpin + 'static,
    ) -> impl Stream<Item = Result<StreamEvent>> {
        futures::stream::unfold(stream, |mut s| async move {
            let result = s.next().await;
            match result {
                Some(Ok(bytes)) => {
                    if let Some(event) = Self::parse_sse_line(bytes) {
                        Some((Ok(event), s))
                    } else {
                        Some((Ok(StreamEvent::TextDelta(String::new())), s))
                    }
                }
                Some(Err(e)) => Some((Err(AgentError::Api(e)), s)),
                None => None,
            }
        })
    }

    fn parse_sse_line(bytes: Bytes) -> Option<StreamEvent> {
        let text = String::from_utf8_lossy(&bytes);

        for line in text.lines() {
            if line.starts_with("data: ") {
                let json_str = &line[6..];
                if json_str == "[DONE]" {
                    return None;
                }

                if let Ok(json) = serde_json::from_str::<Value>(json_str) {
                    match json["type"].as_str() {
                        Some("content_block_delta") => {
                            if let Some(text) = json["delta"]["text"].as_str() {
                                return Some(StreamEvent::TextDelta(text.to_string()));
                            }
                        }
                        Some("content_block_start") => {
                            if let Some(block) = json["content_block"].as_object() {
                                if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                    return Some(StreamEvent::ToolCallStart {
                                        id: block["id"].as_str().unwrap().to_string(),
                                        name: block["name"].as_str().unwrap().to_string(),
                                    });
                                }
                            }
                        }
                        Some("content_block_stop") => {
                            if let Some(block) = json["content_block"].as_object() {
                                if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                    if let Some(id) = block["id"].as_str() {
                                        return Some(StreamEvent::ToolCallDone(id.to_string()));
                                    }
                                }
                            }
                        }
                        Some("message_stop") => {
                            if let Some(reason) = json["stop_reason"].as_str() {
                                return Some(StreamEvent::StopReason(reason.to_string()));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        None
    }
}
