// @amadeus-header
// summary: HTTP handler implementation for execute routes.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::handlers::execute
// - fn: crate::api::handlers::execute::execute
// uses:
// - module: crate::api::http::AppState
// - module: crate::api::types
// - module: crate::client::LLMClient
// - module: crate::permissions
// - module: crate::policy
// - module: crate::tools::bash::BashTool
// - protocol: axum HTTP handlers
// invariants:
// - Handler request and response handling stays aligned with route contracts.
// side_effects:
// - Performs network or HTTP operations.
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

//! # Execute Handler
//!
//! Handles POST /execute requests to run bash commands directly.

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::api::http::AppState;
use crate::api::types::{ErrorResponse, ExecuteRequest, ExecuteResponse};
use crate::client::LLMClient;
use crate::permissions::{PermissionDecision, PermissionEnforcer};
use crate::policy::Policy;
use crate::tools::bash::BashTool;

/// Process a command execution request.
pub async fn execute<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(request): Json<ExecuteRequest>,
) -> std::result::Result<Json<ExecuteResponse>, Json<ErrorResponse>> {
    let bash = BashTool::from_config(&state.config);
    let command = request.command;
    let timeout_secs = request.timeout_secs.unwrap_or(state.config.timeout_seconds);
    let input = serde_json::json!({ "command": command.clone() });

    validate_execute_permissions(&state.config, &input).map_err(Json)?;
    let result = bash
        .execute_with_metadata(&command, timeout_secs)
        .await
        .map_err(|err| ErrorResponse::from_agent_error(&err))?;

    Ok(Json(ExecuteResponse {
        output: result.output,
        exit_code: result.exit_code,
        timed_out: result.timed_out,
    }))
}

fn validate_execute_permissions(
    config: &crate::agent::config::Config,
    input: &serde_json::Value,
) -> std::result::Result<(), ErrorResponse> {
    let enforcer = PermissionEnforcer::from_config(config);
    let decision = enforcer.check("bash", input);

    if let PermissionDecision::Deny { reason, .. } | PermissionDecision::Ask { reason, .. } =
        decision
    {
        let mut response = ErrorResponse::new("PermissionDenied", reason);
        response.tool = Some("bash".to_string());
        return Err(response);
    }

    let policy = Policy::from_config(config);
    if policy.needs_approval("bash", input) {
        let mut response =
            ErrorResponse::new("PermissionDenied", policy.approval_reason("bash", input));
        response.tool = Some("bash".to_string());
        return Err(response);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use amadeus_core::PermissionMode;

    #[test]
    fn validate_execute_permissions_allows_read_only_command() {
        let mut config = crate::agent::config::Config::default();
        config.permission_mode = PermissionMode::ReadOnly;

        let input = serde_json::json!({
            "command": "ls -la"
        });

        assert!(
            validate_execute_permissions(&config, &input).is_ok(),
            "read-only command should pass permission checks"
        );
    }

    #[test]
    fn validate_execute_permissions_denies_dangerous_command_for_read_only_mode() {
        let mut config = crate::agent::config::Config::default();
        config.permission_mode = PermissionMode::ReadOnly;

        let input = serde_json::json!({
            "command": "rm -rf /tmp/amadeus_test"
        });

        let err = validate_execute_permissions(&config, &input).unwrap_err();
        assert_eq!(err.error, "PermissionDenied");
        assert_eq!(err.tool, Some("bash".to_string()));
    }

    #[test]
    fn validate_execute_permissions_denies_policy_gated_command() {
        let mut config = crate::agent::config::Config::default();
        config.permission_mode = PermissionMode::Allow;

        let input = serde_json::json!({
            "command": "echo hello > /tmp/amadeus_test.txt"
        });

        let err = validate_execute_permissions(&config, &input).unwrap_err();
        assert_eq!(err.error, "PermissionDenied");
        assert_eq!(err.tool, Some("bash".to_string()));
    }
}
