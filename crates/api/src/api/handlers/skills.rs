// @amadeus-header
// summary: HTTP handler implementation for skills routes.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::handlers::skills
// - fn: crate::api::handlers::skills::list_skills
// uses:
// - module: crate::api::http::AppState
// - module: crate::api::types
// - module: crate::client::LLMClient
// - module: crate::skills
// - protocol: axum HTTP handlers
// invariants:
// - Handler request and response handling stays aligned with route contracts.
// side_effects:
// - Performs network or HTTP operations.
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

//! # Skills Handler
//!
//! Handles the skills listing endpoint for prompt templates.

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::api::http::AppState;
use crate::api::types::{ErrorResponse, SkillSummary, SkillsResponse};
use crate::client::LLMClient;
use crate::skills;

/// GET /skills
///
/// List all available skills/prompt templates.
pub async fn list_skills<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> std::result::Result<Json<SkillsResponse>, Json<ErrorResponse>> {
    let config = &state.config;

    let registry = match skills::load_for_config(config) {
        Ok(r) => r,
        Err(e) => return Err(Json(ErrorResponse::new("SkillLoadError", e.to_string()))),
    };

    let skills: Vec<SkillSummary> = registry
        .all()
        .into_iter()
        .map(|skill| SkillSummary {
            name: skill.name.clone(),
            description: skill.description.clone(),
        })
        .collect();

    Ok(Json(SkillsResponse { skills }))
}
