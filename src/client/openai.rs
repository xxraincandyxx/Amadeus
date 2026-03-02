//! # OpenAI API Client
//!
//! Client implementation for the OpenAI Chat Completions API.
//!
//! ## API Details
//!
//! - Endpoint: `POST /v1/chat/completions`
//! - Auth: `Authorization: Bearer <api-key>` header
//! - Streaming: Server-Sent Events (SSE) format
//!
//! ## Differences from Anthropic
//!
//! | Aspect | Anthropic | OpenAI |
//! |--------|-----------|--------|
//! | System prompt | Separate `system` field | First message with `role: "system"` |
//! | Tool format | Direct schema | Wrapped in `function` object |
//! | Tool calls | In content array | In `tool_calls` array |
//! | Stop reason | `stop_reason` field | `finish_reason` field |
//!
//! ## Example
//!
//! ```rust,ignore
//! use crate::client::openai::OpenAIClient;
//!
//! let client = OpenAIClient::new(
//!     "sk-proj-xxx".to_string(),    // API key
//!     None,                          // Base URL (uses default)
//!     "gpt-4".to_string(),           // Model
//! );
//!
//! let (stop_reason, content) = client.create_message(
//!     "You are a helpful assistant",
//!     &messages,
//!     &tools,
//!     8000,
//! ).await?;
//! ```

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

use reqwest::{Client, StatusCode};
use serde_json::Value;

use std::time::Duration;

use crate::agent::messages::{ContentBlock, Message};
use crate::client::{LLMClient, StreamEvent};
use crate::error::{AgentError, Result};

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use std::pin::Pin;

/*
 * ============================================================================
 * CONSTANTS
 * ============================================================================
 */

/// Default base URL for the OpenAI API.
const DEFAULT_BASE_URL: &str = "https://api.openai.com";

/*
 * ============================================================================
 * OPENAI CLIENT STRUCT
 * ============================================================================
 */

#[derive(Clone)]
pub struct OpenAIClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

/*
 * ============================================================================
 * HELPER METHODS
 * ============================================================================
 */

