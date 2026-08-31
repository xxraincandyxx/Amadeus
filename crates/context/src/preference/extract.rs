// @amadeus-header
// summary: Rule-based automatic preference extraction from tool results, user behavior, and manual config.
// layer: core
// status: active
// feature_flags: none
// provides:
// - type: crate::preference::extract::ToolCallRecord
// - type: crate::preference::extract::BehaviorRecord
// - type: crate::preference::extract::ManualConfigRecord
// - type: crate::preference::extract::PreferenceChange
// - fn: crate::preference::extract::confidence_from_counts
// - fn: crate::preference::extract::infer_category_from_config_key
// - fn: crate::preference::extract::infer_task_from_tool
// - fn: crate::preference::extract::is_safety_sensitive
// uses:
// - type: crate::preference::Evidence
// - type: crate::preference::PreferenceCategory
// - type: crate::preference::PreferenceSource
// - type: crate::preference::PreferenceStore
// - type: crate::preference::ScenarioTag
// - type: crate::preference::StoredPreference
// - type: crate::preference::VersionDraft
// - format: serde_json values
// invariants:
// - Extraction thresholds keep produced confidence in [0.3, 0.95].
// - A recorded failure never deletes a preference; it only downgrades it.
// - Deterministic inputs produce deterministic extraction outcomes.
// side_effects:
// - Persists counters and versioned preferences through the store.
// tests:
// - cmd: cargo test -p context
// @end-amadeus-header

//! Automatic preference extraction over multi-source records.
//!
//! Three ingestion entry points feed rule-based extractors:
//! - [`PreferenceStore::ingest_tool_result`] — tool-choice preferences from
//!   tool execution results (the primary data source for the OS Agent).
//! - [`PreferenceStore::ingest_user_behavior`] — operation habits and
//!   correction-driven "avoid" preferences from user behavior data.
//! - [`PreferenceStore::ingest_manual_config`] — explicit preferences from
//!   manually configured settings.
//!
//! All extractors are deterministic heuristics that run fully on-device and
//! never require an LLM call.

use serde::{Deserialize, Serialize};

use super::{
    Evidence, PreferenceCategory, PreferenceError, PreferenceSource, PreferenceStore, ScenarioTag,
    StoredPreference, VersionDraft,
};

const MIN_TOOL_SAMPLES: u32 = 2;
const TOOL_SUCCESS_RATE: f32 = 0.8;
const TOOL_USAGE_SHARE: f32 = 0.6;
const MIN_FAIL_FOR_DOWNGRADE: u32 = 2;
const DOWNGRADE_FAIL_RATE: f32 = 0.5;
const DOWNGRADE_STEP: f32 = 0.15;
const MIN_CONFIDENCE: f32 = 0.3;
const HABIT_MIN_COUNT: u32 = 2;
const MANUAL_CONFIG_CONFIDENCE: f32 = 0.98;
const MAX_LATENCY_SAMPLES: usize = 64;

/// A single tool execution result record.
///
/// `task` is the normalized task kind (e.g. `file_ops`); when empty it is
/// inferred from the tool name. `input`/`output` are kept as optional
/// evidence and truncated when attached to a preference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub tool: String,
    pub task: String,
    pub ok: bool,
    pub latency_ms: u64,
    pub timestamp: i64,
    pub scenario: ScenarioTag,
    pub input: Option<serde_json::Value>,
    pub output: Option<String>,
}

impl ToolCallRecord {
    pub fn new(
        tool: impl Into<String>,
        task: impl Into<String>,
        ok: bool,
        timestamp: i64,
        scenario: ScenarioTag,
    ) -> Self {
        Self {
            tool: tool.into(),
            task: task.into(),
            ok,
            latency_ms: 0,
            timestamp,
            scenario,
            input: None,
            output: None,
        }
    }
}

/// A single user behavior record (an action observed in a scenario, with an
/// optional correction that signals the agent's action was rejected).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorRecord {
    pub action: String,
    pub scenario: ScenarioTag,
    pub timestamp: i64,
    pub correction: Option<String>,
}

/// A manually configured setting that is treated as an explicit preference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualConfigRecord {
    pub key: String,
    pub value: String,
    pub scenario: Option<ScenarioTag>,
    pub timestamp: i64,
}

