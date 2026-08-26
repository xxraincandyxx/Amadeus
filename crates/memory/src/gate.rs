// @amadeus-header
// summary: Context gate transforming conversation context into mid-term memory records.
// layer: core
// status: active
// feature_flags: none
// provides:
// - trait: crate::gate::ContextGate
// - type: crate::gate::GateInput
// - type: crate::gate::GateConfig
// - type: crate::gate::RuleBasedGate
// - type: crate::gate::GateReport
// uses:
// - type: amadeus_messages::Message
// - type: amadeus_compaction::CompactionResult
// - type: crate::record::{MemoryKind, MemoryRecord}
// invariants:
// - Gate transformations are deterministic; identical input yields identical records.
// - Record keys are deduplicated within one transform; later records win.
// side_effects: none
// tests:
// - cmd: cargo test -p memory
// @end-amadeus-header

//! The context gate: transforms short-term context (conversation history and
//! compaction results) into mid-term memory records.
//!
//! The strategy mirrors the compaction summarization focuses documented in
//! `docs/COMPACTION.md` — key tasks/objectives, decisions, files modified,
//! errors and resolutions, and the current-state snapshot. The gate decides
//! what kind of data crosses from context into the mid-term database.

use amadeus_compaction::CompactionResult;
use amadeus_messages::{ContentBlock, Message};

use crate::record::{MemoryKind, MemoryRecord};

/// Input to a gate transformation: the context being retired or compacted.
#[derive(Debug, Clone, Copy, Default)]
pub struct GateInput<'a> {
    /// Session the context belongs to, used as record provenance.
    pub session_id: Option<&'a str>,
    /// The conversation messages leaving the short-term context window.
    pub messages: &'a [Message],
    /// The compaction result, when the gate runs as part of compaction.
    /// Its summary becomes the `StateSnapshot` record.
    pub compaction: Option<&'a CompactionResult>,
}

/// Tuning knobs for [`RuleBasedGate`].
#[derive(Debug, Clone)]
pub struct GateConfig {
    /// Importance floor applied when no kind-specific value exists (0–100).
    pub default_importance: u8,
    /// Maximum records produced by a single transform; excess is dropped.
    pub max_records_per_transform: usize,
    /// Minimum content length (chars) for a record to be emitted.
    pub min_content_chars: usize,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            default_importance: 50,
            max_records_per_transform: 64,
            min_content_chars: 8,
        }
    }
}

/// Outcome of a gate transform, for logging and telemetry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GateReport {
    /// Records emitted, by kind.
    pub emitted: usize,
    /// Candidates rejected for being too short.
    pub skipped_short: usize,
    /// Candidates dropped because the per-transform cap was hit.
    pub dropped_over_cap: usize,
}

/// The context-to-memory gate.
///
/// Implementations decide which parts of short-term context survive into the
/// mid-term memory database, and as what [`MemoryKind`]. A future LLM-backed
/// gate can implement this trait with a summarizer behind it.
pub trait ContextGate: Send + Sync + std::fmt::Debug {
    /// Unique name of this gate implementation.
    fn name(&self) -> &'static str;

    /// Transform context into mid-term memory records.
    fn transform(
        &self,
        input: &GateInput<'_>,
    ) -> Result<Vec<MemoryRecord>, amadeus_context::memory::MemoryError>;
}

/// Deterministic, rule-based gate. No LLM calls.
///
/// Extraction rules (following the compaction summarization strategy):
///
/// | Source | Kind | Trigger |
/// |---|---|---|
/// | User text message | [`MemoryKind::Task`] | message text at least `min_content_chars` |
/// | Assistant text | [`MemoryKind::Decision`] | text contains "decide"/"decision" |
/// | Tool-use block | [`MemoryKind::FileState`] | input contains a file path |
/// | Tool-result block | [`MemoryKind::ErrorResolution`] | content mentions "error" |
/// | Compaction summary | [`MemoryKind::StateSnapshot`] | `input.compaction` present |
#[derive(Debug, Clone)]
pub struct RuleBasedGate {
    config: GateConfig,
}

impl RuleBasedGate {
    /// Create a gate with custom configuration.
    pub fn new(config: GateConfig) -> Self {
        Self { config }
    }

    /// Kind-specific importance following the compaction strategy's
    /// priorities: decisions and snapshots outrank tasks and file touches.
    fn kind_importance(&self, kind: MemoryKind) -> u8 {
        match kind {
            MemoryKind::Decision => 80,
            MemoryKind::StateSnapshot => 75,
            MemoryKind::ErrorResolution => 70,
            MemoryKind::Task => 60,
            MemoryKind::FileState => 50,
            MemoryKind::Fact => 40,
        }
        .max(self.config.default_importance)
    }
}

impl Default for RuleBasedGate {
    fn default() -> Self {
        Self::new(GateConfig::default())
    }
}

