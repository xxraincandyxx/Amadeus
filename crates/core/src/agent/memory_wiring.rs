// @amadeus-header
// summary: Wiring that connects the preference, mid-term, and privacy memory crates into the agent loop.
// layer: agent
// status: active
// feature_flags: none
// provides:
// - fn: crate::agent::memory_wiring::open_preference_store
// - fn: crate::agent::memory_wiring::ingest_tool_execution
// - fn: crate::agent::memory_wiring::archive_compacted_context
// uses:
// - type: amadeus_context::preference::PreferenceStore
// - type: amadeus_memory::gate::RuleBasedGate
// - type: amadeus_memory::store::JsonMidTermStore
// - type: amadeus_privacy::SensitiveDataDetector
// - artifact: .amadeus/preferences.json
// - artifact: .amadeus/mid_term_memory.json
// invariants:
// - Memory wiring never fails the agent loop; every error is logged and swallowed.
// - Preference evidence and mid-term records are redacted before persistence.
// side_effects:
// - Reads and writes .amadeus/preferences.json and .amadeus/mid_term_memory.json.
// tests:
// - cmd: cargo test -p core agent::memory_wiring
// @end-amadeus-header

//! Integration seams for the memory indicators (2), (5) boundary, and (6).
//!
//! The feature crates stay independent and deterministic; this module owns
//! the side-effecting glue that connects them to the running agent:
//!
//! - **Indicator 2** — a workspace [`PreferenceStore`] is opened per agent
//!   and every tool execution is ingested as a [`ToolCallRecord`].
//! - **Indicator 6** — when compaction retires context, the
//!   [`RuleBasedGate`] transforms the retired messages into records persisted
//!   in a [`JsonMidTermStore`].
//! - **Indicator 5 (boundary)** — all evidence and record content crosses
//!   the [`SensitiveDataDetector`] before it touches disk.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use amadeus_compaction::CompactionResult;
use amadeus_context::preference::{PreferenceStore, ScenarioTag, ToolCallRecord};
use amadeus_memory::gate::{ContextGate, GateInput, RuleBasedGate};
use amadeus_memory::store::{JsonMidTermStore, MidTermStore};
use amadeus_messages::Message;
use amadeus_privacy::SensitiveDataDetector;
use tracing::{debug, warn};

/// Process-wide detector; regex compilation happens once.
fn shared_detector() -> &'static SensitiveDataDetector {
    static DETECTOR: OnceLock<SensitiveDataDetector> = OnceLock::new();
    DETECTOR.get_or_init(SensitiveDataDetector::new)
}

/// Open the workspace preference store (`{workdir}/.amadeus/preferences.json`).
///
/// Construction is infallible: the file is created lazily on the first
/// mutation, and a corrupt existing file falls back to a fresh store with a
/// warning (see [`PreferenceStore::new`]).
pub fn open_preference_store(workdir: &Path) -> Arc<PreferenceStore> {
    Arc::new(PreferenceStore::new(
        workdir.join(".amadeus").join("preferences.json"),
    ))
}

/// Feed one executed tool call into the preference store (indicator 2).
///
/// The tool output is redacted before it can become preference evidence, and
/// the raw tool input is never attached (it may carry secrets in arguments).
pub fn ingest_tool_execution(
    store: &PreferenceStore,
    tool: &str,
    output: &str,
    is_error: bool,
    duration_ms: u64,
) {
    let mut record = ToolCallRecord::new(
        tool,
        "",
        !is_error,
        amadeus_context::preference::now_ms(),
        ScenarioTag::default(),
    );
    record.latency_ms = duration_ms;
    let (redacted, spans) = shared_detector().redact(output);
    record.output = Some(redacted);
    match store.ingest_tool_result(record) {
        Ok(changes) => {
            if !changes.is_empty() {
                debug!(tool = %tool, ?changes, redacted_spans = spans.len(), "preference updated");
            }
        }
        Err(error) => warn!(tool = %tool, error = %error, "preference ingestion failed"),
    }
}

