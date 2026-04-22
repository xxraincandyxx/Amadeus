// @amadeus-header
// summary: HTTP handler implementation for research-oriented summarization routes.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::handlers::summarize
// - fn: crate::api::handlers::summarize::summarize
// uses:
// - module: crate::agent::compaction::ContextCompactor
// - module: crate::agent::messages::Message
// - module: crate::api::http::AppState
// - module: crate::api::types
// - module: crate::client::LLMClient
// - protocol: axum HTTP handlers
// invariants:
// - Summarization request and response shapes stay stable for research workflows.
// side_effects:
// - Performs network or HTTP operations.
// tests:
// - tests/compaction_test.rs
// @end-amadeus-header

use std::sync::Arc;

use axum::{extract::State, Json};

use crate::agent::compaction::{CompactionConfig, ContextCompactor};
use crate::agent::messages::Message;
use crate::api::http::AppState;
use crate::api::types::{ErrorResponse, SummarizeRequest, SummarizeResponse};
use crate::client::LLMClient;

/// POST /summarize
///
/// Generate a summary using either the LLM-backed compaction summarizer
/// or the extract-based fallback summarizer.
pub async fn summarize<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(request): Json<SummarizeRequest>,
) -> Result<Json<SummarizeResponse>, Json<ErrorResponse>> {
    let mut config = CompactionConfig::default();
    if let Some(max_summary_chars) = request.max_summary_chars {
        config.max_summary_chars = max_summary_chars;
    }

    let compactor = ContextCompactor::new(config);
    let messages = vec![Message::user(&request.text)];
    let mechanism = request.mechanism.unwrap_or_else(|| "llm".to_string());

    match mechanism.as_str() {
        "extract" => Ok(Json(SummarizeResponse {
            summary: compactor.extract_summary(&messages),
            mechanism,
            prompt_used: None,
        })),
        "llm" => {
            let summary = compactor
                .summarize_preview(&messages, &state.client, request.prompt.as_deref())
                .await
                .map_err(|e| Json(ErrorResponse::from_agent_error(&e)))?;
            Ok(Json(SummarizeResponse {
                summary,
                mechanism,
                prompt_used: request.prompt,
            }))
        }
        _ => Err(Json(ErrorResponse::new(
            "InvalidMechanism",
            "Mechanism must be 'llm' or 'extract'",
        ))),
    }
}