/// The outcome of one ingestion pass: what changed in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreferenceChange {
    Created {
        id: String,
        version: u64,
    },
    Updated {
        id: String,
        version: u64,
        reason: String,
    },
    Noop,
}

/// Monotonic confidence in `[0.0, 0.95]` from raw success/failure counts.
///
/// Confidence rises with both the success rate and the volume of evidence;
/// it saturates at 0.95 so a single streak can never reach certainty.
pub fn confidence_from_counts(success: u32, fail: u32) -> f32 {
    let total = success + fail;
    if total == 0 {
        return 0.0;
    }
    let rate = success as f32 / total as f32;
    let volume = (total as f32 / (total as f32 + 4.0)).min(1.0);
    let conf = 0.45 + 0.35 * rate + 0.15 * volume;
    // The + 1e-4 epsilon guards against IEEE-754 ties like 0.45+0.35+0.075
    // computing to 0.8749999x and rounding down to 0.87 instead of 0.88.
    ((conf.min(0.95) * 100.0) + 0.0001).round() / 100.0
}

/// Map a config key to a preference category by keyword.
pub fn infer_category_from_config_key(key: &str) -> PreferenceCategory {
    let k = key.to_ascii_lowercase();
    if contains_any(
        &k,
        &[
            "language", "style", "format", "theme", "verbose", "output", "response",
        ],
    ) {
        PreferenceCategory::OutputStyle
    } else if contains_any(
        &k,
        &[
            "permission",
            "approval",
            "blocked",
            "allow",
            "safety",
            "secret",
            "audit",
        ],
    ) {
        PreferenceCategory::SafetyPolicy
    } else if contains_any(&k, &["tool", "editor", "shell", "search", "model"]) {
        PreferenceCategory::ToolChoice
    } else if contains_any(&k, &["workflow", "flow", "step", "pipeline", "plan"]) {
        PreferenceCategory::Workflow
    } else if contains_any(&k, &["habit", "frequent", "shortcut", "recent"]) {
        PreferenceCategory::Habit
    } else {
        PreferenceCategory::General
    }
}

/// Infer a normalized task kind from a tool name when the record did not
/// provide one (e.g. `read_file` -> `file_ops`).
pub fn infer_task_from_tool(tool: &str) -> Option<String> {
    let t = tool.to_ascii_lowercase();
    if t.contains("file") || t.contains("read") || t.contains("write") || t.contains("edit") {
        Some("file_ops".into())
    } else if t.contains("search") || t.contains("grep") || t.contains("query") {
        Some("search".into())
    } else if t.contains("web") || t.contains("http") || t.contains("fetch") {
        Some("web".into())
    } else if t.contains("bash") || t.contains("shell") || t.contains("exec") {
        Some("shell".into())
    } else if t.contains("memory") || t.contains("rag") || t.contains("embed") {
        Some("memory".into())
    } else {
        None
    }
}

/// Whether an action touches security-sensitive operations.
pub fn is_safety_sensitive(action: &str) -> bool {
    let a = action.to_ascii_lowercase();
    contains_any(
        &a,
        &[
            "sudo", "delete", "chmod", "chown", "format", "secret", "password", "token", "env",
            "key", "drop ", "rm ",
        ],
    )
}

