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

async fn run_agent<C>(
    agent: Agent<C>,
    message: &str,
) -> std::result::Result<Json<ChatResponse>, Json<ErrorResponse>>
where
    C: crate::client::LLMClient,
{
    let history = Arc::new(RwLock::new(Vec::new()));

    match agent.run(message, history).await {
        Ok(content) => Ok(Json(ChatResponse {
            content,
            tool_calls: Vec::new(),
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
