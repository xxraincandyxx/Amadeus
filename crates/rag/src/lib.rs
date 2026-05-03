// @amadeus-header
// summary: RAG (Retrieval-Augmented Generation) with embedding-based semantic search.
// layer: core
// status: active
// feature_flags: none
// provides:
// - crate: amadeus_rag
// - module: crate::embedding
// - module: crate::chunker
// - type: crate::vector_store::VectorMemoryProvider
// uses:
// - crate: amadeus_context (MemoryProvider trait)
// - service: /v1/embeddings (OpenAI-compatible endpoint)
// invariants:
// - VectorMemoryProvider is Send + Sync via internal Mutex.
// - EmbeddingClient batches requests into groups of 32.
// side_effects:
// - EmbeddingClient makes HTTP calls to the embedding API.
// - VectorMemoryProvider reads/writes .amadeus/rag_index.json.
// tests:
// - cmd: cargo test -p rag
// @end-amadeus-header

//! RAG (Retrieval-Augmented Generation) crate.
//!
//! Provides embedding-based semantic search over documents:
//! - [`EmbeddingClient`](embedding::EmbeddingClient) — calls an OpenAI-compatible `/v1/embeddings` endpoint
//! - [`chunk_text`](chunker::chunk_text) — splits text into overlapping chunks at natural boundaries
//! - [`VectorMemoryProvider`](vector_store::VectorMemoryProvider) — persistent vector store implementing the `MemoryProvider` trait

pub mod chunker;
pub mod embedding;
pub mod tool;
pub mod vector_store;
