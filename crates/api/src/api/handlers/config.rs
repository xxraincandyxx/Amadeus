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
use crate::api::types::{
    ConfigResponse, ErrorResponse, PromptConfigSummary, ToolConfigSummary, ToolInventorySummary,
    UpdateConfigRequest, UpdateConfigResponse,
};
use crate::client::LLMClient;
use crate::tools::registry::ToolRegistry;

/// GET /config
///
/// Get the current agent configuration.
pub async fn get_config<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> Json<ConfigResponse> {
    let config = &state.config;

    Json(config_response(config, None, None, None, None, None))
}

fn config_response(
    config: &crate::agent::config::Config,
    model: Option<String>,
    max_tokens: Option<u32>,
    context_window_size: Option<u32>,
    tool_timeout_secs: Option<u64>,
    require_approval: Option<bool>,
) -> ConfigResponse {
    let registry = ToolRegistry::with_defaults(config);
    ConfigResponse {
        working_dir: config.workdir.display().to_string(),
        model: model.unwrap_or_else(|| config.model.clone()),
        max_tokens: max_tokens.unwrap_or(4096),
        context_window_size: context_window_size.unwrap_or(config.context_window_size),
        tool_timeout_secs: tool_timeout_secs.unwrap_or(config.timeout_seconds),
        require_approval: require_approval.unwrap_or(false),
        shell_profile: None, // Not stored in config currently
        session_log_dir: config
            .session_log_dir
            .as_ref()
            .map(|p| p.display().to_string()),
        prompt: PromptConfigSummary {
            active_profile: config.prompt_profile_name().to_string(),
            section_count: config.prompt_profile_section_count(),
            configured: config.prompt_profile().is_some(),
        },
        tools: ToolConfigSummary {
            active_profile: registry.profile().name.clone(),
            inventory: registry
                .inventory()
                .into_iter()
                .map(|tool| ToolInventorySummary {
                    name: tool.name,
                    pack: tool.pack,
                    source: tool.source.as_str().to_string(),
                    required_permission: tool.required_permission.as_str().to_string(),
                    overridden: tool.overridden,
                })
                .collect(),
        },
    }
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

    let updated_config = config_response(
        config,
        request.model,
        request.max_tokens,
        request.context_window_size,
        request.tool_timeout_secs,
        request.require_approval,
    );

    Ok(Json(UpdateConfigResponse {
        success: true,
        config: updated_config,
    }))
}
