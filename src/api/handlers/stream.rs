//! # Stream Handler
//!
//! Handles GET /stream requests for real-time agent execution updates using SSE.
//!
//! ## Overview
//!
//! This handler provides a Server-Sent Events (SSE) interface for real-time
//! interaction with the agent. It allows clients to receive immediate feedback
//! as the agent generates text and executes tools, which is ideal for
//! interactive UI applications.
//!
//! ## Event Protocol
//!
//! The handler streams the following event types:
//!
//! | Event | Payload | Description |
//! |-------|---------|-------------|
//! | `text` | [`TextEvent`] | A chunk of generated text |
//! | `tool_start` | [`ToolStartEvent`] | Notification that a tool is starting |
//! | `tool_done` | [`ToolDoneEvent`] | Results of a tool execution |
//! | `done` | [`DoneEvent`] | Final termination reason |
//! | `error` | [`ErrorEvent`] | Critical failure notification |

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;

// Axum types for SSE and extraction
use axum::{
    extract::{Query, State},
    response::sse::{Event, Sse},
};

// Async streaming utilities
use futures::stream::{Stream, StreamExt};
use serde::Serialize;

// Internal SDK components
use crate::agent::events::AgentEvent;
use crate::agent::messages::Message;
use crate::api::http::AppState;
use crate::client::LLMClient;

/*
 * ============================================================================
 * DATA STRUCTURES
 * ============================================================================
 */

/// Query parameters for the stream endpoint.
#[derive(Debug, serde::Deserialize)]
pub struct StreamQuery {
    /// The user's input message.
    pub message: String,
    /// Optional timeout for tool execution.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// Payload for the `text` event.
#[derive(Debug, Serialize)]
pub struct TextEvent {
    /// The incremental text delta.
    pub content: String,
}

/// Payload for the `tool_start` event.
#[derive(Debug, Serialize)]
pub struct ToolStartEvent {
    /// Unique ID for the tool call.
    pub id: String,
    /// Name of the tool being executed.
    pub name: String,
}

/// Payload for the `tool_done` event.
#[derive(Debug, Serialize)]
pub struct ToolDoneEvent {
    /// Unique ID for the tool call.
    pub id: String,
    /// Name of the tool.
    pub name: String,
    /// The execution output (truncated if large).
    pub output: String,
    /// Whether the tool execution failed.
    pub is_error: bool,
}

/// Payload for the `done` event.
#[derive(Debug, Serialize)]
pub struct DoneEvent {
    /// Why the agent finished (e.g., "end_turn").
    pub stop_reason: String,
}

/// Payload for the `error` event.
#[derive(Debug, Serialize)]
pub struct ErrorEvent {
    /// Human-readable error message.
    pub message: String,
}

/// Type alias for the complex SSE stream return type.
type BoxedSseStream = Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>;

/*
 * ============================================================================
 * HANDLERS
 * ============================================================================
 */

/// GET /stream
///
/// Initiates a real-time event stream for an agent conversation.
/// This handler creates a temporary agent using the Supervisor's core client
/// and configuration, ensuring consistent behavior with the rest of the SDK.
pub async fn stream<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Query(params): Query<StreamQuery>,
) -> Sse<BoxedSseStream> {
    // -------------------------------------------------------------------------
    // INITIALIZE TEMPORARY AGENT
    // -------------------------------------------------------------------------
    // We use the AgentBuilder to reconstruct an agent with the global SDK config.
    // In a multi-agent context, this represents a "primary" agent interaction.
    let agent = crate::agent::loop_agent::AgentBuilder::new(
        state.supervisor.client().clone(),
        state.supervisor.config().clone()
    )
    .with_default_tools()
    .build();

    create_sse_stream(agent, &params.message).await
}

/*
 * ============================================================================
 * STREAM LOGIC
 * ============================================================================
 */

/// Internal helper to transform an `AgentEvent` stream into an Axum SSE stream.
///
/// This function manages the conversation history injection and orchestrates
/// the mapping between internal SDK events and public API SSE tokens.
async fn create_sse_stream<C: LLMClient + Clone + 'static>(
    agent: crate::agent::loop_agent::Agent<C>, 
    message: &str
) -> Sse<BoxedSseStream> {
    // 1. Inject initial user message into history
    {
        let h_arc = agent.history();
        let mut h = h_arc.write().await;
        h.push(Message::user(message));
    }

    // 2. Start the agent loop in streaming mode
    let stream = agent.run_stream();

    // 3. Map SDK events to SSE events
    let sse_stream = stream.filter_map(|event| async move {
        match event {
            // Text delta received from LLM
            Ok(AgentEvent::TextDelta { delta }) => Some(Ok(Event::default()
                .event("text")
                .json_data(TextEvent { content: delta })
                .unwrap())),
            
            // Tool execution initiated
            Ok(AgentEvent::ToolStart { id, name }) => Some(Ok(Event::default()
                .event("tool_start")
                .json_data(ToolStartEvent { id, name })
                .unwrap())),
            
            // Tool execution completed
            Ok(AgentEvent::ToolComplete {
                id,
                name,
                output,
                is_error,
                ..
            }) => Some(Ok(Event::default()
                .event("tool_done")
                .json_data(ToolDoneEvent {
                    id,
                    name,
                    output,
                    is_error,
                })
                .unwrap())),
            
            // Agent loop finished successfully
            Ok(AgentEvent::Done { .. }) => Some(Ok(Event::default()
                .event("done")
                .json_data(DoneEvent {
                    stop_reason: "end_turn".to_string(),
                })
                .unwrap())),
            
            // Critical agent error
            Ok(AgentEvent::Error { message }) => Some(Ok(Event::default()
                .event("error")
                .json_data(ErrorEvent { message })
                .unwrap())),
            
            // Intermediate deltas (e.g. tool arguments) are suppressed for public API
            Ok(AgentEvent::ToolInputDelta { .. }) => None,
            
            // Stream processing error
            Err(e) => Some(Ok(Event::default()
                .event("error")
                .json_data(ErrorEvent {
                    message: e.to_string(),
                })
                .unwrap())),
        }
    });

    Sse::new(Box::pin(sse_stream))
}
