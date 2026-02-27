//! # Stream Handler
//!
//! Handles GET /stream requests for real-time agent execution updates using SSE.

use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    response::sse::{Event, Sse},
};
use futures::stream::{Stream, StreamExt};
use serde::Serialize;

use crate::agent::events::AgentEvent;
use crate::agent::messages::Message;
use crate::api::http::AppState;
use crate::client::LLMClient;

#[derive(Debug, serde::Deserialize)]
pub struct StreamQuery {
    pub message: String,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct TextEvent {
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ToolStartEvent {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct ToolDoneEvent {
    pub id: String,
    pub name: String,
    pub output: String,
    pub is_error: bool,
}

#[derive(Debug, Serialize)]
pub struct DoneEvent {
    pub stop_reason: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorEvent {
    pub message: String,
}

type BoxedSseStream = Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>;

/// Stream agent execution events via SSE.
pub async fn stream<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Query(params): Query<StreamQuery>,
) -> Sse<BoxedSseStream> {
    // In a supervisor context, "streaming" might mean streaming from a temporary worker
    // or just using the base agent. For now, we'll implement it by creating a temporary
    // agent using the supervisor's client/config.
    
    // NOTE: In a real production system, you might want to look up a persistent worker.
    let agent = crate::agent::loop_agent::AgentBuilder::new(
        state.supervisor.client().clone(),
        state.supervisor.config().clone()
    )
    .with_default_tools()
    .build();

    create_sse_stream(agent, &params.message).await
}

async fn create_sse_stream<C: LLMClient + Clone + 'static>(
    agent: crate::agent::loop_agent::Agent<C>, 
    message: &str
) -> Sse<BoxedSseStream> {
    // Add to history
    {
        let h_arc = agent.history();
        let mut h = h_arc.write().await;
        h.push(Message::user(message));
    }

    let stream = agent.run_stream();

    let sse_stream = stream.filter_map(|event| async move {
        match event {
            Ok(AgentEvent::TextDelta { delta }) => Some(Ok(Event::default()
                .event("text")
                .json_data(TextEvent { content: delta })
                .unwrap())),
            Ok(AgentEvent::ToolStart { id, name }) => Some(Ok(Event::default()
                .event("tool_start")
                .json_data(ToolStartEvent { id, name })
                .unwrap())),
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
            Ok(AgentEvent::Done { .. }) => Some(Ok(Event::default()
                .event("done")
                .json_data(DoneEvent {
                    stop_reason: "end_turn".to_string(),
                })
                .unwrap())),
            Ok(AgentEvent::Error { message }) => Some(Ok(Event::default()
                .event("error")
                .json_data(ErrorEvent { message })
                .unwrap())),
            Ok(AgentEvent::ToolInputDelta { .. }) => None,
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
