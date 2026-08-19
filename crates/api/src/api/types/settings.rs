// @amadeus-header
// summary: Configuration query and update API data types.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - type: crate::api::types::ConfigResponse
// - type: crate::api::types::UpdateConfigRequest
// - type: crate::api::types::UpdateConfigResponse
// uses:
// - protocol: serde serialization
// invariants:
// - Serialized fields retain the configuration HTTP contract.
// side_effects: none
// tests:
// - cmd: cargo test -p api --all-features
// @end-amadeus-header

use serde::{Deserialize, Serialize};

/*
 * ============================================================================
 * CONFIG ENDPOINT TYPES
 * ============================================================================
 */

/// Response for the `/config` endpoint.
///
/// Current agent configuration.
#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    /// Working directory for the agent.
    pub working_dir: String,
    /// LLM model identifier.
    pub model: String,
    /// Maximum tokens for completions.
    pub max_tokens: u32,
    /// Context window size for the model.
    pub context_window_size: u32,
    /// Timeout for tool execution in seconds.
    pub tool_timeout_secs: u64,
    /// Whether approval is required for tools.
    pub require_approval: bool,
    /// Shell profile to use.
    pub shell_profile: Option<String>,
    /// Session log directory.
    pub session_log_dir: Option<String>,
    /// Active prompt profile summary.
    pub prompt: PromptConfigSummary,
    /// Active tool profile summary.
    pub tools: ToolConfigSummary,
}

/// Active prompt configuration summary.
#[derive(Debug, Serialize)]
pub struct PromptConfigSummary {
    /// Active prompt profile name.
    pub active_profile: String,
    /// Number of configured inline sections and files.
    pub section_count: usize,
    /// Whether a custom prompt profile is active.
    pub configured: bool,
}

/// Active tool configuration summary.
#[derive(Debug, Serialize)]
pub struct ToolConfigSummary {
    /// Active tool profile name.
    pub active_profile: String,
    /// Composed tool inventory.
    pub inventory: Vec<ToolInventorySummary>,
}

/// Single model-visible tool inventory record.
#[derive(Debug, Serialize)]
pub struct ToolInventorySummary {
    /// Canonical tool name.
    pub name: String,
    /// Source pack.
    pub pack: String,
    /// Tool source.
    pub source: String,
    /// Required permission mode.
    pub required_permission: String,
    /// Whether user config changed model-facing metadata.
    pub overridden: bool,
}

/// Request to update configuration.
#[derive(Debug, Deserialize)]
pub struct UpdateConfigRequest {
    /// New model to use.
    #[serde(default)]
    pub model: Option<String>,
    /// New max tokens setting.
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// New context window size.
    #[serde(default)]
    pub context_window_size: Option<u32>,
    /// New tool timeout.
    #[serde(default)]
    pub tool_timeout_secs: Option<u64>,
    /// New approval requirement.
    #[serde(default)]
    pub require_approval: Option<bool>,
}

/// Response after updating configuration.
#[derive(Debug, Serialize)]
pub struct UpdateConfigResponse {
    /// Whether the update was successful.
    pub success: bool,
    /// Updated configuration.
    pub config: ConfigResponse,
}
