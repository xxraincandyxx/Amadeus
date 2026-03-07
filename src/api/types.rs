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
    /// Each entry includes the tool name, input, and output.
    pub tool_calls: Vec<ToolCall>,

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
pub struct ToolCall {
    /// Name of the tool that was called.
    pub name: String,

    /// The input to the tool.
    pub input: serde_json::Value,

    /// The output from the tool execution.
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
 * SUPERVISOR ENDPOINT TYPES
 * ============================================================================
 */

/// Request body for the `/tasks` endpoint.
///
/// Dispatches a task to the multi-agent supervisor.
#[derive(Debug, Deserialize)]
pub struct TaskRequest {
    /// Unique ID for the task.
    pub id: String,
    /// The prompt/instruction for the task.
    pub prompt: String,
    /// List of required capabilities for workers.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// Response body for the `/tasks` endpoint.
///
/// Contains the result of a multi-agent task execution.
#[derive(Debug, Serialize)]
pub struct TaskResponse {
    pub task_id: String,
    pub worker_id: String,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
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
#[derive(Debug, Serialize, Deserialize)]
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
///   "error": "Timeout",
///   "message": "Operation timed out after 30s",
///   "tool": "bash"
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

    /// Tool name if error is tool-related.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,

    /// Seconds to wait before retrying (for rate limiting).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<u64>,
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
            tool: None,
            retry_after: None,
        }
    }

    /// Create from an AgentError.
    ///
    /// Converts the agent error into an API error response
    /// with structured context information.
    pub fn from_agent_error(err: &crate::error::AgentError) -> Self {
        use crate::error::AgentError;

        match err {
            AgentError::ToolInput { tool, reason } => Self {
                error: "ToolInputError".to_string(),
                message: reason.clone(),
                tool: Some(tool.clone()),
                retry_after: None,
            },
            AgentError::Timeout(secs) => Self {
                error: "Timeout".to_string(),
                message: format!("Operation timed out after {}s", secs),
                tool: None,
                retry_after: None,
            },
            AgentError::CommandBlocked(cmd) => Self {
                error: "CommandBlocked".to_string(),
                message: format!("Command '{}' is blocked for security", cmd),
                tool: Some("bash".to_string()),
                retry_after: None,
            },
            AgentError::PathEscape(path) => Self {
                error: "PathEscape".to_string(),
                message: format!("Path '{}' escapes workspace", path.display()),
                tool: None,
                retry_after: None,
            },
            AgentError::TextNotFound { path, snippet } => Self {
                error: "TextNotFound".to_string(),
                message: format!("Text '{}' not found in {}", snippet, path),
                tool: None,
                retry_after: None,
            },
            AgentError::ToolNotFound(name) => Self {
                error: "ToolNotFound".to_string(),
                message: format!("Tool '{}' not found", name),
                tool: Some(name.clone()),
                retry_after: None,
            },
            _ => Self {
                error: "AgentError".to_string(),
                message: err.to_string(),
                tool: None,
                retry_after: None,
            },
        }
    }
}

/*
 * ============================================================================
 * SESSION ENDPOINT TYPES
 * ============================================================================
 */

/// Response for the `/sessions` endpoint.
///
/// Lists all available conversation sessions.
#[derive(Debug, Serialize)]
pub struct SessionsResponse {
    /// List of available sessions.
    pub sessions: Vec<SessionSummary>,
}

/// Summary of a single session.
#[derive(Debug, Serialize)]
pub struct SessionSummary {
    /// Unique session identifier (filename).
    pub id: String,
    /// ISO 8601 timestamp of when the session was created.
    pub timestamp: String,
    /// Model used for this session.
    pub model: String,
    /// Total tokens used in this session.
    pub total_tokens: u32,
    /// Number of tool calls made.
    pub tool_calls: usize,
    /// Duration of the session in milliseconds.
    pub duration_ms: u64,
    /// Number of messages in the conversation.
    pub message_count: usize,
    /// Number of todos stored with the session.
    pub todo_count: usize,
}

/// Response for the `/sessions/{id}` endpoint.
///
/// Full details of a specific session.
#[derive(Debug, Serialize)]
pub struct SessionDetailResponse {
    /// Unique session identifier.
    pub id: String,
    /// ISO 8601 timestamp of when the session was created.
    pub timestamp: String,
    /// Model used for this session.
    pub model: String,
    /// System prompt used.
    pub system_prompt: String,
    /// Conversation history.
    pub history: Vec<MessageSummary>,
    /// Todos captured in the session.
    pub todos: Vec<TodoSummary>,
    /// Session statistics.
    pub stats: SessionStatsResponse,
}

/// Summary of a todo in the session.
#[derive(Debug, Serialize)]
pub struct TodoSummary {
    /// Stable todo identifier.
    pub id: String,
    /// Todo description.
    pub text: String,
    /// Current status.
    pub status: String,
}