impl PreferenceStore {
    /// Extract tool-choice preferences from a tool execution result.
    ///
    /// The extractor maintains per-(task, tool) success/failure counters and:
    /// 1. creates `tool_choice::{task}` once a tool passes the sample count,
    ///    success-rate, and usage-share thresholds;
    /// 2. reinforces or switches the preference when another tool clearly
    ///    outperforms the current one;
    /// 3. downgrades the preference when the preferred tool starts failing.
    pub fn ingest_tool_result(
        &self,
        record: ToolCallRecord,
    ) -> Result<Vec<PreferenceChange>, PreferenceError> {
        let mut data = self.inner.lock().map_err(PreferenceError::from_poison)?;
        let task = normalize_task(&record.task, &record.tool);
        let stats_key = format!("{task}::{}", record.tool);

        let stat = data.tool_stats.entry(stats_key.clone()).or_default();
        if record.ok {
            stat.success += 1;
        } else {
            stat.fail += 1;
        }
        stat.last_ts = record.timestamp;
        if record.latency_ms > 0 {
            stat.latencies_ms.push(record.latency_ms);
            if stat.latencies_ms.len() > MAX_LATENCY_SAMPLES {
                stat.latencies_ms.remove(0);
            }
        }

        let pref_id = format!("tool_choice::{task}");
        let mut changes = Vec::new();

        if let Some((tool, success, total)) = best_tool_for_task(&data.tool_stats, &task) {
            let success_rate = success as f32 / total as f32;
            let share = task_usage_share(&data.tool_stats, &task, &tool);
            if total >= MIN_TOOL_SAMPLES
                && success_rate >= TOOL_SUCCESS_RATE
                && share >= TOOL_USAGE_SHARE
            {
                let conf = confidence_from_counts(success, total - success);
                let now = record.timestamp;
                match data.preferences.get(&pref_id).cloned() {
                    None => {
                        let draft = VersionDraft {
                            category: PreferenceCategory::ToolChoice,
                            value: tool.clone(),
                            confidence: conf,
                            source: PreferenceSource::ToolResult,
                            scenario: record.scenario.clone(),
                            timestamp: now,
                            reason: "auto_extracted".into(),
                            evidence: vec![evidence_from_tool_record(&record)],
                        };
                        data.preferences.insert(
                            pref_id.clone(),
                            StoredPreference::new(
                                pref_id.clone(),
                                format!("{task} preferred tool"),
                                draft,
                            ),
                        );
                        data.revision += 1;
                        changes.push(PreferenceChange::Created {
                            id: pref_id.clone(),
                            version: 1,
                        });
                    }
                    Some(mut stored) => {
                        let current = &stored.current;
                        let should_switch = current.value != tool
                            && total >= MIN_TOOL_SAMPLES
                            && data
                                .tool_stats
                                .get(&format!("{task}::{}", current.value))
                                .map(|s| success > s.success)
                                .unwrap_or(false);
                        let should_reinforce =
                            current.value == tool && conf > current.confidence + 0.02;
                        if should_switch {
                            let version = stored.push_version(VersionDraft {
                                category: PreferenceCategory::ToolChoice,
                                value: tool.clone(),
                                confidence: conf,
                                source: PreferenceSource::ToolResult,
                                scenario: record.scenario.clone(),
                                timestamp: now,
                                reason: format!(
                                    "auto_switched: '{tool}' outperforms '{}'",
                                    current.value
                                ),
                                evidence: vec![evidence_from_tool_record(&record)],
                            });
                            data.preferences.insert(pref_id.clone(), stored);
                            data.revision += 1;
                            changes.push(PreferenceChange::Updated {
                                id: pref_id.clone(),
                                version,
                                reason: "auto_switched".into(),
                            });
                        } else if should_reinforce {
                            let version = stored.push_version(VersionDraft {
                                category: PreferenceCategory::ToolChoice,
                                value: tool.clone(),
                                confidence: conf,
                                source: PreferenceSource::ToolResult,
                                scenario: record.scenario.clone(),
                                timestamp: now,
                                reason: "auto_reinforced".into(),
                                evidence: vec![evidence_from_tool_record(&record)],
                            });
                            data.preferences.insert(pref_id.clone(), stored);
                            data.revision += 1;
                            changes.push(PreferenceChange::Updated {
                                id: pref_id.clone(),
                                version,
                                reason: "auto_reinforced".into(),
                            });
                        }
                    }
                }
            }
        }

        if !record.ok {
            let stored = data.preferences.get(&pref_id).cloned();
            let stat = data.tool_stats.get(&stats_key);
            let should_downgrade = stored
                .as_ref()
                .map(|s| s.current.value == record.tool)
                .unwrap_or(false)
                && stat
                    .map(|s| {
                        let total = s.success + s.fail;
                        let fail_rate = if total == 0 {
                            0.0
                        } else {
                            s.fail as f32 / total as f32
                        };
                        s.fail >= MIN_FAIL_FOR_DOWNGRADE && fail_rate > DOWNGRADE_FAIL_RATE
                    })
                    .unwrap_or(false);
            if should_downgrade {
                if let (Some(mut stored), Some(stat)) = (stored, stat) {
                    let new_conf = (stored.current.confidence - DOWNGRADE_STEP).max(MIN_CONFIDENCE);
                    let version = stored.push_version(VersionDraft {
                        category: PreferenceCategory::ToolChoice,
                        value: record.tool.clone(),
                        confidence: new_conf,
                        source: PreferenceSource::ToolResult,
                        scenario: record.scenario.clone(),
                        timestamp: record.timestamp,
                        reason: format!(
                            "auto_downgraded: '{}' failing ({} ok / {} fail)",
                            record.tool, stat.success, stat.fail
                        ),
                        evidence: vec![evidence_from_tool_record(&record)],
                    });
                    data.preferences.insert(pref_id.clone(), stored);
                    data.revision += 1;
                    changes.push(PreferenceChange::Updated {
                        id: pref_id,
                        version,
                        reason: "auto_downgraded".into(),
                    });
                }
            }
        }

        if !changes.is_empty() {
            self.persist_locked(&data)?;
        }
        Ok(changes)
    }

