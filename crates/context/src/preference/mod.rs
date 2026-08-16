// @amadeus-header
// summary: Versioned preference memory store with automatic extraction, scenario resolution, and JSON persistence.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::preference
// - type: crate::preference::Preference
// - type: crate::preference::PreferenceVersion
// - type: crate::preference::Evidence
// - type: crate::preference::PreferenceCategory
// - type: crate::preference::PreferenceSource
// - type: crate::preference::PreferenceInput
// - type: crate::preference::PreferenceStore
// - type: crate::preference::PreferenceError
// - type: crate::preference::ScenarioCandidate
// - fn: crate::preference::now_ms
// - artifact: preferences.json
// uses:
// - module: crate::memory::MemoryEntry
// - module: crate::preference::extract
// - module: crate::preference::scenario
// - format: serde JSON persistence
// invariants:
// - Preference identity is stable: category + name yield a fixed id.
// - Every value change appends a version; history is never rewritten.
// - PreferenceStore stays free of network and async dependencies for edge use.
// side_effects:
// - Reads and writes the preferences JSON file on every mutation.
// tests:
// - cmd: cargo test -p context
// @end-amadeus-header

//! Preference memory: automatic extraction, versioning, and cross-scenario
//! resolution for the OS Agent memory module.
//!
//! Preferences are captured from multi-source records (tool execution
//! results, user behavior, manual config), stored with a full version
//! history, and resolved against runtime scenarios so learned preferences
//! can be reused and traced back across scenarios.

pub mod extract;
pub mod scenario;

pub use extract::{BehaviorRecord, ManualConfigRecord, PreferenceChange, ToolCallRecord};
pub use scenario::ScenarioTag;

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::memory::MemoryEntry;
use scenario::scenario_matches;

const MAX_EVIDENCE: usize = 50;

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// The kind of user preference being captured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreferenceCategory {
    ToolChoice,
    OutputStyle,
    SafetyPolicy,
    Workflow,
    Habit,
    General,
}

impl PreferenceCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ToolChoice => "tool_choice",
            Self::OutputStyle => "output_style",
            Self::SafetyPolicy => "safety_policy",
            Self::Workflow => "workflow",
            Self::Habit => "habit",
            Self::General => "general",
        }
    }
}

/// Where a preference was learned from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreferenceSource {
    ToolResult,
    UserBehavior,
    ManualConfig,
}

impl PreferenceSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ToolResult => "tool_result",
            Self::UserBehavior => "user_behavior",
            Self::ManualConfig => "manual_config",
        }
    }
}

/// A single piece of supporting evidence behind a preference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub source: PreferenceSource,
    pub detail: String,
    pub timestamp: i64,
    pub weight: f32,
}

/// The current state of one preference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preference {
    pub id: String,
    pub category: PreferenceCategory,
    pub name: String,
    pub value: String,
    pub confidence: f32,
    pub source: PreferenceSource,
    pub scenario: ScenarioTag,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    pub created_at: i64,
    pub updated_at: i64,
    pub version: u64,
}

/// An immutable snapshot in a preference's version history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferenceVersion {
    pub version: u64,
    pub value: String,
    pub confidence: f32,
    pub source: PreferenceSource,
    pub scenario: ScenarioTag,
    pub timestamp: i64,
    pub reason: String,
}

/// Input for an explicit (manual or programmatic) preference update.
#[derive(Debug, Clone)]
pub struct PreferenceInput {
    pub category: PreferenceCategory,
    pub name: String,
    pub value: String,
    pub confidence: f32,
    pub source: PreferenceSource,
    pub scenario: ScenarioTag,
    pub reason: String,
    pub evidence: Vec<Evidence>,
}

/// A cross-scenario adaptation candidate: a preference learned in another
/// scenario, ranked by scenario similarity.
#[derive(Debug, Clone)]
pub struct ScenarioCandidate {
    pub preference: Preference,
    pub similarity: f32,
    pub source_scenario: ScenarioTag,
}

/// Errors raised by the preference store.
#[derive(Debug, Clone)]
pub enum PreferenceError {
    LockPoisoned(String),
    Persist(String),
    Serialize(String),
    NotFound(String),
    InvalidInput(String),
}

