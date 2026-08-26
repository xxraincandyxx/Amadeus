// @amadeus-header
// summary: Mid-term memory store trait and JSON-backed reference implementation.
// layer: core
// status: active
// feature_flags: none
// provides:
// - trait: crate::store::MidTermStore
// - type: crate::store::MemoryQuery
// - type: crate::store::JsonMidTermStore
// - impl: amadeus_context::memory::MemoryProvider for JsonMidTermStore
// uses:
// - type: crate::record::{MemoryKind, MemoryRecord}
// - type: amadeus_context::memory::{MemoryEntry, MemoryError, MemoryProvider}
// invariants:
// - Keys are unique; upsert replaces by key and preserves created_at/access_count.
// - Every mutation flushes the versioned envelope to disk before returning.
// side_effects:
// - Writes `.amadeus/mid_term_memory.json` (or the configured path).
// tests:
// - cmd: cargo test -p memory
// @end-amadeus-header

//! Mid-term memory database: the [`MidTermStore`] trait and a JSON-file
//! reference implementation.
//!
//! The store persists gate output (`crate::record::MemoryRecord`) in a
//! versioned envelope so future format changes stay forward-compatible.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use amadeus_context::memory::{MemoryEntry, MemoryError, MemoryProvider};

use crate::record::{MemoryKind, MemoryRecord};

/// Current envelope format version.
const ENVELOPE_VERSION: u32 = 1;

/// Query filter for [`MidTermStore::query`]. All set fields are ANDed.
#[derive(Debug, Clone, Default)]
pub struct MemoryQuery {
    /// Only records of these kinds.
    pub kinds: Option<Vec<MemoryKind>>,
    /// Only records from this session.
    pub session_id: Option<String>,
    /// Only records with at least this importance.
    pub min_importance: Option<u8>,
    /// Only records whose key contains this substring (case-insensitive).
    pub key_contains: Option<String>,
    /// Maximum number of records to return.
    pub limit: Option<usize>,
}

impl MemoryQuery {
    /// Query matching everything.
    pub fn all() -> Self {
        Self::default()
    }
}

/// The mid-term memory database interface.
///
/// Implementations store [`MemoryRecord`]s keyed by `record.key`; storing a
/// record with an existing key replaces it (upsert semantics).
pub trait MidTermStore: Send + Sync + std::fmt::Debug {
    /// Unique name of this store implementation.
    fn name(&self) -> &'static str;

    /// Insert or replace a record by key.
    fn upsert(&self, record: MemoryRecord) -> Result<(), MemoryError>;

    /// Get a record by key.
    fn get(&self, key: &str) -> Option<MemoryRecord>;

    /// Delete a record by key. Returns `NotFound` if absent.
    fn delete(&self, key: &str) -> Result<(), MemoryError>;

    /// Query records matching the filter, newest first.
    fn query(&self, filter: &MemoryQuery) -> Vec<MemoryRecord>;

    /// Total number of stored records.
    fn count(&self) -> usize;

    /// Force persistence of in-memory state, if the store buffers writes.
    fn flush(&self) -> Result<(), MemoryError>;
}

/// Versioned on-disk envelope.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct StoreEnvelope {
    version: u32,
    updated_at: u64,
    records: Vec<MemoryRecord>,
}

impl Default for StoreEnvelope {
    fn default() -> Self {
        Self {
            version: ENVELOPE_VERSION,
            updated_at: crate::now_epoch_secs(),
            records: Vec::new(),
        }
    }
}

/// JSON-file-backed [`MidTermStore`], persisted as a versioned envelope.
///
/// Loads existing records on construction; every mutation flushes the full
/// envelope to disk (write-through, matching the `JsonFileMemoryProvider`
/// and `VectorMemoryProvider` precedent).
#[derive(Debug)]
pub struct JsonMidTermStore {
    path: PathBuf,
    envelope: Mutex<StoreEnvelope>,
}

impl JsonMidTermStore {
    /// Open (or create) the store at `path`, typically
    /// `{workdir}/.amadeus/mid_term_memory.json`.
    pub fn open(path: PathBuf) -> Self {
        let envelope = Self::load_from_disk(&path);
        Self {
            path,
            envelope: Mutex::new(envelope),
        }
    }

