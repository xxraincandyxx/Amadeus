//! # HTTP Request/Response Types
//!
//! JSON types for the REST API endpoints. These types define the
//! structure of requests and responses for the HTTP API.
//!
//! ## Design Principles
//!
//! 1. **Simple and flat** - Easy to serialize/deserialize
//! 2. **Optional fields** - Use `Option<T>` for optional parameters
//! 3. **Clear naming** - Field names match API parameter names
//! 4. **Documented** - Each field has a purpose description
//!
//! ## Type Organization
//!
//! ```text
//! Request Types          Response Types
//! ─────────────          ──────────────
//! ChatRequest      →     ChatResponse
//! ExecuteRequest   →     ExecuteResponse
//! StreamRequest    →     SSE events
//! (health check)   →     HealthResponse
//! ```
//!
//! ## JSON Examples
//!
//! ### ChatRequest
//! ```json
//! {
//!   "message": "List files in the src directory",
//!   "timeout_secs": 60,
//!   "stream": false
//! }
//! ```
//!
//! ### ExecuteRequest
//! ```json
//! {
//!   "command": "ls -la src/",
//!   "timeout_secs": 30
//! }
//! ```

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

// Serde traits for JSON serialization/deserialization
//
// Serialize: Converts Rust struct TO JSON (for responses)
// Deserialize: Converts JSON TO Rust struct (for requests)
use serde::{Deserialize, Serialize};

/*
 * ============================================================================
 * CHAT ENDPOINT TYPES
 * ============================================================================
 */

/// Request body for the `/chat` endpoint.
///
/// Sends a message to the agent and receives a response.
/// The agent will use the configured LLM provider and tools.
///
/// # Example
///
/// ```json
/// {
///   "message": "What files are in the src directory?",
///   "timeout_secs": 60,
///   "stream": false
/// }
/// ```
///
/// # Fields
///
/// - `message`: The user's prompt/question
/// - `timeout_secs`: Optional timeout for command execution (default: 300)
/// - `stream`: Whether to use streaming (default: false, use `/stream` for SSE)
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    /// The message to send to the agent.
    ///
    /// This is the user's prompt or question.
    /// The agent will process this and potentially execute tools.
    ///
    /// # Example
    ///
    /// "List all Rust files in the project"
    pub message: String,

    /// Timeout for tool execution in seconds.
    ///
    /// Optional. Defaults to 300 seconds (5 minutes) if not specified.
    /// This timeout applies to bash command execution, not LLM API calls.
    ///
    /// # Example
    ///
    /// - `60` - 1 minute timeout
    /// - `300` - 5 minute timeout (default)
    /// - `3600` - 1 hour timeout
    #[serde(default)]
    pub timeout_secs: Option<u64>,

    /// Whether to stream the response.
    ///
    /// Optional. Defaults to `false`.
    ///
    /// - `false`: Wait for complete response (use ChatResponse)
    /// - `true`: Use Server-Sent Events (SSE) for streaming
    ///
    /// Note: For streaming, prefer the dedicated `/stream` endpoint.
    #[serde(default)]
    pub stream: Option<bool>,
}

/// Response body for the `/chat` endpoint.
///
/// Contains the agent's response after processing the message.
/// May include text content and/or tool call information.
///
/// # Example
///
/// ```json
/// {
///   "content": "I found 3 Rust files in the project:\n...",
///   "tool_calls": [],
///   "stop_reason": "end_turn"
/// }
/// ```
#[derive(Debug, Serialize)]
pub struct ChatResponse {
    /// The text content of the agent's response.
    ///
    /// This is the main output from the agent.
    /// May be empty if the agent only made tool calls.
    pub content: String,

    /// Tool calls made during processing.
    ///
    /// List of tools the agent executed.
    /// Each entry includes the tool name and command.
    pub tool_calls: Vec<ToolCallInfo>,

    /// Why the agent stopped generating.
    ///
    /// Common values:
    /// - `"end_turn"` - Agent finished its response
    /// - `"tool_use"` - Agent is waiting for tool execution
    /// - `"max_tokens"` - Agent hit token limit
    pub stop_reason: String,
}

