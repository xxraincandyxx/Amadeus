// @amadeus-header
// summary: VectorMemoryProvider — persistent vector store with optional int8 quantization, implementing MemoryProvider.
// layer: core
// status: active
// feature_flags: none
// provides:
// - type: crate::vector_store::VectorMemoryProvider
// - type: crate::vector_store::DocumentInfo
// - type: crate::vector_store::Quantization
// - fn: crate::vector_store::cosine_similarity
// uses:
// - crate: amadeus_context::memory::{MemoryEntry, MemoryError, MemoryProvider}
// invariants:
// - Thread-safe via internal Mutex<Vec<VectorEntry>>.
// - Persists to .amadeus/rag_index.json as a versioned envelope (v2); legacy
//   bare-array files (v1) are migrated transparently on load.
// - Cosine similarity returns 0.0 for zero-vectors.
// - Entries with mismatched embedding dimensions are rejected at ingest.
// side_effects:
// - Reads/writes .amadeus/rag_index.json on construction and mutations.
// tests:
// - cmd: cargo test -p rag
// @end-amadeus-header

//! Vector-backed memory provider with cosine similarity search and optional
//! int8 scalar quantization (the "lightweight" edge-deployment mode: 4x less
//! memory and integer dot products at search time).
//!
//! Implements [`context::memory::MemoryProvider`] so entries integrate with
//! the existing memory registry. Additionally supports embedding-based
//! semantic search via [`VectorMemoryProvider::search`].

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use amadeus_context::memory::{MemoryEntry, MemoryError, MemoryProvider};
use serde::{Deserialize, Serialize};

/// On-disk envelope version. v1 was a bare JSON array of entries.
const STORE_VERSION: u32 = 2;

/// Embedding storage mode for the vector store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantization {
    /// Full-precision f32 vectors (default; matches historical behavior).
    None,
    /// Per-vector abs-max scalar quantization to i8: 4x smaller memory and
    /// disk footprint, integer dot products during search.
    Int8,
}

impl Quantization {
    /// Parse a config string (`"none"` / `"int8"`). Unknown values warn and
    /// fall back to `None` so a typo never silently changes recall.
    pub fn parse(value: &str) -> Self {
        match value {
            "none" => Self::None,
            "int8" => Self::Int8,
            other => {
                tracing::warn!(
                    quantization = %other,
                    "Unknown rag quantization, falling back to none"
                );
                Self::None
            }
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Int8 => "int8",
        }
    }
}

/// In-memory embedding representation.
#[derive(Debug, Clone)]
enum StoredEmbedding {
    /// Entries stored via `MemoryProvider::store` carry no embedding.
    Missing,
    Dense(Vec<f32>),
    Quantized {
        data: Vec<i8>,
        scale: f32,
        norm: f32,
    },
}

impl StoredEmbedding {
    fn dimension(&self) -> Option<usize> {
        match self {
            Self::Missing => None,
            Self::Dense(v) => Some(v.len()),
            Self::Quantized { data, .. } => Some(data.len()),
        }
    }
}

/// Quantize an f32 vector to i8 with per-vector abs-max scaling.
/// Returns the quantized bytes, the scale, and the ORIGINAL f32 norm.
fn quantize_int8(v: &[f32]) -> (Vec<i8>, f32, f32) {
    let abs_max = v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if abs_max < f32::EPSILON {
        return (vec![0; v.len()], 1.0, norm);
    }
    let scale = abs_max / 127.0;
    let data = v
        .iter()
        .map(|x| (x / scale).round().clamp(-127.0, 127.0) as i8)
        .collect();
    (data, scale, norm)
}