impl ContextGate for RuleBasedGate {
    fn name(&self) -> &'static str {
        "rule_based"
    }

    fn transform(
        &self,
        input: &GateInput<'_>,
    ) -> Result<Vec<MemoryRecord>, amadeus_context::memory::MemoryError> {
        let mut report = GateReport::default();
        let mut records: Vec<MemoryRecord> = Vec::new();
        let push =
            |record: MemoryRecord, report: &mut GateReport, records: &mut Vec<MemoryRecord>| {
                if record.content.chars().count() < self.config.min_content_chars {
                    report.skipped_short += 1;
                    return;
                }
                if records.len() >= self.config.max_records_per_transform {
                    report.dropped_over_cap += 1;
                    return;
                }
                // Deduplicate by key within one transform; later records win.
                if let Some(existing) = records.iter_mut().find(|r| r.key == record.key) {
                    *existing = record;
                } else {
                    records.push(record);
                }
                report.emitted += 1;
            };

        for (index, message) in input.messages.iter().enumerate() {
            for block in &message.content {
                match block {
                    ContentBlock::Text { text } => {
                        if message.role == "user" {
                            let objective = first_sentence(text);
                            push(
                                MemoryRecord::new(
                                    format!("task:{}", slug(&objective)),
                                    MemoryKind::Task,
                                    objective,
                                    input.session_id.map(String::from),
                                    [index, index],
                                    self.kind_importance(MemoryKind::Task),
                                ),
                                &mut report,
                                &mut records,
                            );
                        } else if message.role == "assistant"
                            && contains_any(text, &["decide", "decision"])
                        {
                            push(
                                MemoryRecord::new(
                                    format!("decision:{}", slug(text)),
                                    MemoryKind::Decision,
                                    text.clone(),
                                    input.session_id.map(String::from),
                                    [index, index],
                                    self.kind_importance(MemoryKind::Decision),
                                ),
                                &mut report,
                                &mut records,
                            );
                        }
                    }
                    ContentBlock::ToolUse {
                        input: tool_input, ..
                    } => {
                        for path in extract_paths(tool_input) {
                            push(
                                MemoryRecord::new(
                                    format!("file:{}", slug(&path)),
                                    MemoryKind::FileState,
                                    format!("File touched: {}", path),
                                    input.session_id.map(String::from),
                                    [index, index],
                                    self.kind_importance(MemoryKind::FileState),
                                ),
                                &mut report,
                                &mut records,
                            );
                        }
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        if content.to_lowercase().contains("error") {
                            push(
                                MemoryRecord::new(
                                    format!("error:{}", slug(content)),
                                    MemoryKind::ErrorResolution,
                                    content.clone(),
                                    input.session_id.map(String::from),
                                    [index, index],
                                    self.kind_importance(MemoryKind::ErrorResolution),
                                ),
                                &mut report,
                                &mut records,
                            );
                        }
                    }
                }
            }
        }

        if let Some(compaction) = input.compaction {
            if let Some(summary) = &compaction.summary {
                let summarized = compaction.messages_summarized;
                push(
                    MemoryRecord::new(
                        format!(
                            "snapshot:{}:{}",
                            input.session_id.unwrap_or("anon"),
                            summarized
                        ),
                        MemoryKind::StateSnapshot,
                        summary.clone(),
                        input.session_id.map(String::from),
                        [0, summarized.saturating_sub(1)],
                        self.kind_importance(MemoryKind::StateSnapshot),
                    ),
                    &mut report,
                    &mut records,
                );
            }
        }

        tracing::debug!(?report, gate = self.name(), "context gate transform done");
        Ok(records)
    }
}

/// First sentence (or first 120 chars) of a text, as the task objective.
fn first_sentence(text: &str) -> String {
    let trimmed = text.trim();
    let cut = trimmed.find(['.', '!', '?', '\n']).unwrap_or(trimmed.len());
    let sentence = &trimmed[..cut.min(trimmed.len())];
    sentence.chars().take(120).collect()
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    let lower = haystack.to_lowercase();
    needles.iter().any(|n| lower.contains(n))
}

/// Extract file-path-looking string values from a tool-use input JSON object.
fn extract_paths(input: &serde_json::Value) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(map) = input.as_object() {
        for (key, value) in map {
            let path_keys = ["path", "file_path", "file", "notebook_path"];
            if path_keys.contains(&key.as_str()) {
                if let Some(s) = value.as_str() {
                    if s.contains('/') || s.contains('.') {
                        paths.push(s.to_string());
                    }
                }
            }
        }
    }
    paths
}

