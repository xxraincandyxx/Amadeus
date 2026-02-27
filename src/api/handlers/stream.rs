//! # Stream Handler
//!
//! Handles GET /stream requests for real-time agent execution updates using SSE.

use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;

use axum::{
    extract::Query,
    response::sse::{Event, Sse},
};
use futures::stream::{Stream, StreamExt};
use serde::Serialize;

use crate::agent::config::{Config, Provider};
use crate::agent::events::AgentEvent;
use crate::agent::loop_agent::Agent;
use crate::agent::messages::Message;
use crate::client::{AnthropicClient, OpenAIClient};

/// Stream agent execution events via Server-Sent Events (SSE).
///
/// This endpoint provides real-time feedback as the agent thinks,
/// uses tools, and receives results. It is ideal for UI applications
/// that want to show a live typing effect and tool execution progress.
///
/// ### Events
///
/// | Event Name | Payload Type | Description |
/// |------------|--------------|-------------|
/// | `text` | [`TextEvent`] | Partial text response delta |
/// | `tool_start` | [`ToolStartEvent`] | Agent started using a tool |
/// | `tool_done` | [`ToolDoneEvent`] | Tool execution finished |
/// | `done` | [`DoneEvent`] | Agent loop completed |
/// | `error` | [`ErrorEvent`] | An error occurred during execution |
///
/// ### Request
///
/// - **Method:** GET
/// - **Path:** /stream
/// - **Query Params:** [`StreamQuery`]
///
/// ### Example
///
/// ```bash
/// curl -N http://localhost:3000/stream?message=hello
/// ```
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

pub async fn stream(Query(params): Query<StreamQuery>) -> Sse<BoxedSseStream> {
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            let error_stream = Box::pin(futures::stream::iter(vec![Ok(Event::default()
                .event("error")
                .json_data(ErrorEvent {
                    message: e.to_string(),
                })
                .unwrap())]));
            return Sse::new(error_stream);
        }
    };

    let config = Arc::new(config);

    match config.provider {
        Provider::Anthropic => {
            let client = AnthropicClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            let agent = Agent::new(client, config);
            create_sse_stream(agent, &params.message).await
        }
        Provider::OpenAI => {
            let client = OpenAIClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            let agent = Agent::new(client, config);
            create_sse_stream(agent, &params.message).await
        }
    }
}

async fn create_sse_stream<C>(agent: Agent<C>, message: &str) -> Sse<BoxedSseStream>
where
    C: crate::client::LLMClient + Clone + 'static,
{
    // Add to history
    {
        let mut h = agent.history().write().await;
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
