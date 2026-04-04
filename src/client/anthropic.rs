//! # Anthropic API Client
//!
//! Client implementation for the Anthropic Messages API.
//!
//! ## API Details
//!
//! - Endpoint: `POST /v1/messages`
//! - Auth: `x-api-key` header + `anthropic-version` header
//! - Streaming: Server-Sent Events (SSE) format
//!
//! ## Example
//!
//! ```rust,ignore
//! use crate::client::anthropic::AnthropicClient;
//!
//! let client = AnthropicClient::new(
//!     "sk-ant-xxx".to_string(),           // API key
//!     None,                                // Base URL (uses default)
//!     "claude-sonnet-4-5-20250929".to_string(), // Model
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

// HTTP client types from reqwest
// Client: The HTTP client used to make requests
// StatusCode: HTTP status codes (200, 404, 500, etc.)
use reqwest::{Client, StatusCode};

// Duration for HTTP client timeouts
use std::time::Duration;

// Tracing for structured logging
use tracing::{debug, info, warn};

// JSON value type from serde_json
// Value can hold any JSON data (object, array, string, number, etc.)
use serde_json::Value;

// Our message types
use crate::agent::messages::{ContentBlock, Message};

// The trait we're implementing
use crate::client::LLMClient;

// Event types for streaming
use crate::client::StreamEvent;

// Our error types
use crate::error::{AgentError, Result};

// async_trait - enables async methods in trait implementations
use async_trait::async_trait;

// Pin - pins a value in memory (needed for streams)
use std::pin::Pin;

// Stream and StreamExt for handling streaming responses
// Stream: The trait for async iteration
// StreamExt: Provides extension methods like .next()
use futures::{Stream, StreamExt};

/*
 * ============================================================================
 * CONSTANTS
 * ============================================================================
 */

/// Anthropic API version for the `anthropic-version` header.
///
/// Anthropic requires this header to specify which API version you're using.
/// Different versions may have different behaviors.
/// "2023-06-01" is a stable version that works with the Messages API.
const API_VERSION: &str = "2023-06-01";

/// Default base URL for the Anthropic API.
const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

/*
 * ============================================================================
 * ANTHROPIC CLIENT STRUCT
 * ============================================================================
 */

/// Client for the Anthropic Messages API.
///
/// This client handles communication with Anthropic's Claude models,
/// supporting both non-streaming and streaming responses.
#[derive(Clone)]
pub struct AnthropicClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

/*
 * ============================================================================
 * ANTHROPIC CLIENT IMPLEMENTATION
 * ============================================================================
 */

impl AnthropicClient {
    /// Create a new Anthropic client.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Anthropic API key (starts with `sk-ant-`)
    /// * `base_url` - Optional custom base URL (for proxies). Uses default if `None`.
    /// * `model` - Model to use (e.g., `claude-sonnet-4-5-20250929`)
    pub fn new(api_key: String, base_url: Option<String>, model: String) -> Self {
        // `unwrap_or_else` provides a default if the Option is None
        //
        // If base_url is Some(url), return url
        // If base_url is None, return the default string
        let base_url = base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        // Create a configured HTTP client with connection pooling and timeouts
        // This improves performance for multiple API calls
        let mut builder = Client::builder()
            // Maximum idle connections per host (connection pooling)
            .pool_max_idle_per_host(5)
            // How long to keep idle connections alive
            .pool_idle_timeout(Duration::from_secs(30))
            // Total request timeout (includes connection + response)
            .timeout(Duration::from_secs(120))
            // Connection establishment timeout
            .connect_timeout(Duration::from_secs(10));

        // Disable proxy by default to avoid issues with local proxies breaking connections,
        // unless AMADEUS_NO_PROXY is explicitly set to false/0
        if std::env::var("AMADEUS_NO_PROXY")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(true)
        {
            builder = builder.no_proxy();
        }

        let client = builder.build().expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            base_url,
            model,
        }
    }
}

/*
 * ============================================================================
 * LLM CLIENT TRAIT IMPLEMENTATION
 * ============================================================================
 */

