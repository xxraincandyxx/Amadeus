// @amadeus-header
// summary: Retrieval ingestion, query, result, and document API data types.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - type: crate::api::types::RagDocumentsResponse
// - type: crate::api::types::RagIngestRequest
// - type: crate::api::types::RagQueryRequest
// - type: crate::api::types::RagQueryResponse
// uses:
// - protocol: serde serialization
// invariants:
// - Serialized fields retain the retrieval HTTP contract.
// side_effects: none
// tests:
// - cmd: cargo test -p api --all-features
// @end-amadeus-header

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// RAG API types
// ---------------------------------------------------------------------------

/// Request for `POST /rag/ingest`.
#[derive(Debug, Deserialize)]
pub struct RagIngestRequest {
    /// Raw text to ingest directly.
    #[serde(default)]
    pub text: Option<String>,
    /// Path to a local file to ingest.
    #[serde(default)]
    pub path: Option<String>,
    /// Optional human-readable document identifier. Auto-generated if not provided.
    #[serde(default)]
    pub document_id: Option<String>,
    /// Target characters per chunk.
    #[serde(default)]
    pub chunk_size: Option<usize>,
    /// Overlap characters between adjacent chunks.
    #[serde(default)]
    pub chunk_overlap: Option<usize>,
}

/// Response for `POST /rag/ingest`.
#[derive(Debug, Serialize)]
pub struct RagIngestResponse {
    pub document_id: String,
    pub chunk_count: usize,
}

/// Request for `POST /rag/query`.
#[derive(Debug, Deserialize)]
pub struct RagQueryRequest {
    /// Natural language query for semantic search.
    pub query: String,
    /// Number of top results to return (default from config).
    #[serde(default)]
    pub top_k: Option<usize>,
}

/// Response for `POST /rag/query`.
#[derive(Debug, Serialize)]
pub struct RagQueryResponse {
    pub query: String,
    pub results: Vec<RagSearchResult>,
}

/// A single semantic search result with relevance score.
#[derive(Debug, Serialize)]
pub struct RagSearchResult {
    pub rank: usize,
    pub key: String,
    pub content: String,
    pub source: String,
    pub score: f32,
}

/// Response for `GET /rag/documents`.
#[derive(Debug, Serialize)]
pub struct RagDocumentsResponse {
    pub documents: Vec<RagDocumentInfo>,
}

/// Summary info for an ingested document.
#[derive(Debug, Serialize)]
pub struct RagDocumentInfo {
    pub id: String,
    pub chunk_count: usize,
    pub ingested_at: String,
}