/// Statistics for a session.
#[derive(Debug, Serialize)]
pub struct SessionStatsResponse {
    /// Total tokens used.
    pub total_tokens: u32,
    /// Number of tool calls made.
    pub tool_calls: usize,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// Summary of a message in the conversation.
#[derive(Debug, Serialize)]
pub struct MessageSummary {
    /// Role: "user", "assistant", or "system".
    pub role: String,
    /// Text content of the message.
    pub content: String,
}

/// Request to restore a session.
#[derive(Debug, Deserialize)]
pub struct RestoreSessionRequest {
    /// Whether to clear existing history before restoring.
    #[serde(default)]
    pub clear_history: bool,
}

/// Response for session restore.
#[derive(Debug, Serialize)]
pub struct RestoreSessionResponse {
    /// Whether the restore was successful.
    pub success: bool,
    /// Number of messages restored.
    pub message_count: usize,
}

/*
 * ============================================================================
 * CONFIG ENDPOINT TYPES
 * ============================================================================
 */

/// Response for the `/config` endpoint.
///
/// Current agent configuration.
#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    /// Working directory for the agent.
    pub working_dir: String,
    /// LLM model identifier.
    pub model: String,
    /// Maximum tokens for completions.
    pub max_tokens: u32,
    /// Context window size for the model.
    pub context_window_size: u32,
    /// Timeout for tool execution in seconds.
    pub tool_timeout_secs: u64,
    /// Whether approval is required for tools.
    pub require_approval: bool,
    /// Shell profile to use.
    pub shell_profile: Option<String>,
    /// Session log directory.
    pub session_log_dir: Option<String>,
}

/// Request to update configuration.
#[derive(Debug, Deserialize)]
pub struct UpdateConfigRequest {
    /// New model to use.
    #[serde(default)]
    pub model: Option<String>,
    /// New max tokens setting.
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// New context window size.
    #[serde(default)]
    pub context_window_size: Option<u32>,
    /// New tool timeout.
    #[serde(default)]
    pub tool_timeout_secs: Option<u64>,
    /// New approval requirement.
    #[serde(default)]
    pub require_approval: Option<bool>,
}

/// Response after updating configuration.
#[derive(Debug, Serialize)]
pub struct UpdateConfigResponse {
    /// Whether the update was successful.
    pub success: bool,
    /// Updated configuration.
    pub config: ConfigResponse,
}

/*
 * ============================================================================
 * HISTORY ENDPOINT TYPES
 * ============================================================================
 */

/// Response for the `/history` endpoint.
///
/// Current conversation history.
#[derive(Debug, Serialize)]
pub struct HistoryResponse {
    /// List of messages in the conversation.
    pub messages: Vec<MessageSummary>,
    /// Total number of messages.
    pub total: usize,
}

/*
 * ============================================================================
 * SKILLS ENDPOINT TYPES
 * ============================================================================
 */

/// Response for the `/skills` endpoint.
///
/// List of available skills/prompt templates.
#[derive(Debug, Serialize)]
pub struct SkillsResponse {
    /// List of available skills.
    pub skills: Vec<SkillSummary>,
}

/// Summary of a skill.
#[derive(Debug, Serialize)]
pub struct SkillSummary {
    /// Name of the skill.
    pub name: String,
    /// Description of what the skill does.
    pub description: String,
}

/*
 * ============================================================================
 * APPROVAL ENDPOINT TYPES
 * ============================================================================
 */

/// Request to submit an approval decision.
#[derive(Debug, Deserialize)]
pub struct ApprovalRequest {
    /// The approval decision: "approve", "deny", or "modify".
    pub decision: String,
    /// Modified command (only for "modify" decision).
    #[serde(default)]
    pub modified_command: Option<String>,
    /// Reason for denial (optional).
    #[serde(default)]
    pub reason: Option<String>,
}

/// Response for approval submission.
#[derive(Debug, Serialize)]
pub struct ApprovalResponse {
    /// Whether the decision was recorded.
    pub success: bool,
    /// The decision that was made.
    pub decision: String,
}

/*
 * ============================================================================
 * SSE EVENT TYPES (for documentation)
 * ============================================================================
 */

/// Token usage event payload (SSE event: "token_usage").
#[derive(Debug, Serialize)]
pub struct TokenUsageEvent {
    /// Input/prompt tokens.
    pub input_tokens: u32,
    /// Output/completion tokens.
    pub output_tokens: u32,
    /// Total tokens used.
    pub total_tokens: u32,
    /// Context window usage percentage.
    pub context_percent: u8,
}

/// Approval request event payload (SSE event: "approval_request").
#[derive(Debug, Serialize)]
pub struct ApprovalRequestEvent {
    /// Unique ID for this approval request.
    pub id: String,
    /// Tool name requiring approval.
    pub tool: String,
    /// Human-readable description of the action.
    pub action: String,
    /// The command or input to be executed.
    pub input: serde_json::Value,
}

/// Tool progress event payload (SSE event: "tool_progress").
#[derive(Debug, Serialize)]
pub struct ToolProgressEvent {
    /// Tool call ID.
    pub id: String,
    /// Progress message.
    pub message: String,
    /// Progress percentage (0-100) if available.
    pub percent: Option<u8>,
}
