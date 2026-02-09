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

pub struct OpenAIClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAIClient {
    pub fn new(api_key: String, base_url: Option<String>, model: String) -> Self {
        let base_url = base_url.unwrap_or_else(|| "https://api.openai.com".to_string());
        Self {
            client: Client::new(),
            api_key,
            base_url,
            model,
        }
    }

    fn transform_tools(tools: &[Value]) -> Vec<Value> {
        tools
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool["name"],
                        "description": tool["description"],
                        "parameters": tool["input_schema"]
                    }
                })
            })
            .collect()
    }

    fn prepare_messages(system: &str, messages: &[Message]) -> Vec<Value> {
        let mut result = vec![serde_json::json!({"role": "system", "content": system})];

        for msg in messages {
            let content = msg
                .content
                .iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => {
                        serde_json::json!({"type": "text", "text": text})
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        serde_json::json!({"type": "tool_use", "id": id, "name": name, "input": input})
                    }
                    ContentBlock::ToolResult { tool_use_id, content } => {
                        serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_use_id,
                            "content": content
                        })
                    }
                })
                .collect::<Vec<_>>();

            result.push(serde_json::json!({
                "role": msg.role,
                "content": content
            }));
        }

        result
    }

    fn parse_response(response: Value) -> Result<(String, Vec<ContentBlock>)> {
        let choices = response["choices"]
            .as_array()
            .ok_or_else(|| AgentError::InvalidResponse("Missing choices".to_string()))?;

        let choice = &choices[0];
        let finish_reason = choice["finish_reason"].as_str().unwrap_or("").to_string();
        let message = &choice["message"];

        let stop_reason = match finish_reason.as_str() {
            "tool_calls" => "tool_use".to_string(),
            "stop" => "end_turn".to_string(),
            "length" => "max_tokens".to_string(),
            _ => finish_reason,
        };

        let content = if let Some(content_array) = message["content"].as_array() {
            content_array
                .iter()
                .map(|block| match block["type"].as_str() {
                    Some("text") => ContentBlock::Text {
                        text: block["text"].as_str().unwrap_or("").to_string(),
                    },
                    Some("tool_use") => ContentBlock::ToolUse {
                        id: block["id"].as_str().unwrap_or("").to_string(),
                        name: block["name"].as_str().unwrap_or("").to_string(),
                        input: crate::agent::messages::ToolInput {
                            command: block["input"]["command"].as_str().unwrap_or("").to_string(),
                        },
                    },
                    Some("tool_result") => ContentBlock::ToolResult {
                        tool_use_id: block["tool_use_id"].as_str().unwrap_or("").to_string(),
                        content: block["content"].as_str().unwrap_or("").to_string(),
                    },
                    _ => ContentBlock::Text {
                        text: String::new(),
                    },
                })
                .collect()
        } else if let Some(tool_calls) = message["tool_calls"].as_array() {
            tool_calls
                .iter()
                .map(|call| {
                    let id = call["id"].as_str().unwrap_or("").to_string();
                    let name = call["function"]["name"].as_str().unwrap_or("").to_string();
                    let args_json = call["function"]["arguments"].as_str().unwrap_or("{}");
                    let args: serde_json::Value = serde_json::from_str(args_json).unwrap_or(serde_json::json!({}));
                    let command = args["command"].as_str().unwrap_or("").to_string();

                    ContentBlock::ToolUse {
                        id,
                        name,
                        input: crate::agent::messages::ToolInput { command },
                    }
                })
                .collect()
        } else {
            vec![ContentBlock::Text {
                text: String::new(),
            }]
        };

        Ok((stop_reason, content))
    }
}

#[async_trait]
impl LLMClient for OpenAIClient {
    async fn create_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        let openai_messages = Self::prepare_messages(system, messages);
        let openai_tools = Self::transform_tools(tools);

        let body = serde_json::json!({
            "model": self.model,
            "messages": openai_messages,
            "tools": openai_tools,
            "max_tokens": max_tokens,
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            let status = response.status().as_u16();
            let error = response.text().await?;
            return Err(AgentError::InvalidResponse(format!(
                "API error {}: {}",
                status, error
            )));
        }

        let json: Value = response.json().await?;
        Self::parse_response(json)
    }

    async fn create_message_stream(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        let openai_messages = Self::prepare_messages(system, messages);
        let openai_tools = Self::transform_tools(tools);

        let body = serde_json::json!({
            "model": self.model,
            "messages": openai_messages,
            "tools": openai_tools,
            "max_tokens": max_tokens,
            "stream": true,
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            let status = response.status().as_u16();
            let error = response.text().await?;
            return Err(AgentError::InvalidResponse(format!(
                "API error {}: {}",
                status, error
            )));
        }

        let byte_stream = response.bytes_stream();
        Ok(Box::pin(Self::parse_sse_stream(byte_stream)))
    }
}

impl OpenAIClient {
    fn parse_sse_stream(
        stream: impl Stream<Item = reqwest::Result<bytes::Bytes>> + Unpin + 'static,
    ) -> impl Stream<Item = Result<StreamEvent>> {
        futures::stream::unfold(stream, |mut s| async move {
            let result = s.next().await;
            match result {
                Some(Ok(bytes)) => {
                    if let Some(event) = Self::parse_sse_line(bytes) {
                        return Some((Ok(event), s));
                    }
                    return Some((
                        Err(AgentError::StreamError("Invalid SSE format".to_string())),
                        s,
                    ))
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
                    if let Some(choices) = json["choices"].as_array() {
                        if choices.is_empty() {
                            break;
                        }
                        let choice = &choices[0];
                        
                        if let Some(delta) = choice["delta"].as_object() {
                            if let Some(content) = delta["content"].as_str() {
                                return Some(StreamEvent::TextDelta(content.to_string()));
                            }

                            if let Some(tool_calls) = delta["tool_calls"].as_array() {
                                if tool_calls.is_empty() {
                                    continue;
                                }
                                let call = &tool_calls[0];
                                
                                if let Some(id) = call["id"].as_str() {
                                    if let Some(func) = call["function"].as_object() {
                                        if let Some(name) = func["name"].as_str() {
                                            return Some(StreamEvent::ToolCallStart {
                                                id: id.to_string(),
                                                name: name.to_string(),
                                            });
                                        }
                                    }
                                }
                            }

                            if let Some(tool_calls) = delta["tool_calls"].as_array() {
                                if tool_calls.is_empty() {
                                    continue;
                                }
                                let call = &tool_calls[0];
                                
                                if let Some(func) = call["function"].as_object() {
                                    if let Some(args) = func["arguments"].as_str() {
                                        return Some(StreamEvent::ToolCallDelta {
                                            arguments: args.to_string(),
                                        });
                                    }
                                }
                            }

                            if let Some(finish_reason) = choice["finish_reason"].as_str() {
                                return Some(StreamEvent::StopReason(finish_reason.to_string()));
                            }
                        }
                    }
                }
            }
        }
        None
    }
}