    fn load_from_disk(path: &PathBuf) -> StoreEnvelope {
        match fs::read_to_string(path) {
            Ok(contents) => {
                if contents.trim().is_empty() {
                    return StoreEnvelope::default();
                }
                match serde_json::from_str::<StoreEnvelope>(&contents) {
                    Ok(envelope) => envelope,
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to parse mid-term memory file, starting fresh"
                        );
                        StoreEnvelope::default()
                    }
                }
            }
            Err(_) => StoreEnvelope::default(),
        }
    }

    fn flush_locked(&self, envelope: &StoreEnvelope) -> Result<(), MemoryError> {
        let json = serde_json::to_string_pretty(envelope)
            .map_err(|e| MemoryError::WriteFailed(format!("serialization failed: {}", e)))?;
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| MemoryError::WriteFailed(format!("create dir failed: {}", e)))?;
            }
        }
        fs::write(&self.path, json)
            .map_err(|e| MemoryError::WriteFailed(format!("write failed: {}", e)))
    }
}

impl MidTermStore for JsonMidTermStore {
    fn name(&self) -> &'static str {
        "mid_term_json"
    }

    fn upsert(&self, record: MemoryRecord) -> Result<(), MemoryError> {
        let mut envelope = self
            .envelope
            .lock()
            .map_err(|e| MemoryError::WriteFailed(format!("lock poisoned: {}", e)))?;
        if let Some(existing) = envelope.records.iter_mut().find(|r| r.key == record.key) {
            let mut replacement = record;
            replacement.created_at = existing.created_at;
            replacement.access_count = existing.access_count;
            *existing = replacement;
        } else {
            envelope.records.push(record);
        }
        envelope.updated_at = crate::now_epoch_secs();
        self.flush_locked(&envelope)
    }

    fn get(&self, key: &str) -> Option<MemoryRecord> {
        self.envelope
            .lock()
            .ok()?
            .records
            .iter()
            .find(|r| r.key == key)
            .cloned()
    }

    fn delete(&self, key: &str) -> Result<(), MemoryError> {
        let mut envelope = self
            .envelope
            .lock()
            .map_err(|e| MemoryError::WriteFailed(format!("lock poisoned: {}", e)))?;
        let idx = envelope
            .records
            .iter()
            .position(|r| r.key == key)
            .ok_or_else(|| MemoryError::NotFound(key.to_string()))?;
        envelope.records.remove(idx);
        envelope.updated_at = crate::now_epoch_secs();
        self.flush_locked(&envelope)
    }

    fn query(&self, filter: &MemoryQuery) -> Vec<MemoryRecord> {
        let envelope = match self.envelope.lock() {
            Ok(guard) => guard,
            Err(_) => return Vec::new(),
        };
        let mut matched: Vec<MemoryRecord> = envelope
            .records
            .iter()
            .filter(|r| {
                if let Some(kinds) = &filter.kinds {
                    if !kinds.contains(&r.kind) {
                        return false;
                    }
                }
                if let Some(session) = &filter.session_id {
                    if r.session_id.as_deref() != Some(session.as_str()) {
                        return false;
                    }
                }
                if let Some(min) = filter.min_importance {
                    if r.importance < min {
                        return false;
                    }
                }
                if let Some(needle) = &filter.key_contains {
                    if !r.key.to_lowercase().contains(&needle.to_lowercase()) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();
        // Newest first: highest updated_at wins, key breaks ties for stability.
        matched.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.key.cmp(&b.key))
        });
        if let Some(limit) = filter.limit {
            matched.truncate(limit);
        }
        matched
    }

    fn count(&self) -> usize {
        self.envelope.lock().map(|g| g.records.len()).unwrap_or(0)
    }

    fn flush(&self) -> Result<(), MemoryError> {
        let envelope = self
            .envelope
            .lock()
            .map_err(|e| MemoryError::WriteFailed(format!("lock poisoned: {}", e)))?;
        self.flush_locked(&envelope)
    }
}