// #[async_trait] - required because LLMClient has async methods
#[async_trait]
impl LLMClient for AnthropicClient {
    /// Send a non-streaming request to the Anthropic API.
    ///
    /// # Request Format
    ///
    /// ```json
    /// {
    ///   "model": "claude-sonnet-4-5-20250929",
    ///   "max_tokens": 8000,
    ///   "system": "...",
    ///   "messages": [...],
    ///   "tools": [...]
    /// }
    /// ```
    async fn create_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)> {
        // -----------------------------------------------------------------
        // BUILD THE REQUEST URL
        // -----------------------------------------------------------------

        // format! creates a String by formatting
        // {} is a placeholder that gets replaced with self.base_url
        //
        // Result: "https://api.anthropic.com/v1/messages"
        let url = format!("{}/v1/messages", self.base_url);

        debug!(
            model = %self.model,
            messages = messages.len(),
            tools = tools.len(),
            "Creating Anthropic message"
        );

        // -----------------------------------------------------------------
        // BUILD THE REQUEST BODY
        // -----------------------------------------------------------------

        // Enable Prompt Caching by structuring the system prompt as an array of blocks.
        // The ephemeral cache_control ensures the large system prompt (including tool docs)
        // is cached, which drastically reduces latency and token costs for multi-turn agent interactions.
        let system_blocks = serde_json::json!([{
            "type": "text",
            "text": system,
            "cache_control": { "type": "ephemeral" }
        }]);

        // serde_json::json! creates a JSON Value from Rust-like syntax
        // This is a macro, not a function call
        let body = serde_json::json!({
            // The model to use
            "model": self.model,
            // Maximum tokens in the response
            // u32 automatically converts to JSON number
            "max_tokens": max_tokens,
            // System prompt (instructions for the model)
            "system": system_blocks,
            // Conversation history
            // &[Message] automatically serializes to JSON array
            "messages": messages,
            // Available tools
            "tools": tools,
        });

        // -----------------------------------------------------------------
        // SEND THE HTTP REQUEST
        // -----------------------------------------------------------------

        // Build and send the HTTP POST request
        let response = self
            .client
            // Create a POST request to the URL
            .post(&url)
            // Add the API key header
            // Anthropic uses "x-api-key" header for authentication
            .header("x-api-key", &self.api_key)
            // Add the API version header
            // Required by Anthropic to specify API version
            .header("anthropic-version", API_VERSION)
            // Enable prompt caching beta header
            .header("anthropic-beta", "prompt-caching-2024-07-31")
            // Set content type to JSON
            .header("content-type", "application/json")
            // Add the JSON body
            // .json() serializes the Value to JSON and sets the body
            .json(&body)
            // Send the request
            // .send() returns a Future that resolves to the response
            .send()
            // .await waits for the response
            // ? propagates errors (converts reqwest::Error to AgentError)
            .await?;

        // -----------------------------------------------------------------
        // CHECK RESPONSE STATUS
        // -----------------------------------------------------------------

        // Check if the response was successful (HTTP 200 OK)
        if response.status() != StatusCode::OK {
            // Not OK - extract error details
            let status_code = response.status().as_u16();
            // Read the error response body as text
            let error_text = response.text().await?;

            warn!(
                status_code = status_code,
                error = %error_text,
                "Anthropic API error"
            );

            // Return an error with details
            return Err(AgentError::InvalidResponse(format!(
                "API error {}: {}",
                status_code, error_text
            )));
        }

        // -----------------------------------------------------------------
        // PARSE THE SUCCESSFUL RESPONSE
        // -----------------------------------------------------------------

        // Parse the response body as JSON
        // .json() deserializes the response body to the given type
        let json: Value = response.json().await?;

        // Extract the stop_reason from the JSON
        //
        // json["stop_reason"] accesses the "stop_reason" field
        // .as_str() converts the JSON value to &str (returns Option<&str>)
        // .unwrap_or("") provides a default if the field is missing/not a string
        // .to_string() converts &str to String
        let stop_reason = json["stop_reason"].as_str().unwrap_or("").to_string();

        // Parse the content array
        //
        // json["content"].clone() - clones the content value (needed for from_value)
        // serde_json::from_value() - converts a Value to a Rust type
        //
        // This parses the JSON content array into Vec<ContentBlock>
        // ? propagates parse errors
        let content: Vec<ContentBlock> = serde_json::from_value(json["content"].clone())?;

        info!(
            stop_reason = %stop_reason,
            content_blocks = content.len(),
            "Anthropic response received"
        );

        // Return the tuple (stop_reason, content)
        Ok((stop_reason, content))
    }

    /// Send a streaming request to the Anthropic API.
    async fn create_message_stream(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let url = format!("{}/v1/messages", self.base_url);

        // Enable Prompt Caching
        let system_blocks = serde_json::json!([{
            "type": "text",
            "text": system,
            "cache_control": { "type": "ephemeral" }
        }]);

        // Build the request body with stream: true
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "system": system_blocks,
            "messages": messages,
            "tools": tools,
            // Enable streaming mode
            "stream": true,
        });

        // Send the request (same as non-streaming)
        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("anthropic-beta", "prompt-caching-2024-07-31")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        // Check status
        if response.status() != StatusCode::OK {
            let status_code = response.status().as_u16();
            let error_text = response.text().await?;
            return Err(AgentError::InvalidResponse(format!(
                "API error {}: {}",
                status_code, error_text
            )));
        }

        // -----------------------------------------------------------------
        // GET THE BYTE STREAM
        // -----------------------------------------------------------------

        // Get the response body as a stream of bytes
        // .bytes_stream() returns a Stream<Item = Result<Bytes>>
        // Each item is a chunk of bytes from the response
        let byte_stream = response.bytes_stream();

        // Pin the parsed stream and return it
        //
        // Box::pin() - allocates on heap and pins the value
        // Self::parse_sse_stream() - our method that converts bytes to events
        Ok(Box::pin(Self::parse_sse_stream(byte_stream)))
    }
}

