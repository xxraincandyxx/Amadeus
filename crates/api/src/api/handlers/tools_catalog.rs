//! Handlers for tool catalog inspection.

use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

use crate::api::http::AppState;
use crate::api::types::{ErrorResponse, ToolCatalogEntry, ToolCatalogResponse};
use crate::client::LLMClient;

/// `GET /tools/catalog` — return the full tool catalog.
pub async fn get_tool_catalog<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> Result<Json<ToolCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Build a default tools registry and extract spec names
    use crate::tools::registry::ToolRegistry;

    let registry = ToolRegistry::with_defaults(&state.config);

    let entries: Vec<ToolCatalogEntry> = registry
        .catalog()
        .names()
        .iter()
        .map(|name| {
            let spec = registry.catalog().spec(name);
            ToolCatalogEntry {
                name: name.clone(),
                description: spec
                    .map(|s| s.description.clone())
                    .unwrap_or_default(),
                permission_mode: spec
                    .map(|s| s.required_permission.as_str().to_string())
                    .unwrap_or_else(|| "unknown".into()),
                level: spec
                    .map(|s| s.level.as_str().to_string())
                    .unwrap_or_else(|| "unknown".into()),
            }
        })
        .collect();

    Ok(Json(ToolCatalogResponse { tools: entries }))
}
