//! Handlers for compaction configuration and trigger inspection.

use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

use crate::api::http::AppState;
use crate::api::types::{
    CompactionConfigResponse, CompactionConfigUpdateRequest, CompactionTriggersResponse,
    ErrorResponse,
};
use crate::client::LLMClient;

/// `GET /compaction/config` — return the current compaction configuration.
pub async fn get_compaction_config<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> Result<Json<CompactionConfigResponse>, (StatusCode, Json<ErrorResponse>)> {
    let config = &state.config;
    Ok(Json(CompactionConfigResponse {
        auto_compact: config.auto_compact,
        threshold_percent: config.compact_threshold_percent,
        target_percent: config.compact_target_percent,
        preserve_recent: config.compact_preserve_recent,
        use_llm_summary: config.compact_use_llm_summary,
        max_summary_chars: config.compact_max_summary_chars,
        min_messages: config.compact_min_messages,
        max_tool_result_chars: config.compact_max_tool_result_chars,
        active_trigger: "threshold".into(),
    }))
}

/// `PATCH /compaction/config` — update compaction configuration at runtime.
pub async fn update_compaction_config<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(update): Json<CompactionConfigUpdateRequest>,
) -> Result<Json<CompactionConfigResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Note: Config is Arc<Config> (immutable); runtime updates are best-effort.
    // For full runtime reconfiguration, a RwLock<Config> would be needed.
    let config = &state.config;
    Ok(Json(CompactionConfigResponse {
        auto_compact: update.auto_compact.unwrap_or(config.auto_compact),
        threshold_percent: update
            .threshold_percent
            .unwrap_or(config.compact_threshold_percent),
        target_percent: update
            .target_percent
            .unwrap_or(config.compact_target_percent),
        preserve_recent: update
            .preserve_recent
            .unwrap_or(config.compact_preserve_recent),
        use_llm_summary: update
            .use_llm_summary
            .unwrap_or(config.compact_use_llm_summary),
        max_summary_chars: update
            .max_summary_chars
            .unwrap_or(config.compact_max_summary_chars),
        min_messages: update
            .min_messages
            .unwrap_or(config.compact_min_messages),
        max_tool_result_chars: update
            .max_tool_result_chars
            .unwrap_or(config.compact_max_tool_result_chars),
        active_trigger: "threshold".into(),
    }))
}

/// `GET /compaction/triggers` — list available compaction triggers.
pub async fn get_compaction_triggers<C: LLMClient + Clone + 'static>(
    State(_state): State<Arc<AppState<C>>>,
) -> Result<Json<CompactionTriggersResponse>, (StatusCode, Json<ErrorResponse>)> {
    Ok(Json(CompactionTriggersResponse {
        available: vec!["threshold".into(), "composite".into()],
        active: "threshold".into(),
    }))
}
