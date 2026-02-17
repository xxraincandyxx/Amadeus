//! # Execute Handler
//!
//! Handles POST /execute requests for direct bash command execution.
//!
//! ## Endpoint
//!
//! `POST /execute`
//!
//! ## Request Body
//!
//! ```json
//! {
//!   "command": "ls -la",
//!   "timeout_secs": 30
//! }
//! ```
//!
//! ## Response
//!
//! ```json
//! {
//!   "output": "total 24\ndrwxr-xr-x ...",
//!   "exit_code": 0,
//!   "timed_out": false
//! }
//! ```
//!
//! ## Purpose
//!
//! Provides direct access to the bash tool without LLM involvement.
//! Useful for:
//! - Quick command execution
//! - Testing commands before giving to agent
//! - Scripting and automation

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

// Axum types for HTTP handling
use axum::Json;

// Request and response types
use crate::api::types::{ErrorResponse, ExecuteRequest, ExecuteResponse};

// Bash tool for command execution
use crate::agent::messages::ToolInput;
use crate::tools::bash::BashTool;

// Error types
use crate::error::AgentError;

/*
 * ============================================================================
 * HANDLER FUNCTION
 * ============================================================================
 */

/// Handle POST /execute requests.
///
/// Executes a bash command directly and returns the output.
///
/// # Request Body
///
/// - `command`: The shell command to execute (required)
/// - `timeout_secs`: Timeout in seconds (optional, default: 30)
///
/// # Response
///
/// - `output`: Combined stdout and stderr
/// - `exit_code`: Command exit code (0 = success)
/// - `timed_out`: Whether the command timed out
///
/// # Example
///
/// ```bash
/// curl -X POST http://localhost:3000/execute \
///   -H "Content-Type: application/json" \
///   -d '{"command": "ls -la", "timeout_secs": 10}'
/// ```
///
/// # Security Note
///
/// This endpoint executes arbitrary shell commands.
/// In production, consider:
/// - Authentication/authorization
/// - Command whitelisting
/// - Sandboxing (containers, namespaces)
/// - Rate limiting
pub async fn execute(
    // Parse request body as JSON
    Json(request): Json<ExecuteRequest>,
) -> Result<Json<ExecuteResponse>, Json<ErrorResponse>> {
    // -------------------------------------------------------------------------
    // DETERMINE TIMEOUT
    // -------------------------------------------------------------------------

    // Use request timeout or default (30 seconds)
    //
    // Shorter default than chat because this is for quick commands
    let timeout_secs = request.timeout_secs.unwrap_or(30);

    // -------------------------------------------------------------------------
    // CREATE BASH TOOL
    // -------------------------------------------------------------------------

    // Create a BashTool instance
    //
    // BashTool::new takes:
    // - timeout_secs: Maximum seconds for command execution
    // - workdir: Working directory (use current directory ".")
    let tool = BashTool::new(timeout_secs, ".".to_string());

    // -------------------------------------------------------------------------
    // CREATE TOOL INPUT
    // -------------------------------------------------------------------------

    // Create the ToolInput struct
    //
    // ToolInput has one field: command (the shell command string)
    let input = ToolInput {
        command: request.command,
    };

    // -------------------------------------------------------------------------
    // EXECUTE COMMAND
    // -------------------------------------------------------------------------

    // Execute the command
    //
    // tool.execute returns Result<String>:
    // - Ok(output): Command succeeded, output is stdout+stderr
    // - Err(AgentError::Timeout): Command timed out
    // - Err(AgentError::Io): Command failed to start
    match tool.execute(&input).await {
        // Success - command completed
        Ok(output) => Ok(Json(ExecuteResponse {
            output,
            exit_code: 0,
            timed_out: false,
        })),

        // Timeout error
        Err(AgentError::Timeout(_)) => Ok(Json(ExecuteResponse {
            output: String::new(),
            exit_code: -1,
            timed_out: true,
        })),

        // Other errors (IO, etc.)
        Err(e) => Ok(Json(ExecuteResponse {
            output: format!("Error: {}", e),
            exit_code: -1,
            timed_out: false,
        })),
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
    fn test_execute_request_deserialization() {
        let json = r#"{"command": "ls -la", "timeout_secs": 60}"#;
        let request: ExecuteRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.command, "ls -la");
        assert_eq!(request.timeout_secs, Some(60));
    }

    #[test]
    fn test_execute_request_defaults() {
        let json = r#"{"command": "echo hello"}"#;
        let request: ExecuteRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.command, "echo hello");
        assert_eq!(request.timeout_secs, None);
    }
}