impl PreferenceError {
    pub(crate) fn from_poison(err: impl fmt::Display) -> Self {
        Self::LockPoisoned(err.to_string())
    }
}

impl fmt::Display for PreferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LockPoisoned(msg) => write!(f, "preference store lock poisoned: {msg}"),
            Self::Persist(msg) => write!(f, "preference persistence failed: {msg}"),
            Self::Serialize(msg) => write!(f, "preference serialization failed: {msg}"),
            Self::NotFound(msg) => write!(f, "preference not found: {msg}"),
            Self::InvalidInput(msg) => write!(f, "invalid preference input: {msg}"),
        }
    }
}

impl std::error::Error for PreferenceError {}

/// Internal mutation draft shared by the extractors and the store itself.
#[derive(Debug, Clone)]
pub(crate) struct VersionDraft {
    pub(crate) category: PreferenceCategory,
    pub(crate) value: String,
    pub(crate) confidence: f32,
    pub(crate) source: PreferenceSource,
    pub(crate) scenario: ScenarioTag,
    pub(crate) timestamp: i64,
    pub(crate) reason: String,
    pub(crate) evidence: Vec<Evidence>,
}

/// A preference together with its full version history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredPreference {
    pub(crate) current: Preference,
    #[serde(default)]
    pub(crate) versions: Vec<PreferenceVersion>,
}

impl StoredPreference {
    /// Build a brand-new preference from a draft (version 1).
    pub(crate) fn new(id: String, name: impl Into<String>, draft: VersionDraft) -> Self {
        let entry = PreferenceVersion {
            version: 1,
            value: draft.value.clone(),
            confidence: draft.confidence,
            source: draft.source,
            scenario: draft.scenario.clone(),
            timestamp: draft.timestamp,
            reason: draft.reason,
        };
        let current = Preference {
            id,
            category: draft.category,
            name: name.into(),
            value: draft.value,
            confidence: draft.confidence,
            source: entry.source,
            scenario: entry.scenario.clone(),
            evidence: draft.evidence,
            created_at: entry.timestamp,
            updated_at: entry.timestamp,
            version: 1,
        };
        Self {
            current,
            versions: vec![entry],
        }
    }

    /// Append a new version from a draft, promote it to current, and return
    /// the new version number. History is never rewritten.
    pub(crate) fn push_version(&mut self, draft: VersionDraft) -> u64 {
        let version = self.versions.len() as u64 + 1;
        let entry = PreferenceVersion {
            version,
            value: draft.value.clone(),
            confidence: draft.confidence,
            source: draft.source,
            scenario: draft.scenario.clone(),
            timestamp: draft.timestamp,
            reason: draft.reason,
        };
        self.versions.push(entry.clone());
        self.current.version = version;
        self.current.value = draft.value;
        self.current.confidence = draft.confidence;
        self.current.source = entry.source;
        self.current.scenario = draft.scenario;
        self.current.updated_at = draft.timestamp;
        if !draft.evidence.is_empty() {
            self.current.evidence.extend(draft.evidence);
            let excess = self.current.evidence.len().saturating_sub(MAX_EVIDENCE);
            if excess > 0 {
                self.current.evidence.drain(0..excess);
            }
        }
        version
    }
}

/// Persistent state of the store.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct PreferenceData {
    #[serde(default)]
    pub(crate) revision: u64,
    pub(crate) preferences: BTreeMap<String, StoredPreference>,
    #[serde(default)]
    pub(crate) tool_stats: BTreeMap<String, ToolStat>,
    #[serde(default)]
    pub(crate) habit_counts: BTreeMap<String, u32>,
}

/// Success/failure counters for one (task, tool) pair.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct ToolStat {
    pub(crate) success: u32,
    pub(crate) fail: u32,
    #[serde(default)]
    pub(crate) last_ts: i64,
    #[serde(default)]
    pub(crate) latencies_ms: Vec<u64>,
}

struct Clock(Box<dyn Fn() -> i64 + Send + Sync>);

impl fmt::Debug for Clock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Clock")
    }
}

