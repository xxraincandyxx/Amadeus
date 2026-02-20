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

// Bytes - a container for contiguous byte data
// Used for parsing streaming responses
use bytes::Bytes;

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
        let client = Client::builder()
            // Maximum idle connections per host (connection pooling)
            .pool_max_idle_per_host(5)
            // How long to keep idle connections alive
            .pool_idle_timeout(Duration::from_secs(30))
            // Total request timeout (includes connection + response)
            .timeout(Duration::from_secs(120))
            // Connection establishment timeout
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
        let url = format!("{}/v1/messages", self.base_url);

        debug!(
            model = %self.model,
            messages = messages.len(),
            tools = tools.len(),
            "Creating Anthropic message"
        );

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

            warn!(
                status_code = status_code,
                error = %error_text,
                "Anthropic API error"
            );

            return Err(AgentError::InvalidResponse(format!(
                "API error {}: {}",
                status_code, error_text
            )));
        }

        let json: Value = response.json().await?;

        let stop_reason = json["stop_reason"].as_str().unwrap_or("").to_string();
        let content: Vec<ContentBlock> = serde_json::from_value(json["content"].clone())?;

        info!(
            stop_reason = %stop_reason,
            content_blocks = content.len(),
            "Anthropic response received"
        );

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

        // Build the request body with stream: true
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "system": system,
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
    ///
    /// SSE = Server-Sent Events
    /// A format where the server sends events like:
    ///   data: {"type": "content_block_delta", ...}
    ///   data: {"type": "message_stop", ...}
    fn parse_sse_stream(
        // The input stream of bytes
        // impl Stream means "any type that implements Stream"
        // + Unpin: the stream can be moved safely
        // + 'static: the stream has no borrowed references
        stream: impl Stream<Item = reqwest::Result<bytes::Bytes>> + Unpin + 'static,
    ) -> impl Stream<Item = Result<StreamEvent>> {
        // -----------------------------------------------------------------
        // USE UNFOLD TO CREATE THE OUTPUT STREAM
        // -----------------------------------------------------------------

        // futures::stream::unfold creates a stream from a state and a closure
        //
        // It's like a while loop that yields values:
        //
        //   let mut state = initial_state;
        //   loop {
        //       let (item, new_state) = closure(state).await;
        //       match item {
        //           Some(value) => yield value,
        //           None => break,  // End the stream
        //       }
        //       state = new_state;
        //   }
        //
        // Parameters:
        // - stream: the initial state (our byte stream)
        // - |mut s| async move { ... }: the closure that produces values

        futures::stream::unfold(stream, |mut s| async move {
            // Get the next chunk of bytes
            // .next() returns Option<...> from the stream
            let result = s.next().await;

            match result {
                // Got a chunk of bytes
                Some(Ok(bytes)) => {
                    // Try to parse it as an SSE event
                    if let Some(event) = Self::parse_sse_line(bytes) {
                        // Return the event and keep the stream
                        // Some((value, new_state)) yields value and continues
                        Some((Ok(event), s))
                    } else {
                        // No valid event - return empty text and continue
                        // This handles empty lines, comments, etc.
                        Some((Ok(StreamEvent::TextDelta(String::new())), s))
                    }
                }

                // Error from the byte stream
                Some(Err(e)) => Some((Err(AgentError::Api(e.to_string())), s)),

                // Stream ended (None from .next())
                None => None, // None ends the unfold stream
            }
        })
    }

    /// Parse a single SSE line into a StreamEvent.
    ///
    /// SSE format: `data: <json>\n`
    fn parse_sse_line(bytes: Bytes) -> Option<StreamEvent> {
        // Convert bytes to string (lossy - replace invalid UTF-8)
        let text = String::from_utf8_lossy(&bytes);

        // Process each line
        for line in text.lines() {
            // SSE data lines start with "data: "
            if let Some(json_str) = line.strip_prefix("data: ") {
                // Extract the JSON part (skip "data: ")
                // &line[6..] is string slicing: skip first 6 characters
                // Check for end-of-stream marker
                if json_str == "[DONE]" {
                    return None; // Signal end of stream
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
                                return Some(StreamEvent::TextDelta(text.to_string()));
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
                                    return Some(StreamEvent::ToolCallStart {
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
                                        return Some(StreamEvent::ToolCallDone(id.to_string()));
                                    }
                                }
                            }
                        }

                        // -----------------------------------------------------
                        // MESSAGE STOP
                        // -----------------------------------------------------
                        Some("message_stop") => {
                            if let Some(reason) = json["stop_reason"].as_str() {
                                return Some(StreamEvent::StopReason(reason.to_string()));
                            }
                        }

                        // Ignore other event types
                        _ => {}
                    }
                }
            }
        }

        // No valid event found
        None
    }
}
