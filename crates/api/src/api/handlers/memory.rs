//! Handlers for memory provider inspection and entry loading.

use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

use crate::api::http::AppState;
use crate::api::types::{
    ErrorResponse, MemoryEntriesResponse, MemoryEntryInfo, MemoryProviderInfo,
    MemoryProvidersResponse,
};
use crate::client::LLMClient;

/// `GET /memory/providers` — list all registered memory providers.
pub async fn list_memory_providers<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> Result<Json<MemoryProvidersResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Build a default registry with file and session providers
    let mut registry = crate::context::memory::MemoryRegistry::new();
    registry.register(Arc::new(
        crate::context::memory_file::FileMemoryProvider::new(state.config.workdir.clone()),
    ));
    registry.register(Arc::new(
        crate::context::memory_session::SessionMemoryProvider::new(),
    ));

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
    let mut registry = crate::context::memory::MemoryRegistry::new();
    registry.register(Arc::new(
        crate::context::memory_file::FileMemoryProvider::new(state.config.workdir.clone()),
    ));

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