    /// Extract operation habits and correction-driven "avoid" preferences
    /// from user behavior records.
    ///
    /// A repeated action in the same scenario becomes a `habit::{scenario}::{action}`
    /// preference. A non-empty `correction` instead creates or reinforces an
    /// `avoid::{action}` preference and weakens any conflicting habit.
    pub fn ingest_user_behavior(
        &self,
        record: BehaviorRecord,
    ) -> Result<Vec<PreferenceChange>, PreferenceError> {
        let mut data = self.inner.lock().map_err(PreferenceError::from_poison)?;
        let mut changes = Vec::new();
        let now = record.timestamp;

        if let Some(correction) = &record.correction {
            let pref_id = format!("avoid::{}", record.action);
            let category = if is_safety_sensitive(&record.action) {
                PreferenceCategory::SafetyPolicy
            } else {
                PreferenceCategory::Habit
            };
            let value = format!("do_not: {correction}");
            match data.preferences.get(&pref_id).cloned() {
                None => {
                    let draft = VersionDraft {
                        category,
                        value: value.clone(),
                        confidence: 0.9,
                        source: PreferenceSource::UserBehavior,
                        scenario: record.scenario.clone(),
                        timestamp: now,
                        reason: "user_correction".into(),
                        evidence: vec![evidence_from_behavior(&record)],
                    };
                    data.preferences.insert(
                        pref_id.clone(),
                        StoredPreference::new(pref_id.clone(), "avoided action", draft),
                    );
                    data.revision += 1;
                    changes.push(PreferenceChange::Created {
                        id: pref_id,
                        version: 1,
                    });
                }
                Some(mut stored) => {
                    let version = stored.push_version(VersionDraft {
                        category,
                        value: value.clone(),
                        confidence: 0.9,
                        source: PreferenceSource::UserBehavior,
                        scenario: record.scenario.clone(),
                        timestamp: now,
                        reason: "user_correction".into(),
                        evidence: vec![evidence_from_behavior(&record)],
                    });
                    data.preferences.insert(pref_id.clone(), stored);
                    data.revision += 1;
                    changes.push(PreferenceChange::Updated {
                        id: pref_id,
                        version,
                        reason: "user_correction".into(),
                    });
                }
            }

            let habit_id = format!("habit::{}::{}", record.scenario.name, record.action);
            let weakened = data.preferences.get(&habit_id).cloned().and_then(|stored| {
                if stored.current.confidence > MIN_CONFIDENCE {
                    let new_conf = (stored.current.confidence - 0.2).max(0.2);
                    Some((stored, new_conf))
                } else {
                    None
                }
            });
            if let Some((mut stored, new_conf)) = weakened {
                let version = stored.push_version(VersionDraft {
                    category: stored.current.category,
                    value: stored.current.value.clone(),
                    confidence: new_conf,
                    source: PreferenceSource::UserBehavior,
                    scenario: record.scenario.clone(),
                    timestamp: now,
                    reason: "weakened_by_correction".into(),
                    evidence: vec![evidence_from_behavior(&record)],
                });
                data.preferences.insert(habit_id.clone(), stored);
                data.revision += 1;
                changes.push(PreferenceChange::Updated {
                    id: habit_id,
                    version,
                    reason: "weakened_by_correction".into(),
                });
            }
        } else {
            let count_key = format!("{}::{}", record.scenario.name, record.action);
            let count = data.habit_counts.entry(count_key.clone()).or_insert(0);
            *count += 1;
            let habit_id = format!("habit::{count_key}");
            if *count >= HABIT_MIN_COUNT {
                let conf = habit_confidence(*count);
                match data.preferences.get(&habit_id).cloned() {
                    None => {
                        let draft = VersionDraft {
                            category: PreferenceCategory::Habit,
                            value: record.action.clone(),
                            confidence: conf,
                            source: PreferenceSource::UserBehavior,
                            scenario: record.scenario.clone(),
                            timestamp: now,
                            reason: "auto_habit".into(),
                            evidence: vec![evidence_from_behavior(&record)],
                        };
                        data.preferences.insert(
                            habit_id.clone(),
                            StoredPreference::new(habit_id.clone(), "operation habit", draft),
                        );
                        data.revision += 1;
                        changes.push(PreferenceChange::Created {
                            id: habit_id,
                            version: 1,
                        });
                    }
                    Some(mut stored) => {
                        if conf > stored.current.confidence + 0.02 {
                            let version = stored.push_version(VersionDraft {
                                category: PreferenceCategory::Habit,
                                value: record.action.clone(),
                                confidence: conf,
                                source: PreferenceSource::UserBehavior,
                                scenario: record.scenario.clone(),
                                timestamp: now,
                                reason: "auto_habit".into(),
                                evidence: vec![evidence_from_behavior(&record)],
                            });
                            data.preferences.insert(habit_id.clone(), stored);
                            data.revision += 1;
                            changes.push(PreferenceChange::Updated {
                                id: habit_id,
                                version,
                                reason: "auto_habit".into(),
                            });
                        }
                    }
                }
            }
        }

        if !changes.is_empty() {
            self.persist_locked(&data)?;
        }
        Ok(changes)
    }