fn slug(text: &str) -> String {
    let slug: String = text
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = slug.trim_matches('-').to_lowercase();
    trimmed.chars().take(60).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn compaction_with_summary(summary: &str, summarized: usize) -> CompactionResult {
        CompactionResult {
            original_count: summarized + 6,
            compacted_count: 7,
            original_tokens: 1000,
            new_tokens: 300,
            tokens_saved: 700,
            summary: Some(summary.to_string()),
            messages_summarized: summarized,
            status: amadeus_compaction::CompressionStatus::Compressed,
        }
    }

    #[test]
    fn user_text_becomes_task() {
        let messages = vec![Message::user("Implement the memory module. Then test it.")];
        let gate = RuleBasedGate::default();
        let records = gate
            .transform(&GateInput {
                session_id: Some("s1"),
                messages: &messages,
                compaction: None,
            })
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, MemoryKind::Task);
        assert_eq!(records[0].content, "Implement the memory module");
        assert_eq!(records[0].session_id.as_deref(), Some("s1"));
        assert_eq!(records[0].importance, 60);
        assert!(records[0]
            .key
            .starts_with("task:implement-the-memory-module"));
    }

    #[test]
    fn assistant_decision_text_becomes_decision() {
        let messages = vec![Message::assistant(vec![ContentBlock::Text {
            text: "Decision: we will use a JSON envelope for the store.".into(),
        }])];
        let gate = RuleBasedGate::default();
        let records = gate
            .transform(&GateInput {
                session_id: None,
                messages: &messages,
                compaction: None,
            })
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, MemoryKind::Decision);
        assert_eq!(records[0].importance, 80);
    }

    #[test]
    fn tool_use_with_path_becomes_file_state() {
        let messages = vec![Message::assistant(vec![ContentBlock::ToolUse {
            id: "t1".into(),
            name: "write_file".into(),
            input: json!({"file_path": "src/lib.rs", "content": "fn main() {}"}),
        }])];
        let gate = RuleBasedGate::default();
        let records = gate
            .transform(&GateInput {
                session_id: None,
                messages: &messages,
                compaction: None,
            })
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, MemoryKind::FileState);
        assert_eq!(records[0].content, "File touched: src/lib.rs");
        assert_eq!(records[0].key, "file:src-lib-rs");
    }

    #[test]
    fn error_tool_result_becomes_error_resolution() {
        let messages = vec![Message::tool_results(vec![ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: "error: compilation failed in lib.rs".into(),
        }])];
        let gate = RuleBasedGate::default();
        let records = gate
            .transform(&GateInput {
                session_id: None,
                messages: &messages,
                compaction: None,
            })
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, MemoryKind::ErrorResolution);
        assert_eq!(records[0].importance, 70);
    }

    #[test]
    fn compaction_summary_becomes_state_snapshot() {
        let messages = vec![Message::user("some earlier work")];
        let compaction = compaction_with_summary("Current state: memory module designed.", 12);
        let gate = RuleBasedGate::default();
        let records = gate
            .transform(&GateInput {
                session_id: Some("s9"),
                messages: &messages,
                compaction: Some(&compaction),
            })
            .unwrap();
        let snapshot = records
            .iter()
            .find(|r| r.kind == MemoryKind::StateSnapshot)
            .expect("snapshot record");
        assert_eq!(snapshot.content, "Current state: memory module designed.");
        assert_eq!(snapshot.source_message_indices, [0, 11]);
        assert_eq!(snapshot.key, "snapshot:s9:12");
    }

    #[test]
    fn short_content_is_skipped() {
        let messages = vec![Message::user("hi")];
        let gate = RuleBasedGate::default();
        let records = gate
            .transform(&GateInput {
                session_id: None,
                messages: &messages,
                compaction: None,
            })
            .unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn duplicate_keys_collapse_within_one_transform() {
        let messages = vec![
            Message::user("Refactor the store layer."),
            Message::user("Refactor the store layer."),
        ];
        let gate = RuleBasedGate::default();
        let records = gate
            .transform(&GateInput {
                session_id: None,
                messages: &messages,
                compaction: None,
            })
            .unwrap();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn cap_limits_record_count() {
        let messages: Vec<Message> = (0..10)
            .map(|i| Message::user(&format!("Do unique task number {} now please", i)))
            .collect();
        let gate = RuleBasedGate::new(GateConfig {
            max_records_per_transform: 3,
            ..Default::default()
        });
        let records = gate
            .transform(&GateInput {
                session_id: None,
                messages: &messages,
                compaction: None,
            })
            .unwrap();
        assert_eq!(records.len(), 3);
    }

    #[test]
    fn empty_input_yields_no_records() {
        let gate = RuleBasedGate::default();
        let records = gate
            .transform(&GateInput {
                session_id: None,
                messages: &[],
                compaction: None,
            })
            .unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn records_round_trip_through_store() {
        use crate::store::{JsonMidTermStore, MemoryQuery, MidTermStore};

        let temp = tempfile::TempDir::new().unwrap();
        let store = JsonMidTermStore::open(temp.path().join("mid_term_memory.json"));
        let messages = vec![
            Message::user("Implement the memory module."),
            Message::tool_results(vec![ContentBlock::ToolResult {
                tool_use_id: "t".into(),
                content: "error: build failed".into(),
            }]),
        ];
        let gate = RuleBasedGate::default();
        let records = gate
            .transform(&GateInput {
                session_id: Some("s1"),
                messages: &messages,
                compaction: None,
            })
            .unwrap();
        assert_eq!(records.len(), 2);
        for record in records {
            store.upsert(record).unwrap();
        }
        let decisions_or_errors = store.query(&MemoryQuery {
            kinds: Some(vec![MemoryKind::ErrorResolution]),
            ..Default::default()
        });
        assert_eq!(decisions_or_errors.len(), 1);
        assert_eq!(store.count(), 2);
    }
}