impl OpenAIClient {
    /// Create a new OpenAI client.
    pub fn new(api_key: String, base_url: Option<String>, model: String) -> Self {
        let base_url = base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        // Create a configured HTTP client with connection pooling and timeouts
        let client = Client::builder()
            .pool_max_idle_per_host(5)
            .pool_idle_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            base_url,
            model,
        }
    }

    /// Build the full URL for chat completions, handling custom base URLs gracefully.
    fn build_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');

        // If the base URL already looks like a full endpoint, use it as is
        if base.ends_with("/chat/completions") {
            return base.to_string();
        }

        // Check if the base URL ends with a version identifier (e.g., /v1, /v4)
        // This is common for OpenAI-compatible providers like Zhipu AI (BigModel)
        let has_version = base
            .split('/')
            .next_back()
            .map(|s| s.starts_with('v') && s[1..].chars().all(|c| c.is_ascii_digit()))
            .unwrap_or(false);

        if has_version {
            format!("{}/chat/completions", base)
        } else {
            // Default to the standard OpenAI path structure
            format!("{}/v1/chat/completions", base)
        }
    }

    /// Transform Anthropic-style tools to OpenAI format.
    ///
    /// OpenAI wraps tool definitions differently:
    ///
    /// ```json
    /// // Input (Anthropic style)
    /// { "name": "bash", "description": "...", "input_schema": {...} }
    ///
    /// // Output (OpenAI style)
    /// { "type": "function", "function": { "name": "bash", "description": "...", "parameters": {...} } }
    /// ```
    pub fn transform_tools(tools: &[Value]) -> Vec<Value> {
        // .iter() creates an iterator over references
        // .map() transforms each item
        // .collect() gathers results into a Vec
        tools
            .iter()
            .map(|tool| {
                // Build the OpenAI tool format
                serde_json::json!({
                    // OpenAI requires "type": "function"
                    "type": "function",
                    // The actual function details are nested
                    "function": {
                        // Extract name from Anthropic format
                        // .get() returns Option<&Value>
                        // .and_then() chains Option operations
                        // .as_str() converts Value to &str
                        // .unwrap_or("") provides default if missing
                        "name": tool.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                        "description": tool.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                        // Try "parameters" first (new standard), then "input_schema" (old Anthropic style)
                        "parameters": tool.get("parameters")
                            .or_else(|| tool.get("input_schema"))
                            .unwrap_or(&serde_json::json!({}))
                    }
                })
            })
            // Collect into Vec<Value>
            .collect()
    }

    /// Transform messages for OpenAI's message format.
    ///
    /// Key differences from Anthropic:
    /// - System prompt is first message with role "system"
    /// - Tool results are separate messages with role "tool"
    /// - Tool calls go in a "tool_calls" array, not content
    pub fn prepare_messages(system: &str, messages: &[Message]) -> Vec<Value> {
        // Pre-allocate with estimated capacity: 1 (system) + messages * 2 (potential expansion)
        let mut result = Vec::with_capacity(1 + messages.len() * 2);
        result.push(serde_json::json!({"role": "system", "content": system}));

        for msg in messages {
            let has_tool_results = msg
                .content
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolResult { .. }));

            if has_tool_results {
                for block in &msg.content {
                    if let ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                    } = block
                    {
                        result.push(serde_json::json!({
                            "role": "tool",
                            "tool_call_id": tool_use_id,
                            "content": content
                        }));
                    }
                }
            } else {
                let text_content: Vec<&str> = msg
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect();

                let tool_calls: Vec<Value> = msg
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::ToolUse { id, name, input } => Some(serde_json::json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": if input.is_null() {
                                    "{}".to_string()
                                } else {
                                    serde_json::to_string(input).unwrap_or_else(|_| "{}".to_string())
                                }
                            }
                        })),
                        _ => None,
                    })
                    .collect();

                if msg.role == "assistant" && !tool_calls.is_empty() {
                    // Build message with tool_calls
                    // Note: Don't include null content, some APIs (litellm) reject it
                    if text_content.is_empty() {
                        result.push(serde_json::json!({
                            "role": "assistant",
                            "tool_calls": tool_calls
                        }));
                    } else {
                        result.push(serde_json::json!({
                            "role": "assistant",
                            "content": text_content.join(""),
                            "tool_calls": tool_calls
                        }));
                    }
                } else if !text_content.is_empty() {
                    result.push(serde_json::json!({
                        "role": msg.role,
                        "content": text_content.join("")
                    }));
                }
            }
        }

        result
    }

    /// Parse OpenAI response into standardized format.
    ///
    /// Handles both content array and tool_calls array formats.
    fn parse_response(response: Value) -> Result<(String, Vec<ContentBlock>)> {
        // -----------------------------------------------------------------
        // EXTRACT CHOICES ARRAY
        // -----------------------------------------------------------------

        // OpenAI returns results in a "choices" array
        // We take the first choice (for single-response requests)
        let choices = response
            .get("choices")
            .and_then(|v| v.as_array())
            .ok_or_else(|| AgentError::InvalidResponse("Missing choices".to_string()))?;

        // Get the first choice
        // &choices[0] borrows the first element
        let choice = &choices[0];

        // Extract finish_reason
        let finish_reason = choice
            .get("finish_reason")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Get the message object
        let message = choice
            .get("message")
            .ok_or_else(|| AgentError::InvalidResponse("Missing message".to_string()))?;

        // -----------------------------------------------------------------
        // MAP FINISH REASON TO STOP REASON
        // -----------------------------------------------------------------

        // OpenAI uses different names than Anthropic
        // All arms must return the same type (String)
        let stop_reason = match finish_reason.as_str() {
            "tool_calls" => "tool_use".to_string(), // Tool was called
            "stop" => "end_turn".to_string(),       // Normal completion
            "length" => "max_tokens".to_string(),   // Hit token limit
            _ => finish_reason,                     // Already a String
        };

        // -----------------------------------------------------------------
        // PARSE CONTENT
        // -----------------------------------------------------------------

        // OpenAI can return content in two formats:
        // 1. content array (newer)
        // 2. tool_calls array (older)

        // Handle content in multiple formats:
        // 1. String content (common in OpenAI-compatible APIs)
        // 2. Content array (newer OpenAI format)
        // 3. Tool calls array (when tools are used)
        let content = if let Some(content_str) = message.get("content").and_then(|v| v.as_str()) {
            vec![ContentBlock::Text {
                text: content_str.to_string(),
            }]
        } else if let Some(content_array) = message.get("content").and_then(|v| v.as_array()) {
            content_array
                .iter()
                .map(|block| {
                    // Get the block type
                    let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");

                    match block_type {
                        // Text content
                        "text" => ContentBlock::Text {
                            text: block
                                .get("text")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                        },

                        // Tool use
                        "tool_use" => {
                            let id = block
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let name = block
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let input =
                                block.get("input").cloned().unwrap_or(serde_json::json!({}));
                            ContentBlock::ToolUse { id, name, input }
                        }

                        // Tool result
                        "tool_result" => {
                            let tool_use_id = block
                                .get("tool_use_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let content = block
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            ContentBlock::ToolResult {
                                tool_use_id,
                                content,
                            }
                        }

                        // Unknown type - return empty text
                        _ => ContentBlock::Text {
                            text: String::new(),
                        },
                    }
                })
                .collect()

        // Try tool_calls array (older OpenAI format)
        } else if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
            tool_calls
                .iter()
                .map(|call| {
                    // Extract tool call details
                    let id = call
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    // Function details are nested in "function" object
                    let name = call
                        .get("function")
                        .and_then(|v| v.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    // Arguments are a JSON string (need to parse)
                    let args_json = call
                        .get("function")
                        .and_then(|v| v.get("arguments"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("{}");

                    // Parse the arguments JSON
                    let args: serde_json::Value =
                        serde_json::from_str(args_json).unwrap_or(serde_json::json!({}));

                    ContentBlock::ToolUse {
                        id,
                        name,
                        input: args,
                    }
                })
                .collect()

        // Fallback: empty text
        } else {
            vec![ContentBlock::Text {
                text: String::new(),
            }]
        };

        Ok((stop_reason, content))
    }
}

/*
 * ============================================================================
 * LLM CLIENT TRAIT IMPLEMENTATION
 * ============================================================================
 */

#[async_trait]
impl LLMClient for OpenAIClient {
    /// Send a non-streaming request to the OpenAI API.
    async fn create_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)> {
        // Build URL using helper to handle versioning correctly
        let url = self.build_url();

        // Transform inputs for OpenAI format
        let openai_messages = Self::prepare_messages(system, messages);
        let openai_tools = Self::transform_tools(tools);

        // Build request body - only include tools if non-empty
        let body = if openai_tools.is_empty() {
            serde_json::json!({
                "model": self.model,
                "messages": openai_messages,
                "max_tokens": max_tokens,
            })
        } else {
            serde_json::json!({
                "model": self.model,
                "messages": openai_messages,
                "tools": openai_tools,
                "max_tokens": max_tokens,
            })
        };

        // Send request
        let response = self
            .client
            .post(&url)
            // OpenAI uses Bearer token authentication
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        // Check status
        if response.status() != StatusCode::OK {
            let status = response.status().as_u16();
            let error = response.text().await?;
            return Err(AgentError::InvalidResponse(format!(
                "API error {}: {}",
                status, error
            )));
        }

        // Parse response
        let json: Value = response.json().await?;
        Self::parse_response(json)
    }

    /// Send a streaming request to the OpenAI API.
    async fn create_message_stream(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let url = self.build_url();

        let openai_messages = Self::prepare_messages(system, messages);
        let openai_tools = Self::transform_tools(tools);

        // Build request with stream: true - only include tools if non-empty
        let body = if openai_tools.is_empty() {
            serde_json::json!({
                "model": self.model,
                "messages": openai_messages,
                "max_tokens": max_tokens,
                "stream": true,
            })
        } else {
            serde_json::json!({
                "model": self.model,
                "messages": openai_messages,
                "tools": openai_tools,
                "max_tokens": max_tokens,
                "stream": true,
            })
        };

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

/*
 * ============================================================================
 * SSE STREAM PARSING
 * ============================================================================
 */

impl OpenAIClient {
    /// Parse the SSE byte stream into a stream of StreamEvents.
    fn parse_sse_stream(
        mut stream: impl Stream<Item = reqwest::Result<bytes::Bytes>> + Unpin + 'static,
    ) -> impl Stream<Item = Result<StreamEvent>> {
        use async_stream::stream;

        stream! {
            let mut buffer = String::new();

            while let Some(result) = stream.next().await {
                match result {
                    Ok(bytes) => {
                        // Append new bytes to buffer
                        // Note: from_utf8_lossy might be problematic if a multi-byte char
                        // is split across chunks, but it's better than dropping events.
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        // Process all complete lines in the buffer
                        while let Some(newline_idx) = buffer.find('\n') {
                            let line = buffer[..newline_idx].trim().to_string();
                            // Remove the processed line (including the newline)
                            buffer.drain(..=newline_idx);

                            if line.is_empty() {
                                continue;
                            }

                            for event in Self::parse_sse_line(&line) {
                                yield Ok(event);
                            }
                        }
                    }
                    Err(e) => yield Err(AgentError::Api(e.to_string())),
                }
            }
        }
    }

    /// Parse a single SSE line into a stream of StreamEvents.
    ///
    /// OpenAI's SSE format: `data: <json>`
    fn parse_sse_line(line: &str) -> Vec<StreamEvent> {
        let mut events = Vec::new();

        if let Some(json_str) = line.strip_prefix("data: ") {
            if json_str == "[DONE]" {
                return events;
            }

            if let Ok(json) = serde_json::from_str::<Value>(json_str) {
                // OpenAI structure: choices[0].delta contains the updates
                if let Some(choices) = json.get("choices").and_then(|v| v.as_array()) {
                    if !choices.is_empty() {
                        let choice = &choices[0];

                        // Delta contains incremental updates
                        if let Some(delta) = choice.get("delta").and_then(|v| v.as_object()) {
                            // Text content delta
                            if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                                events.push(StreamEvent::TextDelta(content.to_string()));
                            }

                            // Tool call details (can have start and delta in same chunk)
                            if let Some(tool_calls) =
                                delta.get("tool_calls").and_then(|v| v.as_array())
                            {
                                if !tool_calls.is_empty() {
                                    let call = &tool_calls[0];

                                    // New tool call (has ID)
                                    if let Some(id) = call.get("id").and_then(|v| v.as_str()) {
                                        if let Some(func) =
                                            call.get("function").and_then(|v| v.as_object())
                                        {
                                            if let Some(name) =
                                                func.get("name").and_then(|v| v.as_str())
                                            {
                                                events.push(StreamEvent::ToolCallStart {
                                                    id: id.to_string(),
                                                    name: name.to_string(),
                                                });
                                            }
                                        }
                                    }

                                    // Tool call arguments delta
                                    if let Some(func) = call.get("function").and_then(|v| v.as_object())
                                    {
                                        if let Some(args) =
                                            func.get("arguments").and_then(|v| v.as_str())
                                        {
                                            events.push(StreamEvent::ToolCallDelta {
                                                arguments: args.to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                        }

                        // Finish reason - emit ToolCallDone before StopReason for tool_calls
                        if let Some(finish_reason) =
                            choice.get("finish_reason").and_then(|v| v.as_str())
                        {
                            if finish_reason == "tool_calls" {
                                events.push(StreamEvent::ToolCallDone(String::new()));
                            }
                            events.push(StreamEvent::StopReason(finish_reason.to_string()));
                        }
                    }
                }
            }
        }
        events
    }
}
