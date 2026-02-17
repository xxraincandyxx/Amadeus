//! # Stream Handler
//!
//! Handles GET /stream requests for SSE (Server-Sent Events) streaming.
//!
//! ## Endpoint
//!
//! `GET /stream?message=...`
//!
//! ## Query Parameters
//!
//! - `message`: The message to send (required)
//! - `timeout_secs`: Timeout in seconds (optional, default: 300)
//!
//! ## Response
//!
//! Returns a stream of SSE events:
//!
//! ```text
//! event: text
//! data: {"content": "Hello"}
//!
//! event: tool_start
//! data: {"id": "call_123", "name": "bash"}
//!
//! event: done
//! data: {"stop_reason": "end_turn"}
//! ```
//!
//! ## Purpose
//!
//! Provides real-time streaming responses from the agent.
//! Useful for:
//! - Interactive UI with live updates
//! - Long-running operations
//! - Progress feedback

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

// Axum types for HTTP handling
//
// Query: Extract query parameters from URL
// Sse: Server-Sent Events response type
use axum::{
    extract::Query,
    response::sse::{Event, Sse},
};

// Serde for JSON serialization
use serde::Serialize;

// Standard library types
use std::convert::Infallible;

// Futures stream utilities
use futures::stream::{self, Stream};

// Client stream event type (for conversion helper)
use crate::client::StreamEvent;

/*
 * ============================================================================
 * QUERY PARAMETERS
 * ============================================================================
 */

/// Query parameters for the stream endpoint.
///
/// Extracted from the URL query string.
#[derive(Debug, serde::Deserialize)]
pub struct StreamQuery {
    /// The message to send to the agent.
    pub message: String,

    /// Timeout for tool execution in seconds.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/*
 * ============================================================================
 * SSE EVENT TYPES
 * ============================================================================
 */

/// Text content event.
///
/// Emitted when the agent generates text.
#[derive(Debug, Serialize)]
pub struct TextEvent {
    pub content: String,
}

/// Tool call start event.
///
/// Emitted when the agent starts a tool call.
#[derive(Debug, Serialize)]
pub struct ToolStartEvent {
    pub id: String,
    pub name: String,
}

/// Tool call complete event.
///
/// Emitted when a tool call finishes.
#[derive(Debug, Serialize)]
pub struct ToolDoneEvent {
    pub id: String,
    pub output: String,
}

/// Stream done event.
///
/// Emitted when the stream ends.
#[derive(Debug, Serialize)]
pub struct DoneEvent {
    pub stop_reason: String,
}

/*
 * ============================================================================
 * HANDLER FUNCTION
 * ============================================================================
 */

/// Handle GET /stream requests.
///
/// Returns a Server-Sent Events stream with real-time updates.
///
/// # Query Parameters
///
/// - `message`: The user's prompt (required)
/// - `timeout_secs`: Timeout for tools (optional, default: 300)
///
/// # Response
///
/// An SSE stream with events:
/// - `text`: Text content chunk
/// - `tool_start`: Tool call started
/// - `tool_done`: Tool call completed
/// - `done`: Stream finished
///
/// # Example
///
/// ```bash
/// curl -N "http://localhost:3000/stream?message=list%20files"
/// ```
///
/// # Note
///
/// This is a simplified implementation. A full implementation would:
/// - Connect to the actual agent streaming API
/// - Handle tool execution mid-stream
/// - Support cancellation
pub async fn stream(
    // Extract query parameters from URL
    Query(params): Query<StreamQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // -------------------------------------------------------------------------
    // CREATE MOCK STREAM
    // -------------------------------------------------------------------------

    // NOTE: This is a simplified mock implementation.
    // A full implementation would:
    // 1. Load config and create appropriate client
    // 2. Create agent with streaming enabled
    // 3. Call agent.run_streaming() to get real events
    // 4. Convert StreamEvents to SSE events
    //
    // For now, we return a simple acknowledgment stream

    let message = params.message.clone();

    // Create a stream of mock events
    //
    // In a real implementation, this would come from the agent
    let events = vec![
        // Text event with acknowledgment
        Event::default()
            .event("text")
            .json_data(TextEvent {
                content: format!("Processing: {}", message),
            })
            .unwrap(),
        // Done event
        Event::default()
            .event("done")
            .json_data(DoneEvent {
                stop_reason: "end_turn".to_string(),
            })
            .unwrap(),
    ];

    // Convert to a stream
    let stream = stream::iter(events.into_iter().map(Ok));

    // Return as SSE response
    //
    // Sse wraps the stream and sets appropriate headers:
    // - Content-Type: text/event-stream
    // - Cache-Control: no-cache
    // - Connection: keep-alive
    Sse::new(stream)
}

/*
 * ============================================================================
 * HELPER FUNCTIONS (for future full implementation)
 * ============================================================================
 */

/// Convert a StreamEvent to an SSE Event.
///
/// This would be used in a full implementation to convert
/// the agent's internal stream events to SSE format.
#[allow(dead_code)]
fn stream_event_to_sse(event: StreamEvent) -> Option<Event> {
    match event {
        StreamEvent::TextDelta(text) => Some(
            Event::default()
                .event("text")
                .json_data(TextEvent { content: text })
                .unwrap(),
        ),
        StreamEvent::ToolCallStart { id, name } => Some(
            Event::default()
                .event("tool_start")
                .json_data(ToolStartEvent { id, name })
                .unwrap(),
        ),
        StreamEvent::ToolCallDone(id) => {
            // In a full implementation, we'd have the output
            Some(
                Event::default()
                    .event("tool_done")
                    .json_data(ToolDoneEvent {
                        id,
                        output: String::new(),
                    })
                    .unwrap(),
            )
        }
        StreamEvent::StopReason(reason) => Some(
            Event::default()
                .event("done")
                .json_data(DoneEvent {
                    stop_reason: reason,
                })
                .unwrap(),
        ),
        StreamEvent::ToolCallDelta { .. } => {
            // Delta events don't produce standalone SSE events
            None
        }
    }
}

/*
 * ============================================================================
 * TESTS
 * ============================================================================
 */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_event_serialization() {
        let event = TextEvent {
            content: "Hello, world!".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Hello, world!"));
    }
}
