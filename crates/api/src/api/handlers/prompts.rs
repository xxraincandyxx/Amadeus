//! Handlers for system prompt inspection and custom prompt building.

use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

use crate::api::http::AppState;
use crate::api::types::{
    BuildPromptRequest, BuildPromptResponse, ErrorResponse, PromptSectionInfo,
    PromptSectionsResponse,
};
use crate::client::LLMClient;

/// `GET /prompts/sections` — list all current prompt sections.
pub async fn list_prompt_sections<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> Result<Json<PromptSectionsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let include_sub_agent = state.config.max_subagent_depth > 0;
    let sections = crate::prompts::default_sections(
        &state.config.workdir.display().to_string(),
        include_sub_agent,
    );

    Ok(Json(PromptSectionsResponse {
        sections: sections
            .iter()
            .map(|s| PromptSectionInfo {
                id: s.id.clone(),
                title: s.title.clone(),
                priority: s.priority,
                dynamic: s.dynamic,
                content_preview: s.content.chars().take(200).collect(),
            })
            .collect(),
    }))
}

/// `POST /prompts/build` — build a system prompt with optional custom sections.
pub async fn build_prompt<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(request): Json<BuildPromptRequest>,
) -> Result<Json<BuildPromptResponse>, (StatusCode, Json<ErrorResponse>)> {
    let workdir = request
        .workdir
        .unwrap_or_else(|| state.config.workdir.display().to_string());
    let include_sub_agent = request.include_sub_agent_tool.unwrap_or(true);

    let extra_sections: Vec<crate::prompts::PromptSection> = request
        .extra_sections
        .unwrap_or_default()
        .into_iter()
        .map(|s| {
            crate::prompts::PromptSection::new(s.id, "", s.content)
                .with_priority(s.priority.unwrap_or(100))
                .with_dynamic(true)
        })
        .collect();

    let prompt =
        crate::prompts::build_system_prompt(&workdir, include_sub_agent, &extra_sections);

    // Count sections (approximate)
    let section_count = 8 + extra_sections.len(); // 8 defaults + extras

    Ok(Json(BuildPromptResponse {
        prompt,
        section_count,
    }))
}