/// Archive retired context into the mid-term memory store (indicator 6).
///
/// Runs the rule-based gate over the messages that compaction is about to
/// drop plus the compaction summary, redacts every record (indicator 5
/// boundary), and upserts into `{workdir}/.amadeus/mid_term_memory.json`.
pub fn archive_compacted_context(
    workdir: &Path,
    session_id: &str,
    retired: &[Message],
    result: &CompactionResult,
) {
    let gate = RuleBasedGate::default();
    let mut records = match gate.transform(&GateInput {
        session_id: Some(session_id),
        messages: retired,
        compaction: Some(result),
    }) {
        Ok(records) => records,
        Err(error) => {
            warn!(error = %error, "mid-term gate transform failed");
            return;
        }
    };
    if records.is_empty() {
        return;
    }
    let detector = shared_detector();
    for record in &mut records {
        let (content, _) = detector.redact(&record.content);
        record.content = content;
        let (key, _) = detector.redact(&record.key);
        if key != record.key {
            // The id embeds a slug of the original key; rebuild it so the
            // unredacted key never persists through the id.
            record.id = format!(
                "mt:{}:{}",
                record.session_id.as_deref().unwrap_or("anon"),
                amadeus_memory::record::key_to_slug(&key)
            );
            record.key = key;
        }
    }
    let store = JsonMidTermStore::open(workdir.join(".amadeus").join("mid_term_memory.json"));
    let mut archived = 0usize;
    for record in records {
        match store.upsert(record) {
            Ok(()) => archived += 1,
            Err(error) => warn!(error = %error, "mid-term record upsert failed"),
        }
    }
    debug!(archived, session_id = %session_id, "mid-term memory archived");
}

#[cfg(test)]
mod tests {
    use super::*;
    use amadeus_compaction::CompressionStatus;

    #[test]
    fn tool_executions_create_a_tool_choice_preference() {
        let temp = tempfile::TempDir::new().unwrap();
        let store = open_preference_store(temp.path());
        ingest_tool_execution(&store, "read_file", "src/lib.rs contents", false, 12);
        ingest_tool_execution(&store, "read_file", "src/main.rs contents", false, 9);
        assert!(
            store.all().iter().any(|p| p.id.contains("file_ops")),
            "expected a learned tool_choice preference, got {:?}",
            store.all()
        );
        let raw = std::fs::read_to_string(temp.path().join(".amadeus/preferences.json")).unwrap();
        assert!(raw.contains("read_file"));
    }

    #[test]
    fn ingestion_redacts_sensitive_output() {
        let temp = tempfile::TempDir::new().unwrap();
        let store = open_preference_store(temp.path());
        ingest_tool_execution(
            &store,
            "read_file",
            "contact 13800138000 for details",
            false,
            5,
        );
        ingest_tool_execution(&store, "read_file", "contact 13800138000 again", false, 4);
        let raw = std::fs::read_to_string(temp.path().join(".amadeus/preferences.json")).unwrap();
        assert!(
            !raw.contains("13800138000"),
            "store leaked sensitive output"
        );
    }

    #[test]
    fn compaction_archive_writes_redacted_records() {
        let temp = tempfile::TempDir::new().unwrap();
        let retired = vec![Message::user(
            "Call 13800138000 then implement the memory module for the agent runtime.",
        )];
        let result = CompactionResult {
            original_count: 7,
            compacted_count: 2,
            original_tokens: 1000,
            new_tokens: 300,
            tokens_saved: 700,
            summary: Some("Current state: memory module done. Contact test@example.com.".into()),
            messages_summarized: 5,
            status: CompressionStatus::Compressed,
        };
        archive_compacted_context(temp.path(), "s1", &retired, &result);
        let raw =
            std::fs::read_to_string(temp.path().join(".amadeus/mid_term_memory.json")).unwrap();
        assert!(raw.contains("implement the memory module"));
        assert!(!raw.contains("test@example.com"));
        // The phone number appears in the task key; neither key nor id may leak it.
        assert!(
            !raw.contains("13800138000"),
            "store leaked sensitive digits"
        );
    }
}