/*
 * ============================================================================
 * SSE STREAM PARSING (PRIVATE METHODS)
 * ============================================================================
 */

impl AnthropicClient {
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
    /// SSE format: `data: <json>`
    fn parse_sse_line(line: &str) -> Vec<StreamEvent> {
        let mut events = Vec::new();

        // SSE data lines start with "data: "
        if let Some(json_str) = line.strip_prefix("data: ") {
            // Check for end-of-stream marker
            if json_str == "[DONE]" {
                return events; // Signal end of stream
            }

            // Try to parse the JSON
            if let Ok(json) = serde_json::from_str::<Value>(json_str) {
                // Check the event type
                match json["type"].as_str() {
                    // -----------------------------------------------------
                    // TEXT DELTA
                    // -----------------------------------------------------
                    Some("content_block_delta") => {
                        // Extract text from delta
                        if let Some(text) = json["delta"]["text"].as_str() {
                            events.push(StreamEvent::TextDelta(text.to_string()));
                        }
                        // Extract thinking from delta (extended thinking feature)
                        if let Some(thinking) = json["delta"]["thinking"].as_str() {
                            events.push(StreamEvent::ThinkingDelta(thinking.to_string()));
                        }
                    }

                    // -----------------------------------------------------
                    // TOOL CALL START
                    // -----------------------------------------------------
                    Some("content_block_start") => {
                        // Check if it's a tool_use block
                        if let Some(block) = json["content_block"].as_object() {
                            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                // Extract tool ID and name
                                events.push(StreamEvent::ToolCallStart {
                                    // .unwrap() is safe here because Anthropic
                                    // guarantees these fields exist for tool_use
                                    id: block["id"].as_str().unwrap().to_string(),
                                    name: block["name"].as_str().unwrap().to_string(),
                                });
                            }
                        }
                    }

                    // -----------------------------------------------------
                    // TOOL CALL DONE
                    // -----------------------------------------------------
                    Some("content_block_stop") => {
                        if let Some(block) = json["content_block"].as_object() {
                            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                if let Some(id) = block["id"].as_str() {
                                    events.push(StreamEvent::ToolCallDone(id.to_string()));
                                }
                            }
                        }
                    }

                    // -----------------------------------------------------
                    // MESSAGE STOP
                    // -----------------------------------------------------
                    Some("message_stop") => {
                        if let Some(reason) = json["stop_reason"].as_str() {
                            events.push(StreamEvent::StopReason(reason.to_string()));
                        }
                    }

                    // -----------------------------------------------------
                    // MESSAGE START - contains initial token usage
                    // -----------------------------------------------------
                    Some("message_start") => {
                        if let Some(usage) = json["message"]["usage"].as_object() {
                            let input_tokens = usage
                                .get("input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32;
                            let output_tokens = usage
                                .get("output_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                                as u32;
                            events.push(StreamEvent::TokenUsage {
                                input_tokens,
                                output_tokens,
                            });
                        }
                    }

                    // -----------------------------------------------------
                    // MESSAGE DELTA - contains updated output token usage
                    // -----------------------------------------------------
                    Some("message_delta") => {
                        if let Some(usage) = json["usage"].as_object() {
                            let output_tokens = usage
                                .get("output_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                                as u32;
                            // Input tokens are not provided in message_delta
                            events.push(StreamEvent::TokenUsage {
                                input_tokens: 0,
                                output_tokens,
                            });
                        }
                    }

                    // Ignore other event types
                    _ => {}
                }
            }
        }

        // Return all collected events
        events
    }
}
