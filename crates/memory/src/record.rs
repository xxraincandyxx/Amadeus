// @amadeus-header
// summary: Mid-term memory record data model defining what the gate stores.
// layer: core
// status: active
// feature_flags: none
// provides:
// - type: crate::record::MemoryKind
// - type: crate::record::MemoryRecord
// uses:
// - type: amadeus_context::memory::MemoryEntry
// - protocol: serde serialization
// invariants:
// - Record keys are unique upsert keys; ids are stable per record identity.
// - Timestamps are unix epoch seconds so records stay dependency-light.
// side_effects: none
// tests:
// - cmd: cargo test -p memory
// @end-amadeus-header

//! Data model for mid-term memory records.
//!
//! The record model is the crate's core interface: it decides what kind of
//! data survives the trip from short-term context into the mid-term memory
//! database. Kinds mirror the compaction summarization strategy documented
//! in `docs/COMPACTION.md`: key tasks, decisions, files modified, errors
//! and resolutions, and the current-state snapshot.

use amadeus_context::memory::MemoryEntry;
use serde::{Deserialize, Serialize};

use crate::now_epoch_secs;

/// Classification of a mid-term memory record.
///
/// Mirrors the summarization focuses of the compaction gate strategy
/// (`docs/COMPACTION.md`, LLM summarization step).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    /// Key task or objective extracted from user intent.
    Task,
    /// Important decision made during the session.
    Decision,
    /// File created or modified by a tool operation.
    FileState,
    /// Error encountered and how it was resolved.
    ErrorResolution,
    /// Compaction state snapshot ("current state" of the conversation).
    StateSnapshot,
    /// Standalone fact stored directly by a user or the LLM.
    Fact,
}

impl MemoryKind {
    /// Stable identifier used in record keys and `MemoryEntry::source`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Decision => "decision",
            Self::FileState => "file_state",
            Self::ErrorResolution => "error_resolution",
            Self::StateSnapshot => "state_snapshot",
            Self::Fact => "fact",
        }
    }
}

/// A single mid-term memory record — the unit of storage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryRecord {
    /// Stable unique id, e.g. `mt:<session>:<seq>`.
    pub id: String,
    /// Human-readable upsert key; records with equal keys replace each other.
    pub key: String,
    /// What kind of data this record holds.
    pub kind: MemoryKind,
    /// The memory content.
    pub content: String,
    /// Which session produced this record, if known.
    pub session_id: Option<String>,
    /// Inclusive range of context message indices this record was derived
    /// from, as `[start, end]`.
    pub source_message_indices: [usize; 2],
    /// Creation time, unix epoch seconds.
    pub created_at: u64,
    /// Last update time, unix epoch seconds.
    pub updated_at: u64,
    /// How many times this record has been read back.
    pub access_count: u32,
    /// Gate-assigned importance, 0–100; drives retention and eviction.
    pub importance: u8,
}

impl MemoryRecord {
    /// Build a new record with fresh timestamps and zero access count.
    pub fn new(
        key: impl Into<String>,
        kind: MemoryKind,
        content: impl Into<String>,
        session_id: Option<String>,
        source_message_indices: [usize; 2],
        importance: u8,
    ) -> Self {
        let now = now_epoch_secs();
        Self {
            id: format!(
                "mt:{}:{}",
                session_id.as_deref().unwrap_or("anon"),
                key_to_slug(&key)
            ),
            key: key.into(),
            kind,
            content: content.into(),
            session_id,
            source_message_indices,
            created_at: now,
            updated_at: now,
            access_count: 0,
            importance,
        }
    }
}

/// Convert a record into the flat [`MemoryEntry`] shape used by the existing
/// `MemoryRegistry`/`MemoryProvider` ecosystem.
impl From<MemoryRecord> for MemoryEntry {
    fn from(r: MemoryRecord) -> Self {
        MemoryEntry::new(r.key, r.content, format!("mid_term:{}", r.kind.as_str()))
    }
}

/// Extract kind and content back out of a `MemoryEntry` produced by the
/// conversion above. Returns `None` for entries from other sources.
impl TryFrom<MemoryEntry> for MemoryRecord {
    type Error = &'static str;

    fn try_from(e: MemoryEntry) -> Result<Self, Self::Error> {
        let kind = match e.source.as_str() {
            "mid_term:task" => MemoryKind::Task,
            "mid_term:decision" => MemoryKind::Decision,
            "mid_term:file_state" => MemoryKind::FileState,
            "mid_term:error_resolution" => MemoryKind::ErrorResolution,
            "mid_term:state_snapshot" => MemoryKind::StateSnapshot,
            "mid_term:fact" => MemoryKind::Fact,
            _ => return Err("entry source is not a mid_term record"),
        };
        Ok(MemoryRecord::new(e.key, kind, e.content, None, [0, 0], 0))
    }
}

/// Deterministic slug used inside record ids.
pub(crate) fn key_to_slug(key: &str) -> String {
    let slug: String = key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    slug.trim_matches('-').to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_str_round_trips_through_serde() {
        for kind in [
            MemoryKind::Task,
            MemoryKind::Decision,
            MemoryKind::FileState,
            MemoryKind::ErrorResolution,
            MemoryKind::StateSnapshot,
            MemoryKind::Fact,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: MemoryKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[test]
    fn record_serde_round_trip() {
        let record = MemoryRecord::new(
            "decision-use-json-store",
            MemoryKind::Decision,
            "Use a JSON store for the mid-term database.",
            Some("s1".into()),
            [3, 17],
            80,
        );
        let json = serde_json::to_string(&record).unwrap();
        let back: MemoryRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record, back);
        assert_eq!(record.source_message_indices, [3, 17]);
        assert!(record.id.starts_with("mt:s1:"));
    }

    #[test]
    fn memory_entry_conversion_round_trip() {
        let record = MemoryRecord::new(
            "file-src-lib-rs",
            MemoryKind::FileState,
            "src/lib.rs modified",
            None,
            [0, 0],
            50,
        );
        let entry = MemoryEntry::from(record);
        assert_eq!(entry.source, "mid_term:file_state");
        let back = MemoryRecord::try_from(entry).unwrap();
        assert_eq!(back.kind, MemoryKind::FileState);
        assert_eq!(back.key, "file-src-lib-rs");
        assert_eq!(back.content, "src/lib.rs modified");
    }

    #[test]
    fn try_from_rejects_foreign_sources() {
        let entry = MemoryEntry::new("k", "v", "user");
        assert!(MemoryRecord::try_from(entry).is_err());
    }
}
