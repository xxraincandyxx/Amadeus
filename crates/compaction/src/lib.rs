// @amadeus-header
// summary: Shared compaction model types used across runtime surfaces.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - type: crate::CompactionConfig
// - type: crate::CompressionStatus
// - type: crate::CompactionResult
// uses:
// - protocol: serde serialization
// invariants:
// - Compaction model defaults and status semantics stay stable across frontends.
// side_effects: none
// tests:
// - cmd: cargo test -p compaction
// @end-amadeus-header

//! Shared compaction model types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionConfig {
    pub threshold_percent: u8,
    pub target_percent: u8,
    pub preserve_recent: usize,
    pub use_llm_summary: bool,
    pub max_summary_chars: usize,
    pub min_messages: usize,
    pub max_tool_result_chars: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            threshold_percent: 75,
            target_percent: 30,
            preserve_recent: 6,
            use_llm_summary: true,
            max_summary_chars: 2000,
            min_messages: 10,
            max_tool_result_chars: 5000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionStatus {
    Compressed,
    Inflated,
    EmptySummary,
    Noop,
    TruncatedOnly,
}

#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub original_count: usize,
    pub compacted_count: usize,
    pub original_tokens: usize,
    pub new_tokens: usize,
    pub tokens_saved: usize,
    pub summary: Option<String>,
    pub messages_summarized: usize,
    pub status: CompressionStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compaction_config_defaults_match_runtime_expectations() {
        let config = CompactionConfig::default();
        assert_eq!(config.threshold_percent, 75);
        assert_eq!(config.target_percent, 30);
        assert_eq!(config.preserve_recent, 6);
    }
}
