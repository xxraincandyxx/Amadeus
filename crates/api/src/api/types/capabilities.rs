// @amadeus-header
// summary: Skills, summarization, compaction, prompt, memory, and tool catalog API data types.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - type: crate::api::types::BuildPromptRequest
// - type: crate::api::types::CompactionConfigResponse
// - type: crate::api::types::MemoryEntriesResponse
// - type: crate::api::types::SkillsResponse
// - type: crate::api::types::SummarizeRequest
// - type: crate::api::types::ToolCatalogResponse
// uses:
// - protocol: serde serialization
// invariants:
// - Serialized fields retain their handler HTTP contracts.
// side_effects: none
// tests:
// - cmd: cargo test -p api --all-features
// @end-amadeus-header

use serde::{Deserialize, Serialize};

/*
 * ============================================================================
 * SKILLS ENDPOINT TYPES
 * ============================================================================
 */

/// Response for the `/skills` endpoint.
///
/// List of available skills/prompt templates.
#[derive(Debug, Serialize)]
pub struct SkillsResponse {
    /// List of available skills.
    pub skills: Vec<SkillSummary>,
}

/// Summary of a skill.
#[derive(Debug, Serialize)]
pub struct SkillSummary {
    /// Name of the skill.
    pub name: String,
    /// Description of what the skill does.
    pub description: String,
}

/// Request body for the `/summarize` endpoint.
#[derive(Debug, Deserialize)]
pub struct SummarizeRequest {
    /// Source text to summarize.
    pub text: String,
    /// Optional custom summarization prompt/system instruction.
    #[serde(default)]
    pub prompt: Option<String>,
    /// Summarization mechanism: `llm` or `extract`.
    #[serde(default)]
    pub mechanism: Option<String>,
    /// Maximum summary length in characters.
    #[serde(default)]
    pub max_summary_chars: Option<usize>,
}

/// Response body for the `/summarize` endpoint.
#[derive(Debug, Serialize)]
pub struct SummarizeResponse {
    /// Generated summary text.
    pub summary: String,
    /// Mechanism used to produce the summary.
    pub mechanism: String,
    /// Prompt used when the `llm` mechanism is selected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_used: Option<String>,
}

// ---------------------------------------------------------------------------
// Compaction API types
// ---------------------------------------------------------------------------

/// Response for `GET /compaction/config`.
#[derive(Debug, Serialize)]
pub struct CompactionConfigResponse {
    pub auto_compact: bool,
    pub threshold_percent: u8,
    pub target_percent: u8,
    pub preserve_recent: usize,
    pub use_llm_summary: bool,
    pub max_summary_chars: usize,
    pub min_messages: usize,
    pub max_tool_result_chars: usize,
    pub active_trigger: String,
}

/// Request for `PATCH /compaction/config`.
#[derive(Debug, Deserialize)]
pub struct CompactionConfigUpdateRequest {
    pub auto_compact: Option<bool>,
    pub threshold_percent: Option<u8>,
    pub target_percent: Option<u8>,
    pub preserve_recent: Option<usize>,
    pub use_llm_summary: Option<bool>,
    pub max_summary_chars: Option<usize>,
    pub min_messages: Option<usize>,
    pub max_tool_result_chars: Option<usize>,
}

/// Response for `GET /compaction/triggers`.
#[derive(Debug, Serialize)]
pub struct CompactionTriggersResponse {
    pub available: Vec<String>,
    pub active: String,
}

// ---------------------------------------------------------------------------
// Prompt API types
// ---------------------------------------------------------------------------

/// Response for `GET /prompts/sections`.
#[derive(Debug, Serialize)]
pub struct PromptSectionsResponse {
    pub sections: Vec<PromptSectionInfo>,
}

/// Summary info for a single prompt section.
#[derive(Debug, Serialize)]
pub struct PromptSectionInfo {
    pub id: String,
    pub title: String,
    pub priority: i32,
    pub dynamic: bool,
    pub content_preview: String,
}

/// Request for `POST /prompts/build`.
#[derive(Debug, Deserialize)]
pub struct BuildPromptRequest {
    pub workdir: Option<String>,
    pub include_sub_agent_tool: Option<bool>,
    pub extra_sections: Option<Vec<PromptSectionInput>>,
}

/// A custom section to inject into the prompt.
#[derive(Debug, Deserialize)]
pub struct PromptSectionInput {
    pub id: String,
    pub content: String,
    pub priority: Option<i32>,
}

/// Response for `POST /prompts/build`.
#[derive(Debug, Serialize)]
pub struct BuildPromptResponse {
    pub prompt: String,
    pub section_count: usize,
}

// ---------------------------------------------------------------------------
// Memory API types
// ---------------------------------------------------------------------------

/// Response for `GET /memory/providers`.
#[derive(Debug, Serialize)]
pub struct MemoryProvidersResponse {
    pub providers: Vec<MemoryProviderInfo>,
}

/// Summary info for a single memory provider.
#[derive(Debug, Serialize)]
pub struct MemoryProviderInfo {
    pub name: String,
    pub writable: bool,
    pub entry_count: usize,
}

/// Response for `GET /memory/entries`.
#[derive(Debug, Serialize)]
pub struct MemoryEntriesResponse {
    pub entries: Vec<MemoryEntryInfo>,
}

/// A single memory entry.
#[derive(Debug, Serialize)]
pub struct MemoryEntryInfo {
    pub key: String,
    pub content: String,
    pub source: String,
}

/// Request for `POST /memory/entries`.
#[derive(Debug, Deserialize)]
pub struct StoreMemoryRequest {
    pub key: String,
    pub content: String,
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String {
    "user".to_string()
}

// ---------------------------------------------------------------------------
// Tool catalog API types
// ---------------------------------------------------------------------------

/// Response for `GET /tools/catalog`.
#[derive(Debug, Serialize)]
pub struct ToolCatalogResponse {
    pub tools: Vec<ToolCatalogEntry>,
}

/// A single tool entry in the catalog.
#[derive(Debug, Serialize)]
pub struct ToolCatalogEntry {
    pub name: String,
    pub description: String,
    pub permission_mode: String,
    pub level: String,
}
