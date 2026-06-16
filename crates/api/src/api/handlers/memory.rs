//! Handlers for memory provider inspection, entry loading, and mutation.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::api::http::AppState;
use crate::api::types::{
    ErrorResponse, MemoryEntriesResponse, MemoryEntryInfo, MemoryProviderInfo,
    MemoryProvidersResponse, StoreMemoryRequest,
};
use crate::client::LLMClient;
use crate::context::memory::{MemoryEntry, MemoryProvider, MemoryRegistry};
use crate::context::memory_file::FileMemoryProvider;

/// `GET /memory/providers` — list all registered memory providers.
pub async fn list_memory_providers<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> Result<Json<MemoryProvidersResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut registry = MemoryRegistry::new();
    registry.register(Arc::new(FileMemoryProvider::new(
        state.config.workdir.clone(),
    )));
    registry.register(state.memory_provider.clone());

    let providers: Vec<MemoryProviderInfo> = registry
        .list_providers()
        .iter()
        .map(|p| MemoryProviderInfo {
            name: p.name().to_string(),
            writable: p.writable(),
            entry_count: p.load().len(),
        })
        .collect();

    Ok(Json(MemoryProvidersResponse { providers }))
}

/// `GET /memory/entries` — load all memory entries from all providers.
pub async fn load_memory_entries<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> Result<Json<MemoryEntriesResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut registry = MemoryRegistry::new();
    registry.register(Arc::new(FileMemoryProvider::new(
        state.config.workdir.clone(),
    )));
    registry.register(state.memory_provider.clone());

    let entries: Vec<MemoryEntryInfo> = registry
        .load_all()
        .into_iter()
        .map(|e| MemoryEntryInfo {
            key: e.key,
            content: e.content,
            source: e.source,
        })
        .collect();

    Ok(Json(MemoryEntriesResponse { entries }))
}

/// `POST /memory/entries` — store a new memory entry.
pub async fn store_entry<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(request): Json<StoreMemoryRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let entry = MemoryEntry::new(request.key, request.content, request.source);
    state
        .memory_provider
        .store(entry)
        .map_err(|e| {
            let msg = e.to_string();
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "memory_store_failed".into(),
                    message: msg,
                    tool: None,
                    retry_after: None,
                }),
            )
        })?;

    Ok(Json(serde_json::json!({"success": true})))
}

/// `DELETE /memory/entries/:key` — delete a memory entry by key.
pub async fn delete_entry<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    state
        .memory_provider
        .delete(&key)
        .map_err(|e| {
            let status = match &e {
                crate::context::memory::MemoryError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (
                status,
                Json(ErrorResponse {
                    error: "memory_delete_failed".into(),
                    message: e.to_string(),
                    tool: None,
                    retry_after: None,
                }),
            )
        })?;

    Ok(Json(serde_json::json!({"success": true})))
}
