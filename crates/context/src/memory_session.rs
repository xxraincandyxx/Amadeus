// @amadeus-header
// summary: Session-based memory provider that stores compaction summaries.
// layer: core
// status: active
// feature_flags: none
// provides:
// - type: crate::memory_session::SessionMemoryProvider
// uses:
// - module: crate::memory::{MemoryProvider, MemoryEntry, MemoryError}
// invariants:
// - Entries are stored in-memory and lost when the session ends.
// side_effects:
// - Writes mutate an internal Vec (no filesystem impact).
// tests:
// - cmd: cargo test -p context
// @end-amadeus-header

//! Session-scoped memory provider.

use std::sync::Mutex;

use crate::memory::{MemoryEntry, MemoryError, MemoryProvider};

/// Stores memory entries in-memory for the duration of a session.
///
/// Useful for persisting compaction summaries or user-injected context
/// that should be available across compactions but not persisted to disk.
#[derive(Debug)]
pub struct SessionMemoryProvider {
    entries: Mutex<Vec<MemoryEntry>>,
}

impl Default for SessionMemoryProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionMemoryProvider {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    /// Push a compaction summary entry.
    pub fn push_compaction_summary(&self, index: usize, summary: &str) {
        let entry = MemoryEntry {
            key: format!("compaction_{}", index),
            content: summary.to_string(),
            source: "compaction".into(),
        };
        self.entries.lock().unwrap().push(entry);
    }

    /// Push a generic user entry.
    pub fn push_entry(&self, key: &str, content: &str) {
        let entry = MemoryEntry {
            key: key.to_string(),
            content: content.to_string(),
            source: "session".into(),
        };
        self.entries.lock().unwrap().push(entry);
    }
}

impl MemoryProvider for SessionMemoryProvider {
    fn name(&self) -> &'static str {
        "session"
    }

    fn load(&self) -> Vec<MemoryEntry> {
        self.entries.lock().unwrap().clone()
    }

    fn store(&self, entry: MemoryEntry) -> Result<(), MemoryError> {
        self.entries.lock().unwrap().push(entry);
        Ok(())
    }

    fn writable(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_by_default() {
        let provider = SessionMemoryProvider::new();
        assert!(provider.load().is_empty());
    }

    #[test]
    fn test_push_and_load() {
        let provider = SessionMemoryProvider::new();
        provider.push_compaction_summary(0, "summary 1");
        provider.push_entry("user_note", "note content");

        let entries = provider.load();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].key, "compaction_0");
        assert_eq!(entries[1].key, "user_note");
    }

    #[test]
    fn test_writable() {
        let provider = SessionMemoryProvider::new();
        assert!(provider.writable());
        assert!(provider.store(MemoryEntry::new("k", "v", "test")).is_ok());
        assert_eq!(provider.load().len(), 1);
    }
}
