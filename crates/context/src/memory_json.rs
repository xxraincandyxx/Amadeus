//! JSON file-based memory provider with persistence.
//!
//! Stores memory entries in `.amadeus/memory.json` so they survive
//! server restarts. Entries are flushed to disk on every write.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::memory::{MemoryEntry, MemoryError, MemoryProvider};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct EntryJson {
    key: String,
    content: String,
    source: String,
}

impl From<&MemoryEntry> for EntryJson {
    fn from(e: &MemoryEntry) -> Self {
        Self {
            key: e.key.clone(),
            content: e.content.clone(),
            source: e.source.clone(),
        }
    }
}

impl From<EntryJson> for MemoryEntry {
    fn from(e: EntryJson) -> Self {
        MemoryEntry::new(e.key, e.content, e.source)
    }
}

/// A writable [`MemoryProvider`] that persists entries to a JSON file.
///
/// On construction, loads existing entries from `path` (typically
/// `.amadeus/memory.json`). Every `store` and `delete` immediately
/// flushes the full entry set to disk as a JSON array.
#[derive(Debug)]
pub struct JsonFileMemoryProvider {
    path: PathBuf,
    entries: Mutex<Vec<MemoryEntry>>,
}

impl JsonFileMemoryProvider {
    pub fn new(path: PathBuf) -> Self {
        let entries = Self::load_from_disk(&path);
        Self {
            path,
            entries: Mutex::new(entries),
        }
    }

    fn load_from_disk(path: &PathBuf) -> Vec<MemoryEntry> {
        match fs::read_to_string(path) {
            Ok(contents) => {
                if contents.trim().is_empty() {
                    return Vec::new();
                }
                match serde_json::from_str::<Vec<EntryJson>>(&contents) {
                    Ok(loaded) => loaded.into_iter().map(MemoryEntry::from).collect(),
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to parse memory file, starting fresh"
                        );
                        Vec::new()
                    }
                }
            }
            Err(_) => Vec::new(),
        }
    }

    fn flush_to_disk(&self) -> Result<(), MemoryError> {
        let entries: Vec<EntryJson> = self
            .entries
            .lock()
            .map_err(|e| MemoryError::WriteFailed(format!("lock poisoned: {}", e)))?
            .iter()
            .map(EntryJson::from)
            .collect();

        let json = serde_json::to_string_pretty(&entries).map_err(|e| {
            MemoryError::WriteFailed(format!("serialization failed: {}", e))
        })?;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                MemoryError::WriteFailed(format!("create dir failed: {}", e))
            })?;
        }

        fs::write(&self.path, json)
            .map_err(|e| MemoryError::WriteFailed(format!("write failed: {}", e)))
    }

    /// Delete an entry by key. Returns `NotFound` if no entry matches.
    pub fn delete(&self, key: &str) -> Result<(), MemoryError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|e| MemoryError::WriteFailed(format!("lock poisoned: {}", e)))?;
        let idx = entries
            .iter()
            .position(|e| e.key == key)
            .ok_or_else(|| MemoryError::NotFound(key.to_string()))?;
        entries.remove(idx);
        drop(entries);
        self.flush_to_disk()
    }
}

impl MemoryProvider for JsonFileMemoryProvider {
    fn name(&self) -> &'static str {
        "json_file"
    }

    fn load(&self) -> Vec<MemoryEntry> {
        self.entries.lock().map(|g| g.clone()).unwrap_or_default()
    }

    fn store(&self, entry: MemoryEntry) -> Result<(), MemoryError> {
        {
            let mut entries = self
                .entries
                .lock()
                .map_err(|e| MemoryError::WriteFailed(format!("lock poisoned: {}", e)))?;
            // Replace existing entry with same key, or append
            if let Some(existing) = entries.iter_mut().find(|e| e.key == entry.key) {
                *existing = entry;
            } else {
                entries.push(entry);
            }
        }
        self.flush_to_disk()
    }

    fn writable(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_creates_empty_when_no_file() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("memory.json");
        let provider = JsonFileMemoryProvider::new(path);
        assert!(provider.load().is_empty());
        assert!(provider.writable());
    }

    #[test]
    fn test_store_and_load() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("memory.json");
        let provider = JsonFileMemoryProvider::new(path.clone());

        provider
            .store(MemoryEntry::new("k1", "v1", "user"))
            .unwrap();
        provider
            .store(MemoryEntry::new("k2", "v2", "session"))
            .unwrap();

        let entries = provider.load();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].key, "k1");
        assert_eq!(entries[1].key, "k2");

        // Verify persistence: create a new provider from the same file
        let provider2 = JsonFileMemoryProvider::new(path);
        let entries2 = provider2.load();
        assert_eq!(entries2.len(), 2);
        assert_eq!(entries2[0].content, "v1");
        assert_eq!(entries2[1].content, "v2");
    }

    #[test]
    fn test_store_replaces_existing_key() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("memory.json");
        let provider = JsonFileMemoryProvider::new(path);

        provider
            .store(MemoryEntry::new("k1", "original", "user"))
            .unwrap();
        provider
            .store(MemoryEntry::new("k1", "updated", "user"))
            .unwrap();

        let entries = provider.load();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "updated");
    }

    #[test]
    fn test_delete_entry() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("memory.json");
        let provider = JsonFileMemoryProvider::new(path);

        provider
            .store(MemoryEntry::new("k1", "v1", "user"))
            .unwrap();
        provider
            .store(MemoryEntry::new("k2", "v2", "user"))
            .unwrap();

        provider.delete("k1").unwrap();
        let entries = provider.load();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "k2");

        // Delete non-existent
        assert!(provider.delete("nonexistent").is_err());
    }

    #[test]
    fn test_persistence_across_provider_instances() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("memory.json");

        {
            let provider = JsonFileMemoryProvider::new(path.clone());
            provider
                .store(MemoryEntry::new("persistent", "data", "user"))
                .unwrap();
            provider
                .store(MemoryEntry::new("also_persistent", "more data", "user"))
                .unwrap();
        }

        {
            let provider = JsonFileMemoryProvider::new(path);
            let entries = provider.load();
            assert_eq!(entries.len(), 2);
        }
    }

    #[test]
    fn test_empty_file_loads_as_empty() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("memory.json");
        fs::write(&path, "").unwrap();

        let provider = JsonFileMemoryProvider::new(path);
        assert!(provider.load().is_empty());
    }

    #[test]
    fn test_corrupt_file_loads_as_empty() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("memory.json");
        fs::write(&path, "not valid json {{").unwrap();

        let provider = JsonFileMemoryProvider::new(path);
        assert!(provider.load().is_empty());
    }
}