    /// Extract an explicit preference from a manually configured setting.
    ///
    /// `config::{key}` carries near-certain confidence; a changed value
    /// creates a new version instead of overwriting the previous one.
    pub fn ingest_manual_config(
        &self,
        record: ManualConfigRecord,
    ) -> Result<Vec<PreferenceChange>, PreferenceError> {
        let mut data = self.inner.lock().map_err(PreferenceError::from_poison)?;
        let pref_id = format!("config::{}", record.key);
        let category = infer_category_from_config_key(&record.key);
        let scenario = record.scenario.clone().unwrap_or_default();
        let now = record.timestamp;
        let mut changes = Vec::new();

        match data.preferences.get(&pref_id).cloned() {
            None => {
                let draft = VersionDraft {
                    category,
                    value: record.value.clone(),
                    confidence: MANUAL_CONFIG_CONFIDENCE,
                    source: PreferenceSource::ManualConfig,
                    scenario: scenario.clone(),
                    timestamp: now,
                    reason: "manual_config".into(),
                    evidence: vec![evidence_from_config(&record)],
                };
                data.preferences.insert(
                    pref_id.clone(),
                    StoredPreference::new(pref_id.clone(), "config value", draft),
                );
                data.revision += 1;
                changes.push(PreferenceChange::Created {
                    id: pref_id,
                    version: 1,
                });
            }
            Some(mut stored) => {
                if stored.current.value != record.value {
                    let version = stored.push_version(VersionDraft {
                        category,
                        value: record.value.clone(),
                        confidence: MANUAL_CONFIG_CONFIDENCE,
                        source: PreferenceSource::ManualConfig,
                        scenario: scenario.clone(),
                        timestamp: now,
                        reason: "manual_config_updated".into(),
                        evidence: vec![evidence_from_config(&record)],
                    });
                    data.preferences.insert(pref_id.clone(), stored);
                    data.revision += 1;
                    changes.push(PreferenceChange::Updated {
                        id: pref_id,
                        version,
                        reason: "manual_config_updated".into(),
                    });
                }
            }
        }

        if !changes.is_empty() {
            self.persist_locked(&data)?;
        }
        Ok(changes)
    }
}

fn normalize_task(task: &str, tool: &str) -> String {
    let t = task.trim();
    if t.is_empty() {
        infer_task_from_tool(tool).unwrap_or_else(|| "general".into())
    } else {
        t.to_string()
    }
}

fn best_tool_for_task(
    stats: &std::collections::BTreeMap<String, super::ToolStat>,
    task: &str,
) -> Option<(String, u32, u32)> {
    let prefix = format!("{task}::");
    let mut best: Option<(String, u32, u32)> = None;
    for (key, stat) in stats {
        if let Some(tool) = key.strip_prefix(&prefix) {
            let total = stat.success + stat.fail;
            if total == 0 {
                continue;
            }
            if stat.success as f32 / (total as f32) < 0.6 {
                continue;
            }
            let candidate = (tool.to_string(), stat.success, total);
            match best {
                None => best = Some(candidate),
                Some((_, bs, bt)) => {
                    if stat.success > bs || (stat.success == bs && total > bt) {
                        best = Some(candidate);
                    }
                }
            }
        }
    }
    best
}

