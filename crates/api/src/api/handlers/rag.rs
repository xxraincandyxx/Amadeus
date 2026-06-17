//! Handlers for RAG document ingestion, semantic query, listing, and deletion.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::api::http::AppState;
use crate::api::types::{
    ErrorResponse, RagDocumentInfo, RagDocumentsResponse, RagIngestRequest, RagIngestResponse,
    RagQueryRequest, RagQueryResponse, RagSearchResult,
};
use crate::client::LLMClient;
use amadeus_rag::chunker::chunk_text;

/// `POST /rag/ingest` — ingest text into the vector store.
pub async fn rag_ingest<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(request): Json<RagIngestRequest>,
) -> Result<Json<RagIngestResponse>, (StatusCode, Json<ErrorResponse>)> {
    let text = if let Some(t) = request.text {
        t
    } else if let Some(ref path) = request.path {
        std::fs::read_to_string(path).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "rag_ingest_failed".into(),
                    message: format!("Failed to read file '{}': {}", path, e),
                    tool: None,
                    retry_after: None,
                }),
            )
        })?
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "rag_ingest_failed".into(),
                message: "One of 'text' or 'path' is required.".into(),
                tool: None,
                retry_after: None,
            }),
        ));
    };

    let document_id = request
        .document_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let chunk_size = request.chunk_size.unwrap_or(state.config.rag_chunk_size);
    let chunk_overlap = request
        .chunk_overlap
        .unwrap_or(state.config.rag_chunk_overlap);

    let chunks = chunk_text(&text, chunk_size, chunk_overlap);
    if chunks.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "rag_ingest_failed".into(),
                message: "No content extracted for ingestion.".into(),
                tool: None,
                retry_after: None,
            }),
        ));
    }

    let embeddings = state.embedding_client.embed(&chunks).await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: "rag_embedding_failed".into(),
                message: e.to_string(),
                tool: None,
                retry_after: None,
            }),
        )
    })?;

    let chunk_count = state
        .rag_provider
        .ingest_chunks(&document_id, "", chunks, embeddings)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "rag_store_failed".into(),
                    message: e.to_string(),
                    tool: None,
                    retry_after: None,
                }),
            )
        })?;

    Ok(Json(RagIngestResponse {
        document_id,
        chunk_count,
    }))
}

/// `POST /rag/query` — semantic search over ingested documents.
pub async fn rag_query<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(request): Json<RagQueryRequest>,
) -> Result<Json<RagQueryResponse>, (StatusCode, Json<ErrorResponse>)> {
    let top_k = request.top_k.unwrap_or(state.config.rag_top_k);

    let query_embedding = state
        .embedding_client
        .embed_single(&request.query)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: "rag_embedding_failed".into(),
                    message: e.to_string(),
                    tool: None,
                    retry_after: None,
                }),
            )
        })?;

    let results: Vec<RagSearchResult> = state
        .rag_provider
        .search(&query_embedding, top_k)
        .into_iter()
        .enumerate()
        .map(|(i, (entry, score))| RagSearchResult {
            rank: i + 1,
            key: entry.key,
            content: entry.content,
            source: entry.source,
            score,
        })
        .collect();

    Ok(Json(RagQueryResponse {
        query: request.query,
        results,
    }))
}

/// `GET /rag/documents` — list ingested documents.
pub async fn rag_list_documents<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> Result<Json<RagDocumentsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let documents: Vec<RagDocumentInfo> = state
        .rag_provider
        .list_documents()
        .into_iter()
        .map(|d| RagDocumentInfo {
            id: d.id,
            chunk_count: d.chunk_count,
            ingested_at: d.ingested_at,
        })
        .collect();

    Ok(Json(RagDocumentsResponse { documents }))
}

/// `DELETE /rag/documents/:id` — delete a document and all its chunks.
pub async fn rag_delete_document<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(document_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let removed = state
        .rag_provider
        .delete_document(&document_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "rag_delete_failed".into(),
                    message: e.to_string(),
                    tool: None,
                    retry_after: None,
                }),
            )
        })?;

    Ok(Json(serde_json::json!({
        "deleted": removed,
        "document_id": document_id
    })))
}
