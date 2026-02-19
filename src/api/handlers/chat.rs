//! # Chat Handler
//!
//! Handles POST /chat requests to send messages to the agent.

use std::sync::Arc;

use axum::Json;
use tokio::sync::RwLock;

use crate::agent::config::{Config, Provider};
use crate::agent::loop_agent::Agent;
use crate::api::types::{ChatRequest, ChatResponse, ErrorResponse};
use crate::client::{AnthropicClient, OpenAIClient};

/// Process a chat request and return the agent's response.
///
/// This endpoint initializes a new agent instance for each request,
/// executes the ReAct loop until completion, and returns the final
/// text and all intermediate tool calls.
///
/// ### Request
///
/// - **Method:** POST
/// - **Path:** /chat
/// - **Body:** [`ChatRequest`]
///
/// ### Response
///
/// - **Success:** 200 OK with [`ChatResponse`]
/// - **Error:** 400/500 with [`ErrorResponse`]
///
/// ### Example
///
/// ```bash
/// curl -X POST http://localhost:3000/chat \
///   -H "Content-Type: application/json" \
///   -d '{"message": "What files are in the current directory?"}'
/// ```
pub async fn chat(
    Json(request): Json<ChatRequest>,
) -> std::result::Result<Json<ChatResponse>, Json<ErrorResponse>> {
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => return Err(Json(ErrorResponse::new("ConfigError", e.to_string()))),
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
            run_agent(agent, &request.message).await
        }

        Provider::OpenAI => {
            let client = OpenAIClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            let agent = Agent::new(client, config);
            run_agent(agent, &request.message).await
        }
    }
}

/// Internal helper to run the agent loop.
async fn run_agent<C>(
    agent: Agent<C>,
    message: &str,
) -> std::result::Result<Json<ChatResponse>, Json<ErrorResponse>>
where
    C: crate::client::LLMClient + Clone + 'static,
{
    let history = Arc::new(RwLock::new(Vec::new()));

    match agent.run(message, history).await {
        Ok(result) => Ok(Json(ChatResponse {
            content: result.text,
            tool_calls: result
                .tool_calls
                .into_iter()
                .map(|tc| crate::api::types::ToolCall {
                    name: tc.name,
                    input: tc.input,
                    output: tc.output,
                })
                .collect(),
            stop_reason: "end_turn".to_string(),
        })),
        Err(e) => Err(Json(ErrorResponse::from_agent_error(&e))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_request_deserialization() {
        let json = r#"{"message": "hello", "timeout_secs": 60}"#;
        let request: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.message, "hello");
        assert_eq!(request.timeout_secs, Some(60));
        assert_eq!(request.stream, None);
    }

    #[test]
    fn test_chat_request_defaults() {
        let json = r#"{"message": "hello"}"#;
        let request: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.timeout_secs, None);
        assert_eq!(request.stream, None);
    }
}