fn task_usage_share(
    stats: &std::collections::BTreeMap<String, super::ToolStat>,
    task: &str,
    tool: &str,
) -> f32 {
    let prefix = format!("{task}::");
    let mut total_all = 0u32;
    let mut tool_total = 0u32;
    for (key, stat) in stats {
        if let Some(t) = key.strip_prefix(&prefix) {
            let total = stat.success + stat.fail;
            total_all += total;
            if t == tool {
                tool_total = total;
            }
        }
    }
    if total_all == 0 {
        0.0
    } else {
        tool_total as f32 / total_all as f32
    }
}

fn habit_confidence(count: u32) -> f32 {
    match count {
        0..=1 => 0.0,
        2 => 0.6,
        3 => 0.75,
        4 => 0.85,
        _ => 0.92,
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

fn evidence_from_tool_record(record: &ToolCallRecord) -> Evidence {
    let outcome = if record.ok { "ok" } else { "fail" };
    let detail = match (&record.input, &record.output) {
        (Some(input), Some(output)) => format!(
            "tool call {outcome}: {} input={} output={}",
            record.tool,
            truncate(&input.to_string(), 80),
            truncate(output, 80)
        ),
        _ => format!("tool call {outcome}: {}", record.tool),
    };
    Evidence {
        source: PreferenceSource::ToolResult,
        detail,
        timestamp: record.timestamp,
        weight: if record.ok { 1.0 } else { 0.6 },
    }
}

fn evidence_from_behavior(record: &BehaviorRecord) -> Evidence {
    let detail = match &record.correction {
        Some(c) => format!("user corrected '{}': {}", record.action, truncate(c, 120)),
        None => format!("user performed '{}'", record.action),
    };
    Evidence {
        source: PreferenceSource::UserBehavior,
        detail,
        timestamp: record.timestamp,
        weight: if record.correction.is_some() {
            1.0
        } else {
            0.8
        },
    }
}

fn evidence_from_config(record: &ManualConfigRecord) -> Evidence {
    Evidence {
        source: PreferenceSource::ManualConfig,
        detail: format!("config {}={}", record.key, truncate(&record.value, 120)),
        timestamp: record.timestamp,
        weight: 1.0,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn store() -> PreferenceStore {
        let dir = TempDir::new().unwrap();
        PreferenceStore::new(dir.path().join("preferences.json"))
    }

    fn tool(
        store: &PreferenceStore,
        name: &str,
        task: &str,
        ok: bool,
        ts: i64,
    ) -> Vec<PreferenceChange> {
        let record = ToolCallRecord::new(name, task, ok, ts, ScenarioTag::new("default"));
        store.ingest_tool_result(record).unwrap()
    }

    #[test]
    fn tool_result_creates_preference_after_threshold() {
        let store = store();
        assert!(tool(&store, "read_file", "file_ops", true, 1).is_empty());
        let changes = tool(&store, "read_file", "file_ops", true, 2);
        assert_eq!(changes.len(), 1);
        match &changes[0] {
            PreferenceChange::Created { id, version } => {
                assert_eq!(id, "tool_choice::file_ops");
                assert_eq!(*version, 1);
            }
            other => panic!("expected Created, got {other:?}"),
        }
        let pref = store.current("tool_choice::file_ops").unwrap();
        assert_eq!(pref.value, "read_file");
        assert_eq!(pref.category, PreferenceCategory::ToolChoice);
        assert_eq!(pref.source, PreferenceSource::ToolResult);
        assert!((pref.confidence - 0.85).abs() < 1e-3);
        assert_eq!(store.versions("tool_choice::file_ops").len(), 1);
    }

    #[test]
    fn tool_result_reinforces_existing_preference() {
        let store = store();
        tool(&store, "bash", "file_ops", true, 1);
        tool(&store, "bash", "file_ops", true, 2);
        tool(&store, "bash", "file_ops", true, 3);
        let changes = tool(&store, "bash", "file_ops", true, 4);
        assert_eq!(changes.len(), 1);
        match &changes[0] {
            PreferenceChange::Updated {
                id,
                version,
                reason,
            } => {
                assert_eq!(id, "tool_choice::file_ops");
                assert_eq!(*version, 2);
                assert_eq!(reason, "auto_reinforced");
            }
            other => panic!("expected Updated, got {other:?}"),
        }
        let pref = store.current("tool_choice::file_ops").unwrap();
        assert!((pref.confidence - 0.88).abs() < 1e-3); // confidence_from_counts(4, 0)
    }

    #[test]
    fn tool_result_switches_to_better_tool() {
        let store = store();
        tool(&store, "bash", "file_ops", true, 1);
        tool(&store, "bash", "file_ops", true, 2);
        tool(&store, "read_file", "file_ops", true, 3);
        tool(&store, "read_file", "file_ops", true, 4);
        let changes = tool(&store, "read_file", "file_ops", true, 5);
        assert_eq!(changes.len(), 1);
        match &changes[0] {
            PreferenceChange::Updated {
                id,
                version,
                reason,
            } => {
                assert_eq!(id, "tool_choice::file_ops");
                assert_eq!(*version, 2);
                assert_eq!(reason, "auto_switched");
            }
            other => panic!("expected Updated, got {other:?}"),
        }
        let pref = store.current("tool_choice::file_ops").unwrap();
        assert_eq!(pref.value, "read_file");
        // history keeps the original preference
        let versions = store.versions("tool_choice::file_ops");
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].value, "bash");
        assert_eq!(versions[1].value, "read_file");
    }

    #[test]
    fn tool_failures_downgrade_preference() {
        let store = store();
        tool(&store, "bash", "shell", true, 1);
        tool(&store, "bash", "shell", true, 2);
        let before = store.current("tool_choice::shell").unwrap();
        assert!((before.confidence - 0.85).abs() < 1e-3);

        tool(&store, "bash", "shell", false, 3);
        tool(&store, "bash", "shell", false, 4);
        let changes = tool(&store, "bash", "shell", false, 5);
        assert_eq!(changes.len(), 1);
        match &changes[0] {
            PreferenceChange::Updated {
                id,
                version,
                reason,
            } => {
                assert_eq!(id, "tool_choice::shell");
                assert_eq!(*version, 2);
                assert_eq!(reason, "auto_downgraded");
            }
            other => panic!("expected Updated, got {other:?}"),
        }
        let after = store.current("tool_choice::shell").unwrap();
        assert_eq!(after.value, "bash"); // value kept, confidence lowered
        assert!((after.confidence - 0.70).abs() < 1e-3);
    }

    #[test]
    fn behavior_creates_habit_after_two_occurrences() {
        let store = store();
        let scenario = ScenarioTag::new("dev");
        let b1 = BehaviorRecord {
            action: "run_tests".into(),
            scenario: scenario.clone(),
            timestamp: 1,
            correction: None,
        };
        assert!(store.ingest_user_behavior(b1).unwrap().is_empty());
        let b2 = BehaviorRecord {
            action: "run_tests".into(),
            scenario,
            timestamp: 2,
            correction: None,
        };
        let changes = store.ingest_user_behavior(b2).unwrap();
        assert_eq!(changes.len(), 1);
        match &changes[0] {
            PreferenceChange::Created { id, version } => {
                assert_eq!(id, "habit::dev::run_tests");
                assert_eq!(*version, 1);
            }
            other => panic!("expected Created, got {other:?}"),
        }
        let pref = store.current("habit::dev::run_tests").unwrap();
        assert_eq!(pref.value, "run_tests");
        assert_eq!(pref.category, PreferenceCategory::Habit);
        assert!((pref.confidence - 0.6).abs() < 1e-6);
    }

    #[test]
    fn behavior_correction_creates_safety_preference() {
        let store = store();
        let record = BehaviorRecord {
            action: "delete".into(),
            scenario: ScenarioTag::new("file_ops"),
            timestamp: 1,
            correction: Some("never delete without backup".into()),
        };
        let changes = store.ingest_user_behavior(record).unwrap();
        assert_eq!(changes.len(), 1);
        match &changes[0] {
            PreferenceChange::Created { id, version } => {
                assert_eq!(id, "avoid::delete");
                assert_eq!(*version, 1);
            }
            other => panic!("expected Created, got {other:?}"),
        }
        let pref = store.current("avoid::delete").unwrap();
        assert!(pref.value.contains("never delete without backup"));
        assert_eq!(pref.category, PreferenceCategory::SafetyPolicy);
        assert_eq!(pref.source, PreferenceSource::UserBehavior);
    }

    #[test]
    fn correction_weakens_conflicting_habit() {
        let store = store();
        let scenario = ScenarioTag::new("dev");
        for ts in 1..=2 {
            store
                .ingest_user_behavior(BehaviorRecord {
                    action: "rm_build".into(),
                    scenario: scenario.clone(),
                    timestamp: ts,
                    correction: None,
                })
                .unwrap();
        }
        let habit = store.current("habit::dev::rm_build").unwrap();
        assert!((habit.confidence - 0.6).abs() < 1e-6);

        store
            .ingest_user_behavior(BehaviorRecord {
                action: "rm_build".into(),
                scenario: scenario.clone(),
                timestamp: 3,
                correction: Some("use clean instead".into()),
            })
            .unwrap();
        let habit = store.current("habit::dev::rm_build").unwrap();
        assert!((habit.confidence - 0.4).abs() < 1e-6);
        let avoid = store.current("avoid::rm_build").unwrap();
        assert!(avoid.value.contains("use clean instead"));
    }

    #[test]
    fn manual_config_creates_typed_preference() {
        let store = store();
        let record = ManualConfigRecord {
            key: "tui.language".into(),
            value: "zh-CN".into(),
            scenario: None,
            timestamp: 1,
        };
        let changes = store.ingest_manual_config(record).unwrap();
        assert_eq!(changes.len(), 1);
        let pref = store.current("config::tui.language").unwrap();
        assert_eq!(pref.category, PreferenceCategory::OutputStyle);
        assert_eq!(pref.source, PreferenceSource::ManualConfig);
        assert!((pref.confidence - 0.98).abs() < 1e-6);
    }

    #[test]
    fn manual_config_update_appends_version() {
        let store = store();
        for (ts, value) in [(1, "read-only"), (2, "workspace-write")] {
            store
                .ingest_manual_config(ManualConfigRecord {
                    key: "permission.mode".into(),
                    value: value.into(),
                    scenario: None,
                    timestamp: ts,
                })
                .unwrap();
        }
        let versions = store.versions("config::permission.mode");
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].value, "read-only");
        assert_eq!(versions[1].value, "workspace-write");
        assert_eq!(
            store.current("config::permission.mode").unwrap().category,
            PreferenceCategory::SafetyPolicy
        );

        // unchanged value is a no-op
        let changes = store
            .ingest_manual_config(ManualConfigRecord {
                key: "permission.mode".into(),
                value: "workspace-write".into(),
                scenario: None,
                timestamp: 3,
            })
            .unwrap();
        assert!(changes.is_empty());
        assert_eq!(store.versions("config::permission.mode").len(), 2);
    }

    #[test]
    fn infer_task_from_tool_names() {
        assert_eq!(infer_task_from_tool("read_file").unwrap(), "file_ops");
        assert_eq!(infer_task_from_tool("web_fetch").unwrap(), "web");
        assert_eq!(infer_task_from_tool("bash").unwrap(), "shell");
        assert_eq!(infer_task_from_tool("grep").unwrap(), "search");
        assert_eq!(infer_task_from_tool("memory").unwrap(), "memory");
        assert!(infer_task_from_tool("unknown_xyz").is_none());
    }

    #[test]
    fn empty_task_falls_back_to_tool_inference() {
        let store = store();
        tool(&store, "read_file", "", true, 1);
        let changes = tool(&store, "read_file", "", true, 2);
        match &changes[0] {
            PreferenceChange::Created { id, .. } => assert_eq!(id, "tool_choice::file_ops"),
            other => panic!("expected Created, got {other:?}"),
        }
    }

    #[test]
    fn confidence_is_monotonic_and_capped() {
        assert!((confidence_from_counts(2, 0) - 0.85).abs() < 1e-3);
        assert!(confidence_from_counts(3, 0) > confidence_from_counts(2, 0));
        assert!(confidence_from_counts(10, 0) <= 0.95);
        let high = confidence_from_counts(100, 0);
        assert!(high > 0.93 && high <= 0.95);
        assert!(confidence_from_counts(0, 0).abs() < 1e-6);
        assert!(confidence_from_counts(1, 1) < confidence_from_counts(2, 0));
    }
}