/// Adapter so the mid-term store plugs into the existing `MemoryRegistry`
/// ecosystem. Flat entries carry `source = "mid_term:<kind>"`.
impl MemoryProvider for JsonMidTermStore {
    fn name(&self) -> &'static str {
        "mid_term_json"
    }

    fn load(&self) -> Vec<MemoryEntry> {
        self.query(&MemoryQuery::all())
            .into_iter()
            .map(MemoryEntry::from)
            .collect()
    }

    fn store(&self, entry: MemoryEntry) -> Result<(), MemoryError> {
        let record =
            MemoryRecord::try_from(entry).map_err(|e| MemoryError::WriteFailed(e.to_string()))?;
        self.upsert(record)
    }

    fn delete(&self, key: &str) -> Result<(), MemoryError> {
        MidTermStore::delete(self, key)
    }

    fn writable(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(key: &str, kind: MemoryKind, importance: u8, session: Option<&str>) -> MemoryRecord {
        MemoryRecord::new(
            key,
            kind,
            format!("content for {}", key),
            session.map(String::from),
            [0, 0],
            importance,
        )
    }

    #[test]
    fn test_open_missing_file_starts_empty() {
        let temp = tempfile::TempDir::new().unwrap();
        let store = JsonMidTermStore::open(temp.path().join("mid_term_memory.json"));
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_upsert_get_delete() {
        let temp = tempfile::TempDir::new().unwrap();
        let store = JsonMidTermStore::open(temp.path().join("mid_term_memory.json"));

        store
            .upsert(record("task-a", MemoryKind::Task, 60, Some("s1")))
            .unwrap();
        store
            .upsert(record("fact-b", MemoryKind::Fact, 30, None))
            .unwrap();
        assert_eq!(store.count(), 2);
        assert_eq!(store.get("task-a").unwrap().kind, MemoryKind::Task);
        assert!(store.get("missing").is_none());

        MidTermStore::delete(&store, "task-a").unwrap();
        assert_eq!(store.count(), 1);
        assert!(matches!(
            MidTermStore::delete(&store, "task-a"),
            Err(MemoryError::NotFound(_))
        ));
    }

    #[test]
    fn test_upsert_replaces_by_key_preserving_metadata() {
        let temp = tempfile::TempDir::new().unwrap();
        let store = JsonMidTermStore::open(temp.path().join("mid_term_memory.json"));

        let mut original = record("file-src-lib-rs", MemoryKind::FileState, 50, None);
        original.created_at = 100;
        original.access_count = 7;
        store.upsert(original).unwrap();

        let replacement = record("file-src-lib-rs", MemoryKind::FileState, 90, None);
        store.upsert(replacement).unwrap();

        assert_eq!(store.count(), 1);
        let stored = store.get("file-src-lib-rs").unwrap();
        assert_eq!(stored.importance, 90);
        assert_eq!(stored.created_at, 100);
        assert_eq!(stored.access_count, 7);
    }

    #[test]
    fn test_query_filters() {
        let temp = tempfile::TempDir::new().unwrap();
        let store = JsonMidTermStore::open(temp.path().join("mid_term_memory.json"));

        store
            .upsert(record("task-a", MemoryKind::Task, 60, Some("s1")))
            .unwrap();
        store
            .upsert(record("decision-b", MemoryKind::Decision, 80, Some("s1")))
            .unwrap();
        store
            .upsert(record("task-c", MemoryKind::Task, 20, Some("s2")))
            .unwrap();

        let by_kind = store.query(&MemoryQuery {
            kinds: Some(vec![MemoryKind::Task]),
            ..Default::default()
        });
        assert_eq!(by_kind.len(), 2);

        let by_session = store.query(&MemoryQuery {
            session_id: Some("s1".into()),
            ..Default::default()
        });
        assert_eq!(by_session.len(), 2);

        let by_importance = store.query(&MemoryQuery {
            min_importance: Some(70),
            ..Default::default()
        });
        assert_eq!(by_importance.len(), 1);
        assert_eq!(by_importance[0].key, "decision-b");

        let by_key = store.query(&MemoryQuery {
            key_contains: Some("TASK".into()),
            ..Default::default()
        });
        assert_eq!(by_key.len(), 2);

        let limited = store.query(&MemoryQuery {
            limit: Some(1),
            ..Default::default()
        });
        assert_eq!(limited.len(), 1);
    }

    #[test]
    fn test_persistence_across_instances_and_envelope_version() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("mid_term_memory.json");

        {
            let store = JsonMidTermStore::open(path.clone());
            store
                .upsert(record("decision-x", MemoryKind::Decision, 80, Some("s1")))
                .unwrap();
        }

        let raw = fs::read_to_string(&path).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(envelope["version"], 1);
        assert!(envelope["records"].is_array());

        let store = JsonMidTermStore::open(path);
        assert_eq!(store.count(), 1);
        assert_eq!(
            store.get("decision-x").unwrap().session_id.as_deref(),
            Some("s1")
        );
    }

    #[test]
    fn test_corrupt_file_starts_fresh() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("mid_term_memory.json");
        fs::write(&path, "not json {{").unwrap();
        let store = JsonMidTermStore::open(path);
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_memory_provider_adapter() {
        let temp = tempfile::TempDir::new().unwrap();
        let store = JsonMidTermStore::open(temp.path().join("mid_term_memory.json"));

        let provider_entry = MemoryEntry::new("fact-y", "likes rust", "mid_term:fact");
        MemoryProvider::store(&store, provider_entry).unwrap();
        assert_eq!(store.count(), 1);

        let loaded = MemoryProvider::load(&store);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].source, "mid_term:fact");
        assert!(MemoryProvider::writable(&store));
        MemoryProvider::delete(&store, "fact-y").unwrap();
        assert_eq!(store.count(), 0);
    }
}