/// A versioned, scenario-aware preference memory backed by a JSON file.
///
/// All mutations are serialized under an internal mutex and flushed to disk
/// immediately, so the store is safe to share across threads and survives
/// restarts. The store has no network or async dependencies, keeping it
/// suitable for edge (end-device) deployment.
#[derive(Debug)]
pub struct PreferenceStore {
    path: PathBuf,
    inner: Mutex<PreferenceData>,
    clock: Clock,
}

impl PreferenceStore {
    /// Open (or create) a preference store persisted at `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self::with_clock(path, Box::new(now_ms))
    }

    /// Open a store with an injectable clock (used by tests to control time).
    #[doc(hidden)]
    pub fn with_clock(path: impl Into<PathBuf>, clock: Box<dyn Fn() -> i64 + Send + Sync>) -> Self {
        let path = path.into();
        let data = Self::load_from_disk(&path);
        Self {
            path,
            inner: Mutex::new(data),
            clock: Clock(clock),
        }
    }

    fn ts(&self) -> i64 {
        (self.clock.0)()
    }

    fn load_from_disk(path: &Path) -> PreferenceData {
        match std::fs::read_to_string(path) {
            Ok(contents) if !contents.trim().is_empty() => {
                match serde_json::from_str::<PreferenceData>(&contents) {
                    Ok(data) => data,
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to parse preferences file, starting fresh"
                        );
                        PreferenceData::default()
                    }
                }
            }
            _ => PreferenceData::default(),
        }
    }

    fn persist_locked(&self, data: &PreferenceData) -> Result<(), PreferenceError> {
        let json = serde_json::to_string_pretty(data)
            .map_err(|e| PreferenceError::Serialize(e.to_string()))?;
        match self.path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => {
                std::fs::create_dir_all(parent)
                    .map_err(|e| PreferenceError::Persist(e.to_string()))?;
            }
            _ => {}
        }
        std::fs::write(&self.path, json).map_err(|e| PreferenceError::Persist(e.to_string()))
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, PreferenceData>, PreferenceError> {
        self.inner.lock().map_err(PreferenceError::from_poison)
    }

    /// The persistence path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Number of stored preferences.
    pub fn len(&self) -> usize {
        self.lock().map(|d| d.preferences.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The store's mutation counter (bumped on every change).
    pub fn revision(&self) -> u64 {
        self.lock().map(|d| d.revision).unwrap_or(0)
    }

    /// Current state of one preference by id.
    pub fn current(&self, id: &str) -> Option<Preference> {
        self.lock()
            .ok()
            .and_then(|d| d.preferences.get(id).map(|s| s.current.clone()))
    }

    /// All current preferences, sorted by id.
    pub fn all(&self) -> Vec<Preference> {
        self.lock()
            .map(|d| d.preferences.values().map(|s| s.current.clone()).collect())
            .unwrap_or_default()
    }

    /// Full version history of one preference, oldest first.
    pub fn versions(&self, id: &str) -> Vec<PreferenceVersion> {
        self.lock()
            .ok()
            .and_then(|d| d.preferences.get(id).map(|s| s.versions.clone()))
            .unwrap_or_default()
    }

    /// Explicitly set (or update) a preference. Returns the current version
    /// number. An identical value/confidence/scenario is a no-op.
    pub fn set_preference(&self, input: PreferenceInput) -> Result<u64, PreferenceError> {
        if input.name.is_empty() || input.value.is_empty() {
            return Err(PreferenceError::InvalidInput(
                "name and value must not be empty".into(),
            ));
        }
        let mut data = self.lock()?;
        let before = data.revision;
        let id = format!("{}::{}", input.category.as_str(), input.name);
        let now = self.ts();
        let outcome = match data.preferences.get(&id).cloned() {
            None => {
                let draft = VersionDraft {
                    category: input.category,
                    value: input.value,
                    confidence: input.confidence.clamp(0.0, 1.0),
                    source: input.source,
                    scenario: input.scenario,
                    timestamp: now,
                    reason: if input.reason.is_empty() {
                        "manual_set".into()
                    } else {
                        input.reason
                    },
                    evidence: input.evidence,
                };
                data.preferences.insert(
                    id.clone(),
                    StoredPreference::new(id.clone(), input.name, draft),
                );
                data.revision += 1;
                1u64
            }
            Some(mut stored) => {
                let current = &stored.current;
                let value_same = current.value == input.value;
                let conf_same = (current.confidence - input.confidence).abs() < 0.02;
                let scenario_same = current.scenario == input.scenario;
                if value_same && conf_same && scenario_same {
                    current.version
                } else {
                    let version = stored.push_version(VersionDraft {
                        category: input.category,
                        value: input.value,
                        confidence: input.confidence.clamp(0.0, 1.0),
                        source: input.source,
                        scenario: input.scenario,
                        timestamp: now,
                        reason: if input.reason.is_empty() {
                            "manual_set".into()
                        } else {
                            input.reason
                        },
                        evidence: input.evidence,
                    });
                    data.preferences.insert(id.clone(), stored);
                    data.revision += 1;
                    version
                }
            }
        };
        if data.revision != before {
            self.persist_locked(&data)?;
        }
        Ok(outcome)
    }

    /// Roll back a preference to a historical version. Non-destructive: the
    /// restored state is appended as a new version rather than rewriting
    /// history. Returns the new version number.
    pub fn rollback(
        &self,
        id: &str,
        target_version: u64,
        reason: &str,
    ) -> Result<u64, PreferenceError> {
        let mut data = self.lock()?;
        let mut stored = data
            .preferences
            .get(id)
            .cloned()
            .ok_or_else(|| PreferenceError::NotFound(format!("preference '{id}'")))?;
        let target = stored
            .versions
            .iter()
            .find(|v| v.version == target_version)
            .cloned()
            .ok_or_else(|| {
                PreferenceError::NotFound(format!("version {target_version} of '{id}'"))
            })?;
        let version = stored.push_version(VersionDraft {
            category: stored.current.category,
            value: target.value.clone(),
            confidence: target.confidence,
            source: target.source,
            scenario: target.scenario.clone(),
            timestamp: self.ts(),
            reason: format!("rollback_to_v{target_version}: {reason}"),
            evidence: Vec::new(),
        });
        data.preferences.insert(id.to_string(), stored);
        data.revision += 1;
        self.persist_locked(&data)?;
        Ok(version)
    }

    /// Delete a preference and its history.
    pub fn delete(&self, id: &str) -> Result<(), PreferenceError> {
        let mut data = self.lock()?;
        if data.preferences.remove(id).is_none() {
            return Err(PreferenceError::NotFound(format!("preference '{id}'")));
        }
        data.revision += 1;
        self.persist_locked(&data)
    }

    /// Resolve the preferences directly applicable to a runtime scenario:
    /// exact scenario matches plus wildcard preferences, ranked by
    /// confidence then recency.
    pub fn resolve_for_scenario(&self, scenario: &ScenarioTag) -> Vec<Preference> {
        let data = match self.lock() {
            Ok(g) => g,
            Err(e) => {
                tracing::warn!(error = %e, "preference store lock poisoned");
                return Vec::new();
            }
        };
        let mut out: Vec<Preference> = data
            .preferences
            .values()
            .map(|s| s.current.clone())
            .filter(|p| p.scenario.is_any() || scenario_matches(&p.scenario, scenario))
            .collect();
        sort_by_confidence_recency(&mut out);
        out
    }

    /// Resolve the preferences that were in effect at `timestamp_ms` for a
    /// scenario: for each preference, the newest version created at or
    /// before that time is used (跨场景时间回溯).
    pub fn resolve_as_of(&self, scenario: &ScenarioTag, timestamp_ms: i64) -> Vec<Preference> {
        let data = match self.lock() {
            Ok(g) => g,
            Err(e) => {
                tracing::warn!(error = %e, "preference store lock poisoned");
                return Vec::new();
            }
        };
        let mut out = Vec::new();
        for stored in data.preferences.values() {
            let effective = stored
                .versions
                .iter()
                .filter(|v| v.timestamp <= timestamp_ms)
                .max_by_key(|v| v.version);
            let Some(effective) = effective else { continue };
            let mut p = stored.current.clone();
            p.version = effective.version;
            p.value = effective.value.clone();
            p.confidence = effective.confidence;
            p.source = effective.source;
            p.scenario = effective.scenario.clone();
            if p.scenario.is_any() || scenario_matches(&p.scenario, scenario) {
                out.push(p);
            }
        }
        sort_by_confidence_recency(&mut out);
        out
    }

    /// Ranked cross-scenario adaptation candidates: preferences learned in
    /// other scenarios, ordered by scenario similarity (跨场景偏好适配).
    pub fn cross_scenario_candidates(
        &self,
        scenario: &ScenarioTag,
        limit: usize,
    ) -> Vec<ScenarioCandidate> {
        let data = match self.lock() {
            Ok(g) => g,
            Err(e) => {
                tracing::warn!(error = %e, "preference store lock poisoned");
                return Vec::new();
            }
        };
        let mut out: Vec<ScenarioCandidate> = data
            .preferences
            .values()
            .map(|s| s.current.clone())
            .filter(|p| !p.scenario.is_any() && !scenario_matches(&p.scenario, scenario))
            .map(|p| {
                let sim = scenario::scenario_similarity(&p.scenario, scenario);
                ScenarioCandidate {
                    similarity: sim,
                    source_scenario: p.scenario.clone(),
                    preference: p,
                }
            })
            .filter(|c| c.similarity > 0.0)
            .collect();
        out.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out.truncate(limit);
        out
    }

    /// Render resolved preferences (and optionally adaptation candidates)
    /// as memory entries suitable for prompt injection through the
    /// [`crate::memory::MemoryRegistry`].
    pub fn to_memory_entries(
        &self,
        scenario: &ScenarioTag,
        include_candidates: bool,
        max_entries: usize,
    ) -> Vec<MemoryEntry> {
        let mut entries: Vec<MemoryEntry> = self
            .resolve_for_scenario(scenario)
            .into_iter()
            .take(max_entries)
            .map(|p| {
                MemoryEntry::new(
                    p.id,
                    format!(
                        "[{}] {} (confidence {:.2}, version {})",
                        p.category.as_str(),
                        p.value,
                        p.confidence,
                        p.version
                    ),
                    "preference",
                )
            })
            .collect();
        if include_candidates && entries.len() < max_entries {
            let remaining = max_entries - entries.len();
            for cand in self.cross_scenario_candidates(scenario, remaining) {
                entries.push(MemoryEntry::new(
                    cand.preference.id,
                    format!(
                        "[{}] {} (similarity {:.2}, from scenario {})",
                        cand.preference.category.as_str(),
                        cand.preference.value,
                        cand.similarity,
                        cand.source_scenario.name
                    ),
                    "preference_candidate",
                ));
            }
        }
        entries
    }
}

fn sort_by_confidence_recency(prefs: &mut [Preference]) {
    prefs.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};
    use tempfile::TempDir;

    fn store() -> PreferenceStore {
        let dir = TempDir::new().unwrap();
        PreferenceStore::new(dir.path().join("preferences.json"))
    }

    fn input(name: &str, value: &str, category: PreferenceCategory) -> PreferenceInput {
        PreferenceInput {
            category,
            name: name.into(),
            value: value.into(),
            confidence: 0.8,
            source: PreferenceSource::ManualConfig,
            scenario: ScenarioTag::new("test"),
            reason: "test".into(),
            evidence: Vec::new(),
        }
    }

    fn clocked_store(start: i64) -> (PreferenceStore, std::sync::Arc<AtomicI64>) {
        let dir = TempDir::new().unwrap();
        let t = std::sync::Arc::new(AtomicI64::new(start));
        let t2 = t.clone();
        let store = PreferenceStore::with_clock(
            dir.path().join("preferences.json"),
            Box::new(move || t2.load(Ordering::SeqCst)),
        );
        (store, t)
    }

    #[test]
    fn set_creates_version_one() {
        let s = store();
        let v = s
            .set_preference(input(
                "verbosity",
                "concise",
                PreferenceCategory::OutputStyle,
            ))
            .unwrap();
        assert_eq!(v, 1);
        let pref = s.current("output_style::verbosity").unwrap();
        assert_eq!(pref.value, "concise");
        assert_eq!(pref.version, 1);
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn invalid_input_rejected() {
        let s = store();
        let mut bad = input("", "x", PreferenceCategory::General);
        bad.value = "".into();
        assert!(matches!(
            s.set_preference(bad),
            Err(PreferenceError::InvalidInput(_))
        ));
    }

    #[test]
    fn update_appends_version_and_keeps_history() {
        let s = store();
        s.set_preference(input(
            "verbosity",
            "concise",
            PreferenceCategory::OutputStyle,
        ))
        .unwrap();
        let mut i2 = input("verbosity", "detailed", PreferenceCategory::OutputStyle);
        i2.reason = "user asked for more detail".into();
        let v = s.set_preference(i2).unwrap();
        assert_eq!(v, 2);
        let versions = s.versions("output_style::verbosity");
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].value, "concise");
        assert_eq!(versions[1].value, "detailed");
        assert_eq!(versions[1].reason, "user asked for more detail");
        assert_eq!(
            s.current("output_style::verbosity").unwrap().value,
            "detailed"
        );
    }

    #[test]
    fn identical_update_is_noop() {
        let s = store();
        s.set_preference(input(
            "verbosity",
            "concise",
            PreferenceCategory::OutputStyle,
        ))
        .unwrap();
        let v = s
            .set_preference(input(
                "verbosity",
                "concise",
                PreferenceCategory::OutputStyle,
            ))
            .unwrap();
        assert_eq!(v, 1);
        assert_eq!(s.versions("output_style::verbosity").len(), 1);
    }

    #[test]
    fn rollback_restores_historical_value() {
        let s = store();
        s.set_preference(input(
            "verbosity",
            "concise",
            PreferenceCategory::OutputStyle,
        ))
        .unwrap();
        s.set_preference(input(
            "verbosity",
            "detailed",
            PreferenceCategory::OutputStyle,
        ))
        .unwrap();
        let v = s
            .rollback("output_style::verbosity", 1, "restore terse style")
            .unwrap();
        assert_eq!(v, 3);
        let pref = s.current("output_style::verbosity").unwrap();
        assert_eq!(pref.value, "concise");
        let versions = s.versions("output_style::verbosity");
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[1].value, "detailed");
        assert_eq!(versions[2].reason, "rollback_to_v1: restore terse style");
    }

    #[test]
    fn rollback_unknown_target_errors() {
        let s = store();
        s.set_preference(input(
            "verbosity",
            "concise",
            PreferenceCategory::OutputStyle,
        ))
        .unwrap();
        assert!(s.rollback("output_style::verbosity", 99, "x").is_err());
        assert!(s.rollback("missing", 1, "x").is_err());
    }

    #[test]
    fn delete_removes_preference() {
        let s = store();
        s.set_preference(input(
            "verbosity",
            "concise",
            PreferenceCategory::OutputStyle,
        ))
        .unwrap();
        s.delete("output_style::verbosity").unwrap();
        assert!(s.current("output_style::verbosity").is_none());
        assert!(s.delete("output_style::verbosity").is_err());
    }

    #[test]
    fn persistence_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("preferences.json");
        {
            let s = PreferenceStore::new(path.clone());
            s.set_preference(input(
                "verbosity",
                "concise",
                PreferenceCategory::OutputStyle,
            ))
            .unwrap();
            s.set_preference(input(
                "verbosity",
                "detailed",
                PreferenceCategory::OutputStyle,
            ))
            .unwrap();
        }
        {
            let s = PreferenceStore::new(path);
            assert_eq!(s.versions("output_style::verbosity").len(), 2);
            assert_eq!(
                s.current("output_style::verbosity").unwrap().value,
                "detailed"
            );
        }
    }

    #[test]
    fn missing_file_loads_empty() {
        let dir = TempDir::new().unwrap();
        let s = PreferenceStore::new(dir.path().join("nope.json"));
        assert!(s.all().is_empty());
        assert_eq!(s.len(), 0);
        assert_eq!(s.revision(), 0);
    }

    #[test]
    fn resolve_returns_matching_and_wildcard() {
        let s = store();
        let mut wildcard = input("tone", "formal", PreferenceCategory::OutputStyle);
        wildcard.scenario = ScenarioTag::default();
        s.set_preference(wildcard).unwrap();
        let mut specific = input("summary_len", "short", PreferenceCategory::OutputStyle);
        specific.scenario = ScenarioTag::new("report_writing");
        s.set_preference(specific).unwrap();
        let mut other = input("diff_context", "on", PreferenceCategory::OutputStyle);
        other.scenario = ScenarioTag::new("code_review");
        s.set_preference(other).unwrap();

        let resolved = s.resolve_for_scenario(&ScenarioTag::new("report_writing"));
        let ids: Vec<&str> = resolved.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"output_style::tone"));
        assert!(ids.contains(&"output_style::summary_len"));
        assert!(!ids.contains(&"output_style::diff_context"));
    }

    #[test]
    fn resolve_as_of_uses_historical_versions() {
        let (s, t) = clocked_store(10);
        let mut i1 = input("verbosity", "concise", PreferenceCategory::OutputStyle);
        i1.scenario = ScenarioTag::new("writing");
        s.set_preference(i1).unwrap();

        t.store(20, Ordering::SeqCst);
        let mut i2 = input("verbosity", "detailed", PreferenceCategory::OutputStyle);
        i2.scenario = ScenarioTag::new("writing");
        s.set_preference(i2).unwrap();

        let sc = ScenarioTag::new("writing");
        let early = s.resolve_as_of(&sc, 15);
        assert_eq!(early.len(), 1);
        assert_eq!(early[0].value, "concise");
        assert_eq!(early[0].version, 1);

        let late = s.resolve_as_of(&sc, 25);
        assert_eq!(late.len(), 1);
        assert_eq!(late[0].value, "detailed");
        assert_eq!(late[0].version, 2);

        let none = s.resolve_as_of(&sc, 5);
        assert!(none.is_empty());
    }

    #[test]
    fn cross_scenario_candidates_ranked_by_similarity() {
        let s = store();
        let mut p1 = input("style", "concise", PreferenceCategory::OutputStyle);
        p1.scenario = ScenarioTag::new("report_writing").with_attr("lang", "cn");
        s.set_preference(p1).unwrap();
        let mut p2 = input("tool_pref", "grep", PreferenceCategory::ToolChoice);
        p2.scenario = ScenarioTag::new("code_search")
            .with_attr("lang", "cn")
            .with_attr("repo", "rust");
        s.set_preference(p2).unwrap();
        let mut p3 = input("misc", "x", PreferenceCategory::General);
        p3.scenario = ScenarioTag::new("unrelated").with_attr("lang", "en");
        s.set_preference(p3).unwrap();

        let target = ScenarioTag::new("code_review")
            .with_attr("lang", "cn")
            .with_attr("repo", "rust");
        let candidates = s.cross_scenario_candidates(&target, 5);
        assert_eq!(candidates.len(), 2); // unrelated is filtered out
        assert_eq!(candidates[0].preference.id, "tool_choice::tool_pref");
        assert!(
            candidates[0].similarity > candidates[1].similarity,
            "{} should outrank {}",
            candidates[0].similarity,
            candidates[1].similarity
        );
        assert!(candidates[0].similarity > 0.3);
    }

    #[test]
    fn memory_entries_bridge() {
        let s = store();
        let mut wildcard = input("tone", "formal", PreferenceCategory::OutputStyle);
        wildcard.scenario = ScenarioTag::default();
        s.set_preference(wildcard).unwrap();
        let mut other = input("tool_pref", "grep", PreferenceCategory::ToolChoice);
        other.scenario = ScenarioTag::new("code_search").with_attr("lang", "cn");
        s.set_preference(other).unwrap();

        let entries = s.to_memory_entries(
            &ScenarioTag::new("code_review").with_attr("lang", "cn"),
            true,
            10,
        );
        assert!(!entries.is_empty());
        let sources: Vec<&str> = entries.iter().map(|e| e.source.as_str()).collect();
        assert!(sources.contains(&"preference"));
        assert!(sources.contains(&"preference_candidate"));
    }
}
