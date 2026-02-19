//! # Execute Handler
//!
//! Handles POST /execute requests for direct bash command execution.

use axum::Json;
use serde_json::json;

use crate::api::types::{ErrorResponse, ExecuteRequest, ExecuteResponse};
use crate::error::AgentError;
use crate::tools::bash::BashTool;
use crate::tools::tool_trait::Tool;

/// Execute a bash command directly on the host system.
///
/// This endpoint provides direct access to the `bash` tool. It is useful
/// for running system commands, tests, or scripts without full agent logic.
///
/// ### Security
///
/// Commands are checked against a blocklist and timed out if they run too long.
///
/// ### Request
///
/// - **Method:** POST
/// - **Path:** /execute
/// - **Body:** [`ExecuteRequest`]
///
/// ### Response
///
/// - **Success:** 200 OK with [`ExecuteResponse`]
/// - **Error:** 200 OK with exit_code -1 in the response body (for execution errors)
///
/// ### Example
///
/// ```bash
/// curl -X POST http://localhost:3000/execute \
///   -H "Content-Type: application/json" \
///   -d '{"command": "ls -la", "timeout_secs": 10}'
/// ```
pub async fn execute(
    Json(request): Json<ExecuteRequest>,
) -> Result<Json<ExecuteResponse>, Json<ErrorResponse>> {
    let timeout_secs = request.timeout_secs.unwrap_or(30);

    let tool = BashTool::new(
        timeout_secs,
        ".".to_string(),
        vec!["rm -rf /".to_string()],
        50_000,
    );

    let input = json!({
        "command": request.command
    });

    match tool.execute(input).await {
        Ok(output) => Ok(Json(ExecuteResponse {
            output,
            exit_code: 0,
            timed_out: false,
        })),

        Err(AgentError::Timeout(_)) => Ok(Json(ExecuteResponse {
            output: String::new(),
            exit_code: -1,
            timed_out: true,
        })),

        Err(e) => Ok(Json(ExecuteResponse {
            output: format!("Error: {}", e),
            exit_code: -1,
            timed_out: false,
        })),
    }
}

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