/// Information about a tool call in the response.
///
/// Represents a single tool invocation made by the agent.
#[derive(Debug, Serialize)]
pub struct ToolCallInfo {
    /// Unique identifier for this tool call.
    ///
    /// Used to correlate tool calls with their results.
    pub id: String,

    /// Name of the tool that was called.
    ///
    /// Currently always "bash".
    pub name: String,

    /// The command that was executed.
    ///
    /// For the bash tool, this is the shell command.
    pub command: String,

    /// The output from the tool execution.
    ///
    /// Combined stdout and stderr.
    pub output: String,
}

/*
 * ============================================================================
 * EXECUTE ENDPOINT TYPES
 * ============================================================================
 */

/// Request body for the `/execute` endpoint.
///
/// Executes a bash command directly without LLM involvement.
/// Useful for direct tool access.
///
/// # Example
///
/// ```json
/// {
///   "command": "ls -la",
///   "timeout_secs": 30
/// }
/// ```
#[derive(Debug, Deserialize)]
pub struct ExecuteRequest {
    /// The shell command to execute.
    ///
    /// Executed via `sh -c` in the working directory.
    /// Supports full shell syntax (pipes, redirects, etc.).
    ///
    /// # Example
    ///
    /// - `"ls -la"` - List files
    /// - `"cat file.txt | grep pattern"` - Pipeline
    /// - `"mkdir -p dir/subdir"` - Create directories
    pub command: String,

    /// Timeout for command execution in seconds.
    ///
    /// Optional. Defaults to 30 seconds if not specified.
    /// If the command runs longer, it will be killed and
    /// `timed_out` will be `true` in the response.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// Response body for the `/execute` endpoint.
///
/// Contains the result of the bash command execution.
///
/// # Example
///
/// ```json
/// {
///   "output": "total 24\ndrwxr-xr-x  5 user ...\n",
///   "exit_code": 0,
///   "timed_out": false
/// }
/// ```
#[derive(Debug, Serialize)]
pub struct ExecuteResponse {
    /// Combined stdout and stderr from the command.
    ///
    /// This is the text output from the command execution.
    /// May be empty if the command produced no output.
    pub output: String,

    /// Exit code from the command.
    ///
    /// - `0` - Success
    /// - `1-255` - Error (command-specific meaning)
    /// - `-1` - Command failed to start or was killed
    pub exit_code: i32,

    /// Whether the command timed out.
    ///
    /// `true` if the command exceeded `timeout_secs`.
    /// When `true`, `output` may contain partial results.
    pub timed_out: bool,
}

/*
 * ============================================================================
 * HEALTH ENDPOINT TYPES
 * ============================================================================
 */

/// Response body for the `/health` endpoint.
///
/// Simple health check to verify the server is running.
///
/// # Example
///
/// ```json
/// {
///   "status": "ok",
///   "version": "0.1.0"
/// }
/// ```
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Health status.
    ///
    /// Always "ok" when the server is healthy.
    pub status: String,

    /// Server version.
    ///
    /// The crate version from Cargo.toml.
    pub version: String,
}

/*
 * ============================================================================
 * ERROR RESPONSE TYPES
 * ============================================================================
 */

/// Error response for API errors.
///
/// Returned when a request fails.
///
/// # Example
///
/// ```json
/// {
///   "error": "AgentError",
///   "message": "Command timed out after 30s"
/// }
/// ```
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Error type name.
    ///
    /// Short identifier for the error type.
    pub error: String,

    /// Human-readable error message.
    ///
    /// Detailed description of what went wrong.
    pub message: String,
}

impl ErrorResponse {
    /// Create a new error response.
    ///
    /// # Arguments
    ///
    /// * `error` - Error type name
    /// * `message` - Error description
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
        }
    }

    /// Create from an AgentError.
    ///
    /// Converts the agent error into an API error response.
    pub fn from_agent_error(err: &crate::error::AgentError) -> Self {
        Self {
            error: "AgentError".to_string(),
            message: err.to_string(),
        }
    }
}
