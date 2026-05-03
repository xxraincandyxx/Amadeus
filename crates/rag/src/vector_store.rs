// @amadeus-header
// summary: VectorMemoryProvider — persistent vector store implementing MemoryProvider trait.
// layer: core
// status: active
// feature_flags: none
// provides:
// - type: crate::vector_store::VectorMemoryProvider
// - type: crate::vector_store::DocumentInfo
// - fn: crate::vector_store::cosine_similarity
// uses:
// - crate: amadeus_context::memory::{MemoryEntry, MemoryError, MemoryProvider}
// invariants:
// - Thread-safe via internal Mutex<Vec<VectorEntry>>.
// - Persists to .amadeus/rag_index.json as a JSON array.
// - Cosine similarity returns 0.0 for zero-vectors.
// side_effects:
// - Reads/writes .amadeus/rag_index.json on construction and mutations.
// tests:
// - cmd: cargo test -p rag
// @end-amadeus-header

//! Vector-backed memory provider with cosine similarity search.
//!
//! Implements [`context::memory::MemoryProvider`] so entries integrate with
//! the existing memory registry. Additionally supports embedding-based
//! semantic search via [`VectorMemoryProvider::search`].

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use amadeus_context::memory::{MemoryEntry, MemoryError, MemoryProvider};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChunkMetadata {
    document_id: String,
    chunk_index: usize,
    original_path: String,
    ingested_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VectorEntryJson {
    key: String,
    content: String,
    source: String,
    embedding: Vec<f32>,
    metadata: ChunkMetadata,
}

#[derive(Debug, Clone)]
struct VectorEntry {
    entry: MemoryEntry,
    embedding: Vec<f32>,
    metadata: ChunkMetadata,
}

impl From<&VectorEntry> for VectorEntryJson {
    fn from(e: &VectorEntry) -> Self {
        Self {
            key: e.entry.key.clone(),
            content: e.entry.content.clone(),
            source: e.entry.source.clone(),
            embedding: e.embedding.clone(),
            metadata: e.metadata.clone(),
        }
    }
}

impl From<VectorEntryJson> for VectorEntry {
    fn from(j: VectorEntryJson) -> Self {
        Self {
            entry: MemoryEntry::new(j.key, j.content, j.source),
            embedding: j.embedding,
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
/// loads existing entries from disk. Thread-safe via internal `Mutex`.
#[derive(Debug)]
pub struct VectorMemoryProvider {
    path: PathBuf,
    entries: Mutex<Vec<VectorEntry>>,
}

impl VectorMemoryProvider {
    pub fn new(path: PathBuf) -> Self {
        let entries = Self::load_from_disk(&path);
        Self {
            path,
            entries: Mutex::new(entries),
        }
    }

    // ── disk ──────────────────────────────────────────────────────────

    fn load_from_disk(path: &PathBuf) -> Vec<VectorEntry> {
        match fs::read_to_string(path) {
            Ok(contents) => {
                if contents.trim().is_empty() {
                    return Vec::new();
                }
                match serde_json::from_str::<Vec<VectorEntryJson>>(&contents) {
                    Ok(loaded) => loaded.into_iter().map(VectorEntry::from).collect(),
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
            Err(_) => Vec::new(),
        }
    }

    fn flush_to_disk(&self) -> Result<(), MemoryError> {
        let entries: Vec<VectorEntryJson> = self
            .entries
            .lock()
            .map_err(|e| MemoryError::WriteFailed(format!("lock poisoned: {}", e)))?
            .iter()
            .map(VectorEntryJson::from)
            .collect();

        let json = serde_json::to_string_pretty(&entries)
            .map_err(|e| MemoryError::WriteFailed(format!("serialization failed: {}", e)))?;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                MemoryError::WriteFailed(format!("create dir failed: {}", e))
            })?;
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

        let count = chunks.len();
        for (i, (chunk, embedding)) in chunks.into_iter().zip(embeddings).enumerate() {
            let key = format!("rag:{}:chunk_{}", document_id, i);
            let entry = VectorEntry {
                entry: MemoryEntry::new(&key, chunk, format!("rag:{}", document_id)),
                embedding,
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
    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Vec<(MemoryEntry, f32)> {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let mut scored: Vec<(&VectorEntry, f32)> = entries
            .iter()
            .map(|e| (e, cosine_similarity(query_embedding, &e.embedding)))
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

        let mut docs: std::collections::HashMap<String, DocumentInfo> = std::collections::HashMap::new();
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
            embedding: Vec::new(),
            metadata: ChunkMetadata {
                document_id: "manual".to_string(),
                chunk_index: 0,
                original_path: String::new(),
                ingested_at: chrono_now(),
            },
        };
        if let Some(existing) = entries.iter_mut().find(|e| e.entry.key == vec_entry.entry.key) {
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

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
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
}