/// Cosine similarity between an f32 query and a stored embedding.
///
/// For quantized entries the query is quantized with the same abs-max rule
/// and the dot product runs on i32 accumulators; norms of the ORIGINAL f32
/// vectors keep the score on the cosine scale.
fn query_similarity(query: &[f32], stored: &StoredEmbedding) -> f32 {
    match stored {
        StoredEmbedding::Missing => 0.0,
        StoredEmbedding::Dense(v) => cosine_similarity(query, v),
        StoredEmbedding::Quantized { data, scale, norm } => {
            if query.len() != data.len() {
                return 0.0;
            }
            let (q_data, q_scale, q_norm) = quantize_int8(query);
            if q_norm < f32::EPSILON || *norm < f32::EPSILON {
                return 0.0;
            }
            let dot: i32 = q_data
                .iter()
                .zip(data.iter())
                .map(|(&a, &b)| a as i32 * b as i32)
                .sum();
            (dot as f32 * q_scale * scale) / (q_norm * norm)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChunkMetadata {
    document_id: String,
    chunk_index: usize,
    original_path: String,
    ingested_at: String,
}

/// v1 (legacy) entry shape: full-precision embedding, bare JSON array file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VectorEntryJson {
    key: String,
    content: String,
    source: String,
    embedding: Vec<f32>,
    metadata: ChunkMetadata,
}

/// v2 entry shape: `embedding` for dense, `embedding_i8`+`scale`+`norm` for
/// quantized entries. Exactly one representation is present.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VectorEntryJsonV2 {
    key: String,
    content: String,
    source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    embedding: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    embedding_i8: Option<Vec<i8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    norm: Option<f32>,
    metadata: ChunkMetadata,
}

/// v2 on-disk envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoreEnvelope {
    version: u32,
    quantization: String,
    dimension: Option<usize>,
    entries: Vec<VectorEntryJsonV2>,
}

#[derive(Debug, Clone)]
struct VectorEntry {
    entry: MemoryEntry,
    embedding: StoredEmbedding,
    metadata: ChunkMetadata,
}

impl VectorEntry {
    fn to_json(&self) -> VectorEntryJsonV2 {
        let (embedding, embedding_i8, scale, norm) = match &self.embedding {
            StoredEmbedding::Missing => (Some(Vec::new()), None, None, None),
            StoredEmbedding::Dense(v) => (Some(v.clone()), None, None, None),
            StoredEmbedding::Quantized { data, scale, norm } => {
                (None, Some(data.clone()), Some(*scale), Some(*norm))
            }
        };
        VectorEntryJsonV2 {
            key: self.entry.key.clone(),
            content: self.entry.content.clone(),
            source: self.entry.source.clone(),
            embedding,
            embedding_i8,
            scale,
            norm,
            metadata: self.metadata.clone(),
        }
    }

    fn from_json_v2(j: VectorEntryJsonV2) -> Self {
        let embedding = match (j.embedding, j.embedding_i8, j.scale, j.norm) {
            (Some(data), None, _, _) if !data.is_empty() => StoredEmbedding::Dense(data),
            (None, Some(data), Some(scale), Some(norm)) => {
                StoredEmbedding::Quantized { data, scale, norm }
            }
            _ => StoredEmbedding::Missing,
        };
        Self {
            entry: MemoryEntry::new(j.key, j.content, j.source),
            embedding,
            metadata: j.metadata,
        }
    }
}

/// Info about an ingested document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentInfo {
    pub id: String,
    pub chunk_count: usize,
    pub original_path: String,
    pub ingested_at: String,
}

/// A writable [`MemoryProvider`] with embedding-based semantic search.
///
/// Persists entries (with embeddings) to a JSON file. On construction,
/// loads existing entries from disk (legacy v1 bare-array files migrate
/// transparently). Thread-safe via internal `Mutex`.
#[derive(Debug)]
pub struct VectorMemoryProvider {
    path: PathBuf,
    quantization: Quantization,
    entries: Mutex<Vec<VectorEntry>>,
}

impl VectorMemoryProvider {
    /// Open the store with full-precision f32 embeddings.
    pub fn new(path: PathBuf) -> Self {
        Self::with_quantization(path, Quantization::None)
    }

    /// Open the store with the given embedding storage mode.
    pub fn with_quantization(path: PathBuf, quantization: Quantization) -> Self {
        let entries = Self::load_from_disk(&path);
        Self {
            path,
            quantization,
            entries: Mutex::new(entries),
        }
    }

    /// The storage mode this provider writes new embeddings in.
    pub fn quantization(&self) -> Quantization {
        self.quantization
    }

    // ── disk ──────────────────────────────────────────────────────────

