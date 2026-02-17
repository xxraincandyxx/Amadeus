//! # Chat Handler
//!
//! Handles POST /chat requests to send messages to the agent.
//!
//! ## Endpoint
//!
//! `POST /chat`
//!
//! ## Request Body
//!
//! ```json
//! {
//!   "message": "List files in src/",
//!   "timeout_secs": 60,
//!   "stream": false
//! }
//! ```
//!
//! ## Response
//!
//! ```json
//! {
//!   "content": "I found the following files...",
//!   "tool_calls": [],
//!   "stop_reason": "end_turn"
//! }
//! ```
//!
//! ## How It Works
//!
//! 1. Parse the request body (ChatRequest)
//! 2. Load configuration from environment
//! 3. Create appropriate LLM client (Anthropic/OpenAI)
//! 4. Create agent with timeout settings
//! 5. Run agent with the message
//! 6. Return response (ChatResponse)

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

// Axum types for HTTP handling
//
// Json: Extract JSON from request, serialize response
use axum::Json;

// Standard library types
use std::sync::Arc;
use tokio::sync::RwLock;

// Request and response types
use crate::api::types::{ChatRequest, ChatResponse, ErrorResponse};

// Agent types
use crate::agent::config::{Config, Provider};
use crate::agent::loop_agent::Agent;

// Client types
use crate::client::{AnthropicClient, OpenAIClient};

/*
 * ============================================================================
 * HANDLER FUNCTION
 * ============================================================================
 */

/// Handle POST /chat requests.
///
/// Sends a message to the agent and returns the response.
///
/// # Request Body
///
/// - `message`: The user's prompt (required)
/// - `timeout_secs`: Timeout for tool execution (optional, default: 300)
/// - `stream`: Whether to stream response (optional, not implemented here)
///
/// # Response
///
/// - `content`: The agent's text response
/// - `tool_calls`: List of tools executed
/// - `stop_reason`: Why generation stopped
///
/// # Errors
///
/// Returns `ErrorResponse` if:
/// - Configuration cannot be loaded
/// - Agent execution fails
/// - Request body is invalid
///
/// # Example
///
/// ```bash
/// curl -X POST http://localhost:3000/chat \
///   -H "Content-Type: application/json" \
///   -d '{"message": "list files", "timeout_secs": 60}'
/// ```
pub async fn chat(
    // Json extractor parses the request body into ChatRequest
    // If parsing fails, axum returns a 400 Bad Request automatically
    Json(request): Json<ChatRequest>,
) -> std::result::Result<Json<ChatResponse>, Json<ErrorResponse>> {
    // -------------------------------------------------------------------------
    // LOAD CONFIGURATION
    // -------------------------------------------------------------------------

    // Load configuration from environment variables
    //
    // Config::load() reads:
    // - PROVIDER (anthropic or openai)
    // - ANTHROPIC_API_KEY or OPENAI_API_KEY
    // - MODEL_ID (optional)
    // - USE_STREAMING (optional)
    //
    // This may fail if required env vars are missing
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => return Err(Json(ErrorResponse::new("ConfigError", e.to_string()))),
    };

    // -------------------------------------------------------------------------
    // DETERMINE TIMEOUT
    // -------------------------------------------------------------------------

    // Use request timeout or default (300 seconds = 5 minutes)
    //
    // This timeout applies to bash command execution, not LLM API calls
    let timeout_secs = request.timeout_secs.unwrap_or(300);

    // -------------------------------------------------------------------------
    // GET WORKDIR AS STRING
    // -------------------------------------------------------------------------

    // config.workdir is a PathBuf, convert to String
    let workdir = config.workdir.to_string_lossy().to_string();

    // -------------------------------------------------------------------------
    // CREATE CLIENT BASED ON PROVIDER
    // -------------------------------------------------------------------------

    // We need to create the agent differently based on the provider
    // because Agent<C> is generic over the client type.
    //
    // The match branches both produce the same ChatResponse type,
    // so we can return them from the same function.
    match config.provider {
        // ---------------------------------------------------------------------
        // ANTHROPIC PROVIDER
        // ---------------------------------------------------------------------
        Provider::Anthropic => {
            // Create the Anthropic client
            //
            // AnthropicClient::new takes:
            // - api_key: The API key from environment
            // - base_url: Optional custom URL (None = use default)
            // - model: The model identifier
            let client = AnthropicClient::new(config.api_key, config.base_url, config.model);

            // Create the agent
            //
            // Agent::new takes:
            // - client: The LLM client (generic)
            // - workdir: Working directory for commands
            // - timeout_secs: Timeout for bash execution
            // - use_streaming: Whether to stream responses
            let agent = Agent::new(client, workdir, timeout_secs, config.use_streaming);

            // Run the agent
            //
            // agent.run takes:
            // - prompt: The user's message
            // - history: Shared conversation history (empty for new conversations)
            run_agent(agent, &request.message).await
        }

        // ---------------------------------------------------------------------
        // OPENAI PROVIDER
        // ---------------------------------------------------------------------
        Provider::OpenAI => {
            // Create the OpenAI client
            //
            // Same parameters as AnthropicClient
            let client = OpenAIClient::new(config.api_key, config.base_url, config.model);

            // Create the agent with OpenAI client
            let agent = Agent::new(client, workdir, timeout_secs, config.use_streaming);

            // Run the agent
            run_agent(agent, &request.message).await
        }
    }
}

/*
 * ============================================================================
 * HELPER FUNCTIONS
 * ============================================================================
 */

/// Run the agent and build the response.
///
/// This is a generic helper that works with any LLMClient type.
/// It handles the common logic of running the agent and extracting
/// the response.
///
/// # Type Parameters
///
/// * `C` - The LLM client type (must implement LLMClient)
///
/// # Arguments
///
/// * `agent` - The agent instance
/// * `message` - The user's message
///
/// # Returns
///
/// A ChatResponse with the agent's output.
async fn run_agent<C>(
    agent: Agent<C>,
    message: &str,
) -> std::result::Result<Json<ChatResponse>, Json<ErrorResponse>>
where
    C: crate::client::LLMClient,
{
    // -------------------------------------------------------------------------
    // CREATE EMPTY HISTORY
    // -------------------------------------------------------------------------

    // For each request, we start with an empty history.
    //
    // This makes each request stateless - the caller is responsible
    // for maintaining conversation context if desired.
    //
    // In the future, we could add a session_id field to ChatRequest
    // and maintain history per session.
    let history = Arc::new(RwLock::new(Vec::new()));

    // -------------------------------------------------------------------------
    // RUN THE AGENT
    // -------------------------------------------------------------------------

    // Run the agent with the user's message
    //
    // This will:
    // 1. Send the message to the LLM
    // 2. If LLM calls a tool, execute it
    // 3. Send tool results back to LLM
    // 4. Repeat until LLM returns text
    //
    // The result is the final text response
    match agent.run(message, history).await {
        Ok(content) => {
            // Build the successful response
            //
            // Currently, we don't have access to tool call details from agent.run()
            // In a future version, we could modify agent.run() to return this info
            Ok(Json(ChatResponse {
                content,
                tool_calls: Vec::new(), // TODO: Extract from history
                stop_reason: "end_turn".to_string(),
            }))
        }
        Err(e) => Err(Json(ErrorResponse::from_agent_error(&e))),
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
