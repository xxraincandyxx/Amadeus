//! # Skills Handler
//!
//! Handles the skills listing endpoint for prompt templates.

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::api::http::AppState;
use crate::api::types::{ErrorResponse, SkillSummary, SkillsResponse};
use crate::client::LLMClient;
use crate::skills::registry::SkillRegistry;

/// GET /skills
///
/// List all available skills/prompt templates.
pub async fn list_skills<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> std::result::Result<Json<SkillsResponse>, Json<ErrorResponse>> {
    let config = state.supervisor.config();

    // Load skills from the configured skills directory
    let skills_dir = config.workdir.join(".amadeus").join("skills");

    let registry = match SkillRegistry::load_from_dir(&skills_dir) {
        Ok(r) => r,
        Err(e) => {
            return Err(Json(ErrorResponse::new(
                "SkillLoadError",
                e.to_string(),
            )))
        }
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