    fn load_from_disk(path: &PathBuf) -> Vec<VectorEntry> {
        let contents = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        if contents.trim().is_empty() {
            return Vec::new();
        }
        // v2 files are a JSON object envelope; v1 files are a bare array.
        if contents.trim_start().starts_with('{') {
            match serde_json::from_str::<StoreEnvelope>(&contents) {
                Ok(envelope) => {
                    return envelope
                        .entries
                        .into_iter()
                        .map(VectorEntry::from_json_v2)
                        .collect()
                }
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "Failed to parse RAG index envelope, starting fresh"
                    );
                    return Vec::new();
                }
            }
        }
        match serde_json::from_str::<Vec<VectorEntryJson>>(&contents) {
            Ok(loaded) => {
                tracing::info!(
                    path = %path.display(),
                    "Migrating legacy v1 RAG index in memory"
                );
                loaded
                    .into_iter()
                    .map(|j| VectorEntry {
                        entry: MemoryEntry::new(j.key, j.content, j.source),
                        embedding: if j.embedding.is_empty() {
                            StoredEmbedding::Missing
                        } else {
                            StoredEmbedding::Dense(j.embedding)
                        },
                        metadata: j.metadata,
                    })
                    .collect()
            }
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "Failed to parse RAG index, starting fresh"
                );
                Vec::new()
            }
        }
    }

    fn flush_to_disk(&self) -> Result<(), MemoryError> {
        let guard = self
            .entries
            .lock()
            .map_err(|e| MemoryError::WriteFailed(format!("lock poisoned: {}", e)))?;
        let dimension = guard
            .iter()
            .find_map(|e| e.embedding.dimension())
            .unwrap_or(0);
        let envelope = StoreEnvelope {
            version: STORE_VERSION,
            quantization: self.quantization.as_str().to_string(),
            dimension: if dimension == 0 {
                None
            } else {
                Some(dimension)
            },
            entries: guard.iter().map(VectorEntry::to_json).collect(),
        };
        drop(guard);

        let json = serde_json::to_string_pretty(&envelope)
            .map_err(|e| MemoryError::WriteFailed(format!("serialization failed: {}", e)))?;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| MemoryError::WriteFailed(format!("create dir failed: {}", e)))?;
        }

        fs::write(&self.path, json)
            .map_err(|e| MemoryError::WriteFailed(format!("write failed: {}", e)))
    }

    // ── vector-specific operations ─────────────────────────────────────

    /// Ingest pre-chunked and pre-embedded document chunks.
    pub fn ingest_chunks(
        &self,
        document_id: &str,
        original_path: &str,
        chunks: Vec<String>,
        embeddings: Vec<Vec<f32>>,
    ) -> Result<usize, MemoryError> {
        if chunks.len() != embeddings.len() {
            return Err(MemoryError::WriteFailed(format!(
                "chunk count ({}) != embedding count ({})",
                chunks.len(),
                embeddings.len()
            )));
        }

        let now = chrono_now();
        let mut entries = self
            .entries
            .lock()
            .map_err(|e| MemoryError::WriteFailed(format!("lock poisoned: {}", e)))?;

        // Dimension consistency: reject embeddings that do not match the
        // dimensions already stored (mixing models corrupts search results).
        if let Some(expected) = entries.iter().find_map(|e| e.embedding.dimension()) {
            if let Some(bad) = embeddings.iter().find(|e| e.len() != expected) {
                return Err(MemoryError::WriteFailed(format!(
                    "embedding dimension mismatch: store holds {}, got {}",
                    expected,
                    bad.len()
                )));
            }
        }

        let count = chunks.len();
        for (i, (chunk, embedding)) in chunks.into_iter().zip(embeddings).enumerate() {
            let key = format!("rag:{}:chunk_{}", document_id, i);
            let stored = match self.quantization {
                Quantization::None => StoredEmbedding::Dense(embedding),
                Quantization::Int8 => {
                    let (data, scale, norm) = quantize_int8(&embedding);
                    StoredEmbedding::Quantized { data, scale, norm }
                }
            };
            let entry = VectorEntry {
                entry: MemoryEntry::new(&key, chunk, format!("rag:{}", document_id)),
                embedding: stored,
                metadata: ChunkMetadata {
                    document_id: document_id.to_string(),
                    chunk_index: i,
                    original_path: original_path.to_string(),
                    ingested_at: now.clone(),
                },
            };
            // Replace existing with same key
            if let Some(existing) = entries.iter_mut().find(|e| e.entry.key == key) {
                *existing = entry;
            } else {
                entries.push(entry);
            }
        }

        drop(entries);
        self.flush_to_disk()?;
        Ok(count)
    }

    /// Semantic search: return top-k entries by cosine similarity to `query_embedding`.
    ///
    /// Entries without embeddings never match. A query whose dimension does
    /// not match the stored embeddings logs a warning and returns no results.
    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Vec<(MemoryEntry, f32)> {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());

        if let Some(expected) = entries.iter().find_map(|e| e.embedding.dimension()) {
            if expected != query_embedding.len() {
                tracing::warn!(
                    expected = expected,
                    got = query_embedding.len(),
                    "RAG query embedding dimension mismatch, returning no results"
                );
                return Vec::new();
            }
        }

        let mut scored: Vec<(&VectorEntry, f32)> = entries
            .iter()
            .filter(|e| e.embedding.dimension().is_some())
            .map(|e| (e, query_similarity(query_embedding, &e.embedding)))
            .collect();

        // Sort descending by score
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .into_iter()
            .take(top_k)
            .filter(|(_, score)| *score > 0.0)
            .map(|(e, score)| (e.entry.clone(), score))
            .collect()
    }

    /// List all ingested documents.
    pub fn list_documents(&self) -> Vec<DocumentInfo> {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());

        let mut docs: std::collections::HashMap<String, DocumentInfo> =
            std::collections::HashMap::new();
        for entry in entries.iter() {
            let doc_id = &entry.metadata.document_id;
            docs.entry(doc_id.clone())
                .and_modify(|info| info.chunk_count += 1)
                .or_insert(DocumentInfo {
                    id: doc_id.clone(),
                    chunk_count: 1,
                    original_path: entry.metadata.original_path.clone(),
                    ingested_at: entry.metadata.ingested_at.clone(),
                });
        }
        docs.into_values().collect()
    }

    /// Delete all chunks for a document ID.
    pub fn delete_document(&self, document_id: &str) -> Result<usize, MemoryError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|e| MemoryError::WriteFailed(format!("lock poisoned: {}", e)))?;

        let before = entries.len();
        entries.retain(|e| e.metadata.document_id != document_id);
        let removed = before - entries.len();

        drop(entries);
        self.flush_to_disk()?;
        Ok(removed)
    }
}

