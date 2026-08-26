// @amadeus-header
// summary: RAG (Retrieval-Augmented Generation) with pluggable embedding backends and semantic search.
// layer: core
// status: active
// feature_flags: none
// provides:
// - crate: amadeus_rag
// - module: crate::embedding
// - module: crate::local_embedding
// - module: crate::kylin_embedding
// - module: crate::chunker
// - trait: crate::embedding::Embedder
// - type: crate::vector_store::VectorMemoryProvider
// uses:
// - crate: amadeus_context (MemoryProvider trait)
// - service: /v1/embeddings (OpenAI-compatible endpoint)
// invariants:
// - VectorMemoryProvider is Send + Sync via internal Mutex.
// - Embedding backends are interchangeable behind the Embedder trait.
// side_effects:
// - Remote/kylin backends make HTTP calls to the embedding API.
// - VectorMemoryProvider reads/writes .amadeus/rag_index.json.
// tests:
// - cmd: cargo test -p rag
// @end-amadeus-header

//! RAG (Retrieval-Augmented Generation) crate.
//!
//! Provides embedding-based semantic search over documents:
//! - [`Embedder`](embedding::Embedder) — pluggable embedding backends:
//!   remote OpenAI-compatible HTTP, zero-dependency local hashing, and the
//!   Galaxy Kylin SDK adapter
//! - [`chunk_text`](chunker::chunk_text) — splits text into overlapping chunks at natural boundaries
//! - [`VectorMemoryProvider`](vector_store::VectorMemoryProvider) — persistent vector store implementing the `MemoryProvider` trait

pub mod chunker;
pub mod embedding;
pub mod kylin_embedding;
pub mod local_embedding;
pub mod tool;
pub mod vector_store;
