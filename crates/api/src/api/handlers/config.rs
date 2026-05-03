// @amadeus-header
// summary: HTTP handler implementation for config routes.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::handlers::config
// - fn: crate::api::handlers::config::get_config
// - fn: crate::api::handlers::config::update_config
// uses:
// - module: crate::api::http::AppState
// - module: crate::api::types
// - module: crate::client::LLMClient
// - protocol: axum HTTP handlers
// invariants:
// - Handler request and response handling stays aligned with route contracts.
// side_effects:
// - Performs network or HTTP operations.
// tests:
// - tests/config_test.rs
// @end-amadeus-header

//! # Config Handler
//!
//! Handles configuration endpoints for getting and updating agent settings.

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::api::http::AppState;
use crate::api::types::{ConfigResponse, ErrorResponse, UpdateConfigRequest, UpdateConfigResponse};
use crate::client::LLMClient;

/// GET /config
///
/// Get the current agent configuration.
pub async fn get_config<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> Json<ConfigResponse> {
    let config = &state.config;

    Json(ConfigResponse {
        working_dir: config.workdir.display().to_string(),
        model: config.model.clone(),
        max_tokens: config.max_output_tokens,
        context_window_size: config.context_window_size,
        tool_timeout_secs: config.timeout_seconds,
        require_approval: false, // Not stored in config currently
        shell_profile: None,     // Not stored in config currently
        session_log_dir: config
            .session_log_dir
            .as_ref()
            .map(|p| p.display().to_string()),
    })
}

/// PATCH /config
///
/// Update agent configuration settings.
///
/// Note: In the current stateless REST implementation, this returns the
/// requested changes but does not persist them. For persistent configuration,
/// use the configuration file or environment variables.
pub async fn update_config<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(request): Json<UpdateConfigRequest>,
) -> std::result::Result<Json<UpdateConfigResponse>, Json<ErrorResponse>> {
    let config = &state.config;

    // In a stateful implementation, we would update the config here.
    // For now, we return what the config would look like with the changes.

    let updated_config = ConfigResponse {
        working_dir: config.workdir.display().to_string(),
        model: request.model.unwrap_or_else(|| config.model.clone()),
        max_tokens: request.max_tokens.unwrap_or(config.max_output_tokens),
        context_window_size: request
            .context_window_size
            .unwrap_or(config.context_window_size),
        tool_timeout_secs: request.tool_timeout_secs.unwrap_or(config.timeout_seconds),
        require_approval: request.require_approval.unwrap_or(false),
        shell_profile: None,
        session_log_dir: config
            .session_log_dir
            .as_ref()
            .map(|p| p.display().to_string()),
    };

    Ok(Json(UpdateConfigResponse {
        success: true,
        config: updated_config,
    }))
}