// ── MemoryProvider impl ─────────────────────────────────────────────────────

impl MemoryProvider for VectorMemoryProvider {
    fn name(&self) -> &'static str {
        "vector_rag"
    }

    fn load(&self) -> Vec<MemoryEntry> {
        self.entries
            .lock()
            .map(|g| g.iter().map(|e| e.entry.clone()).collect())
            .unwrap_or_default()
    }

    fn store(&self, entry: MemoryEntry) -> Result<(), MemoryError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|e| MemoryError::WriteFailed(format!("lock poisoned: {}", e)))?;

        // Store without embedding (for non-RAG usage)
        let vec_entry = VectorEntry {
            entry,
            embedding: StoredEmbedding::Missing,
            metadata: ChunkMetadata {
                document_id: "manual".to_string(),
                chunk_index: 0,
                original_path: String::new(),
                ingested_at: chrono_now(),
            },
        };
        if let Some(existing) = entries
            .iter_mut()
            .find(|e| e.entry.key == vec_entry.entry.key)
        {
            *existing = vec_entry;
        } else {
            entries.push(vec_entry);
        }

        drop(entries);
        self.flush_to_disk()
    }

    fn delete(&self, key: &str) -> Result<(), MemoryError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|e| MemoryError::WriteFailed(format!("lock poisoned: {}", e)))?;
        let idx = entries
            .iter()
            .position(|e| e.entry.key == key)
            .ok_or_else(|| MemoryError::NotFound(key.to_string()))?;
        entries.remove(idx);
        drop(entries);
        self.flush_to_disk()
    }

    fn writable(&self) -> bool {
        true
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

/// Cosine similarity between two f32 vectors. Returns 0.0 for zero-norm
/// inputs. Note: callers must ensure equal dimensions (zip truncates).
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let (dot, na, nb) = a
        .iter()
        .zip(b.iter())
        .fold((0.0f32, 0.0f32, 0.0f32), |(d, na, nb), (&x, &y)| {
            (d + x * y, na + x * x, nb + y * y)
        });
    if na < f32::EPSILON || nb < f32::EPSILON {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

fn chrono_now() -> String {
    // Simple ISO-8601 timestamp without chrono dependency
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Format: YYYY-MM-DDTHH:MM:SSZ
    let (year, month, day, hour, min, sec) = unix_to_utc(secs);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, min, sec
    )
}

fn unix_to_utc(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let days = secs / 86400;
    let time = secs % 86400;
    let hour = time / 3600;
    let min = (time % 3600) / 60;
    let sec = time % 60;

    // Days since 1970-01-01
    let (year, month, day) = days_to_date(days as i64);
    (year, month, day, hour, min, sec)
}

fn days_to_date(days: i64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era as u64 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

// ── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_creates_empty_when_no_file() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("rag_index.json");
        let provider = VectorMemoryProvider::new(path);
        assert!(provider.load().is_empty());
        assert!(provider.writable());
        assert_eq!(provider.name(), "vector_rag");
        assert_eq!(provider.quantization(), Quantization::None);
    }

    #[test]
    fn test_ingest_and_search() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("rag_index.json");
        let provider = VectorMemoryProvider::new(path);

        let embeddings = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let chunks = vec![
            "apple fruit".to_string(),
            "banana fruit".to_string(),
            "car vehicle".to_string(),
        ];

        let count = provider
            .ingest_chunks("test_doc", "/tmp/test.txt", chunks, embeddings)
            .unwrap();
        assert_eq!(count, 3);

        // Search with a query embedding close to "apple"
        let results = provider.search(&[0.9, 0.1, 0.0], 2);
        assert_eq!(results.len(), 2);
        // First result should be "apple fruit" (highest cosine similarity)
        assert!(results[0].0.content.contains("apple"));
    }

    #[test]
    fn test_list_and_delete_document() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("rag_index.json");
        let provider = VectorMemoryProvider::new(path);

        let embeddings = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let chunks = vec!["chunk a".to_string(), "chunk b".to_string()];

        provider
            .ingest_chunks("doc1", "/tmp/doc1.md", chunks, embeddings)
            .unwrap();

        let docs = provider.list_documents();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].id, "doc1");
        assert_eq!(docs[0].chunk_count, 2);

        let removed = provider.delete_document("doc1").unwrap();
        assert_eq!(removed, 2);
        assert!(provider.load().is_empty());
    }

    #[test]
    fn test_persistence() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("rag_index.json");

        {
            let provider = VectorMemoryProvider::new(path.clone());
            provider
                .ingest_chunks(
                    "persist_test",
                    "/tmp/persist.md",
                    vec!["data".to_string()],
                    vec![vec![1.0, 2.0, 3.0]],
                )
                .unwrap();
        }

        {
            let provider = VectorMemoryProvider::new(path);
            let entries = provider.load();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].key, "rag:persist_test:chunk_0");

            let results = provider.search(&[1.0, 2.0, 3.0], 1);
            assert_eq!(results.len(), 1);
            assert!(results[0].1 > 0.99); // nearly identical
        }
    }

    #[test]
    fn test_store_and_delete_memory_provider_trait() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("rag_index.json");
        let provider = VectorMemoryProvider::new(path);

        provider
            .store(MemoryEntry::new("k1", "v1", "user"))
            .unwrap();
        let entries = provider.load();
        assert_eq!(entries.len(), 1);

        provider.delete("k1").unwrap();
        assert!(provider.load().is_empty());

        assert!(provider.delete("nonexistent").is_err());
    }

    #[test]
    fn test_cosine_similarity() {
        assert!((cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 0.001);
        assert!((cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]) - 0.0).abs() < 0.001);
        assert!(cosine_similarity(&[1.0, 1.0], &[-1.0, -1.0]) < -0.9);

        // Zero vectors
        assert!(cosine_similarity(&[0.0, 0.0], &[1.0, 0.0]).abs() < 0.001);
    }

    #[test]
    fn test_corrupt_file_loads_as_empty() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("rag_index.json");
        fs::write(&path, "not valid json {{").unwrap();

        let provider = VectorMemoryProvider::new(path);
        assert!(provider.load().is_empty());
    }

    #[test]
    fn test_dimension_mismatch_rejected_at_ingest() {
        let temp = TempDir::new().unwrap();
        let provider = VectorMemoryProvider::new(temp.path().join("rag_index.json"));
        provider
            .ingest_chunks("d1", "/a", vec!["x".to_string()], vec![vec![1.0, 0.0]])
            .unwrap();
        let result =
            provider.ingest_chunks("d2", "/b", vec!["y".to_string()], vec![vec![1.0, 0.0, 0.0]]);
        assert!(result.is_err());
        // Search with a mismatched query returns nothing instead of
        // silently truncating dimensions.
        assert!(provider.search(&[1.0, 0.0, 0.0], 5).is_empty());
    }

    #[test]
    fn test_entries_without_embeddings_never_match() {
        let temp = TempDir::new().unwrap();
        let provider = VectorMemoryProvider::new(temp.path().join("rag_index.json"));
        provider
            .store(MemoryEntry::new("k1", "v1", "user"))
            .unwrap();
        assert!(provider.search(&[1.0, 0.0], 5).is_empty());
    }

    #[test]
    fn test_quantize_int8_round_trip_preserves_direction() {
        let original: Vec<f32> = (0..64).map(|i| ((i * 7) as f32 % 13.0) - 6.0).collect();
        let (data, scale, norm) = quantize_int8(&original);
        assert_eq!(data.len(), original.len());
        let dequant: Vec<f32> = data.iter().map(|x| *x as f32 * scale).collect();
        let cos = cosine_similarity(&original, &dequant);
        assert!(cos > 0.995, "quantized cosine {} too low", cos);
        let true_norm = original.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - true_norm).abs() < 1e-4);
    }

    #[test]
    fn test_int8_search_matches_f32_top_k() {
        let temp = TempDir::new().unwrap();
        let dense = VectorMemoryProvider::new(temp.path().join("dense.json"));
        let quant = VectorMemoryProvider::with_quantization(
            temp.path().join("quant.json"),
            Quantization::Int8,
        );

        // Deterministic pseudo-random embeddings, 64 dimensions.
        let embeddings: Vec<Vec<f32>> = (0..20)
            .map(|d| {
                (0..64)
                    .map(|i| ((d * 31 + i * 17) % 23) as f32 / 23.0 - 0.5)
                    .collect()
            })
            .collect();
        let chunks: Vec<String> = (0..20).map(|i| format!("chunk {}", i)).collect();

        dense
            .ingest_chunks("doc", "/d", chunks.clone(), embeddings.clone())
            .unwrap();
        quant
            .ingest_chunks("doc", "/d", chunks, embeddings)
            .unwrap();

        let query: Vec<f32> = (0..64)
            .map(|i| ((i * 13) % 19) as f32 / 19.0 - 0.5)
            .collect();
        let dense_top: Vec<String> = dense
            .search(&query, 5)
            .into_iter()
            .map(|(e, _)| e.key)
            .collect();
        let quant_top: Vec<String> = quant
            .search(&query, 5)
            .into_iter()
            .map(|(e, _)| e.key)
            .collect();
        assert_eq!(dense_top, quant_top);
    }

    #[test]
    fn test_int8_persistence_round_trip() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("rag_index.json");
        {
            let provider =
                VectorMemoryProvider::with_quantization(path.clone(), Quantization::Int8);
            provider
                .ingest_chunks(
                    "doc",
                    "/d",
                    vec!["data".to_string()],
                    vec![vec![0.5, -0.25, 1.0]],
                )
                .unwrap();
        }
        // On disk the entry must be in the quantized representation.
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"version\": 2"));
        assert!(raw.contains("embedding_i8"));
        assert!(raw.contains("\"quantization\": \"int8\""));

        let provider = VectorMemoryProvider::with_quantization(path, Quantization::Int8);
        let results = provider.search(&[0.5, -0.25, 1.0], 1);
        assert_eq!(results.len(), 1);
        assert!(results[0].1 > 0.99);
    }

    #[test]
    fn test_legacy_v1_file_migrates() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("rag_index.json");
        let legacy = serde_json::json!([{
            "key": "rag:old:chunk_0",
            "content": "legacy chunk",
            "source": "rag:old",
            "embedding": [1.0, 0.0, 0.0],
            "metadata": {
                "document_id": "old",
                "chunk_index": 0,
                "original_path": "/old.md",
                "ingested_at": "2026-01-01T00:00:00Z"
            }
        }]);
        fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

        let provider = VectorMemoryProvider::new(path.clone());
        let results = provider.search(&[0.9, 0.1, 0.0], 1);
        assert_eq!(results.len(), 1);
        assert!(results[0].0.content.contains("legacy"));

        // Next mutation rewrites the file as a v2 envelope.
        provider
            .ingest_chunks(
                "new",
                "/n",
                vec!["c".to_string()],
                vec![vec![0.0, 1.0, 0.0]],
            )
            .unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"version\": 2"));
    }

    #[test]
    fn test_quantization_parse() {
        assert_eq!(Quantization::parse("none"), Quantization::None);
        assert_eq!(Quantization::parse("int8"), Quantization::Int8);
        assert_eq!(Quantization::parse("bogus"), Quantization::None);
    }
}
