// @amadeus-header
// summary: Agent subsystem code for config.
// layer: agent
// status: active
// feature_flags: none
// provides:
// - module: crate::agent::config
// - type: crate::agent::config::Provider
// - type: crate::agent::config::Config
// uses:
// - module: crate::error
// - protocol: serde serialization
// - artifact: filesystem paths and files
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Reads or writes filesystem state.
// tests:
// - tests/config_test.rs
// @end-amadeus-header

//! # Configuration
//!
//! Load and manage agent configuration from hierarchical settings files and
//! process environment overrides.
//!
//! ## Environment Variables
//!
//! | Variable | Required | Default | Description |
//! |----------|----------|---------|-------------|
//! | `PROVIDER` | No | `anthropic` | LLM provider (`anthropic` or `openai`) |
//! | `ANTHROPIC_API_KEY` | Yes* | - | Anthropic API key |
//! | `ANTHROPIC_BASE_URL` | No | `https://api.anthropic.com` | Custom Anthropic endpoint |
//! | `OPENAI_API_KEY` | Yes* | - | OpenAI API key |
//! | `OPENAI_BASE_URL` | No | `https://api.openai.com` | Custom OpenAI endpoint |
//! | `MODEL_ID` | No | Provider default | Model identifier |
//!
//! *Required based on selected provider.
//!
//! ## Example
//!
//! ```rust,ignore
//! use crate::agent::config::Config;
//!
//! // Load from .amadeus/settings.json, ~/.amadeus/settings.json, and env vars
//! let config = Config::load()?;
//!
//! println!("Provider: {:?}", config.provider);
//! println!("Model: {}", config.model);
//! println!("Working directory: {}", config.workdir.display());
//! ```

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

// Import our crate's error types
// `crate::` means "start from the root of this crate"
// `AgentError` and `Result` were defined in src/error.rs and re-exported in lib.rs
use std::path::Path;

use crate::error::{AgentError, Result};
use crate::permissions::PermissionMode;

// Standard library's environment variable module
use std::env;

// PathBuf - an owned, heap-allocated path (like String but for file paths)
// Why PathBuf instead of String?
// - Handles path separators correctly (/ on Unix, \ on Windows)
// - Provides path manipulation methods (join, parent, etc.)
// - More type-safe (can't accidentally put non-path strings in it)
use std::path::PathBuf;

// Serde for serialization
use serde::{Deserialize, Serialize};

const DEFAULT_MODEL: &str = "claude-sonnet-4-5-20250929";
const DEFAULT_TIMEOUT_SECONDS: u64 = 300;
const DEFAULT_MAX_OUTPUT_BYTES: usize = 50_000;
const DEFAULT_CONTEXT_WINDOW_SIZE: u32 = 200_000;
const DEFAULT_COMPACT_THRESHOLD_PERCENT: u8 = 75;
const DEFAULT_COMPACT_PRESERVE_RECENT: usize = 6;
const DEFAULT_MAX_SUBAGENT_DEPTH: usize = 2;

/*
 * ============================================================================
 * PROVIDER ENUM
 * ============================================================================
 *
 * An enum representing which LLM provider to use.
 */

// `#[derive(Debug)]` - Enables debug formatting with {:?}
// `#[derive(Clone)]` - Allows creating copies with .clone()
// `#[derive(PartialEq)]` - Allows comparison with ==
// `#[derive(Eq)]` - Full equality (required for HashMap keys, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provider {
    // Anthropic Claude models (default)
    // No data - this is a "unit variant" (just a name, no fields)
    Anthropic,

    // OpenAI GPT models
    // Also a unit variant
    OpenAI,
}

/*
 * ============================================================================
 * CONFIG STRUCT
 * ============================================================================
 *
 * Holds all configuration needed to run an agent.
 * Loaded from environment variables when `Config::load()` is called.
 */

// `#[derive(Debug)]` - Enables debug formatting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // -------------------------------------------------------------------------
    // Which LLM provider to use (Anthropic or OpenAI)
    // -------------------------------------------------------------------------

    // The Provider enum we defined above
    // Determines which API client to use
    pub provider: Provider,

    // -------------------------------------------------------------------------
    // Authentication
    // -------------------------------------------------------------------------

    // API key for the selected provider
    // Will be either ANTHROPIC_API_KEY or OPENAI_API_KEY
    //
    // Why String instead of &str?
    // - This struct OWNS the API key data
    // - The data comes from environment variables (not borrowed from elsewhere)
    // - String is heap-allocated and can live as long as needed
    pub api_key: String,

    // -------------------------------------------------------------------------
    // API Endpoint
    // -------------------------------------------------------------------------

    // Optional custom API endpoint
    //
    // `Option<String>` means this can be either:
    // - Some(String) - there's a value
    // - None - no value
    //
    // This is Rust's way of handling nullable values (no null keyword!)
    // Option is an enum: enum Option<T> { Some(T), None }
    //
    // Used for custom endpoints (e.g., proxies, local testing)
    pub base_url: Option<String>,

    // -------------------------------------------------------------------------
    // Model Selection
    // -------------------------------------------------------------------------

    // Model identifier string
    // Examples: "claude-sonnet-4-5-20250929", "gpt-4", "gpt-4o"
    pub model: String,

    // -------------------------------------------------------------------------
    // Execution Settings
    // -------------------------------------------------------------------------

    // Working directory for command execution
    // PathBuf is like String but for file system paths
    //
    // This is where bash commands will run from
    pub workdir: PathBuf,

    // Timeout for shell commands in seconds
    // u64 = unsigned 64-bit integer (0 to ~18 quintillion)
    //
    // Commands that run longer than this are killed
    pub timeout_seconds: u64,

    // -------------------------------------------------------------------------
    // Tool Settings (v2)
    // -------------------------------------------------------------------------

    // Maximum output size in bytes for tool results
    // Prevents large outputs from consuming the context window
    //
    // Default: 50,000 bytes (50KB)
    // Truncates tool output if it exceeds this limit
    pub max_output_bytes: usize,

    // List of blocked shell commands
    // Commands matching these patterns are rejected before execution
    //
    // Default: ["rm -rf /"] (only the most dangerous)
    // Can be extended via BLOCKED_COMMANDS env var (comma-separated)
    pub blocked_commands: Vec<String>,

    // -------------------------------------------------------------------------
    // Logging Settings
    // -------------------------------------------------------------------------

    // Optional directory to save session logs
    // If set, each session's history will be saved as a JSON file here.
    pub session_log_dir: Option<PathBuf>,

    /// Whether to compress session logs using Gzip.
    pub session_log_compress: bool,

    // -------------------------------------------------------------------------
    // Context Window Settings
    // -------------------------------------------------------------------------
    /// Maximum context window size in tokens for the model.
    /// Used to calculate context usage percentage.
    ///
    /// Default: 200,000 (Claude's context window)
    /// Common values: 128000 (GPT-4), 200000 (Claude), 1000000 (Gemini 1.5)
    pub context_window_size: u32,

    // -------------------------------------------------------------------------
    // Compaction Settings
    // -------------------------------------------------------------------------
    /// Enable automatic context compaction when approaching context limits.
    ///
    /// When enabled, the agent will automatically summarize older messages
    /// when the conversation approaches the context window limit.
    ///
    /// Default: true
    pub auto_compact: bool,

    /// Threshold percentage of context window to trigger compaction.
    ///
    /// When the conversation reaches this percentage of the context window,
    /// compaction will be triggered.
    ///
    /// Default: 75
    pub compact_threshold_percent: u8,

    /// Number of recent messages to preserve during compaction.
    ///
    /// These messages will not be summarized and will be kept as-is.
    ///
    /// Default: 6 (typically 3 turns)
    pub compact_preserve_recent: usize,

    // -------------------------------------------------------------------------
    // Sub-agent Settings
    // -------------------------------------------------------------------------
    /// Maximum depth for recursive sub-agent spawning.
    ///
    /// Depth starts at 0 for the root agent. A sub-agent spawned from depth 0
    /// runs at depth 1. When depth >= max_subagent_depth, sub-agent spawning is
    /// disabled.
    ///
    /// Default: 2
    pub max_subagent_depth: usize,
    /// Permission mode for tool execution.
    pub permission_mode: PermissionMode,
}

/*
 * ============================================================================
 * CONFIG IMPLEMENTATION
 * ============================================================================
 */

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: Provider::Anthropic,
            api_key: String::new(),
            base_url: None,
            model: DEFAULT_MODEL.to_string(),
            workdir: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            timeout_seconds: DEFAULT_TIMEOUT_SECONDS,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
            blocked_commands: vec!["rm -rf /".to_string()],
            session_log_dir: None,
            session_log_compress: false,
            context_window_size: DEFAULT_CONTEXT_WINDOW_SIZE,
            auto_compact: true,
            compact_threshold_percent: DEFAULT_COMPACT_THRESHOLD_PERCENT,
            compact_preserve_recent: DEFAULT_COMPACT_PRESERVE_RECENT,
            max_subagent_depth: DEFAULT_MAX_SUBAGENT_DEPTH,
            permission_mode: PermissionMode::WorkspaceWrite,
        }
    }
}

impl Config {
    /// Returns the workspace configuration root.
    pub fn workspace_config_root(&self) -> PathBuf {
        self.workdir.join(".amadeus")
    }

    /// Returns the global configuration root.
    pub fn global_config_root() -> Option<PathBuf> {
        dirs::home_dir().map(|home| home.join(".amadeus"))
    }

    /// Returns the active configuration roots in precedence order.
    pub fn config_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if let Some(global_root) = Self::global_config_root() {
            roots.push(global_root);
        }
        roots.push(self.workspace_config_root());
        roots
    }

    /// Returns the workspace settings path.
    pub fn workspace_settings_path(&self) -> PathBuf {
        self.workspace_config_root().join("settings.json")
    }

    /// Returns the global settings path.
    pub fn global_settings_path() -> Option<PathBuf> {
        Self::global_config_root().map(|root| root.join("settings.json"))
    }

    /// Returns the workspace hook config path.
    pub fn workspace_hooks_path(&self) -> PathBuf {
        self.workspace_config_root().join("hook.json")
    }

    /// Returns the global hook config path.
    pub fn global_hooks_path() -> Option<PathBuf> {
        Self::global_config_root().map(|root| root.join("hook.json"))
    }

    /// Returns the workspace agents directory.
    pub fn agents_dir(&self) -> PathBuf {
        self.workspace_config_root().join("agents")
    }

    /// Returns the workspace skills directory.
    pub fn skills_dir(&self) -> PathBuf {
        self.workspace_config_root().join("skills")
    }

    // -------------------------------------------------------------------------
    // LOAD METHOD
    // -------------------------------------------------------------------------

    /// Load configuration from hierarchical settings and environment overrides.
    ///
    /// This method:
    /// 1. Loads `~/.amadeus/settings.json`
    /// 2. Loads `<workdir>/.amadeus/settings.json`
    /// 3. Applies process environment variables as the highest-priority override
    /// 4. Returns an error if required credentials are still missing
    ///
    /// # Errors
    ///
    /// Returns `AgentError::MissingEnvVar` if a required API key is missing.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Set environment variables
    /// std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-xxx");
    ///
    /// let config = Config::load()?;
    /// assert_eq!(config.provider, Provider::Anthropic);
    /// ```
    // `pub` - Public, accessible from outside the module
    // `fn` - Function keyword
    // `load` - Function name
    // `()` - No parameters (static method / associated function)
    // `-> Result<Self>` - Returns a Result containing Config (Self)
    //
    // Note: No `async` keyword - this is a synchronous function
    // Reading environment variables is fast, no need for async
    pub fn load() -> Result<Self> {
        let workdir = env::current_dir()?;
        Self::load_with_hierarchy(&workdir)
    }

    /// Load configuration for assessment mode without requiring provider credentials.
    pub fn load_for_assessment() -> Result<Self> {
        let workdir = env::current_dir()?;
        Self::load_with_hierarchy_internal(&workdir, false)
    }

    fn validate_credentials(&self) -> Result<()> {
        match self.provider {
            Provider::Anthropic if self.api_key.is_empty() => {
                Err(AgentError::MissingEnvVar("ANTHROPIC_API_KEY".into()))
            }
            Provider::OpenAI if self.api_key.is_empty() => {
                Err(AgentError::MissingEnvVar("OPENAI_API_KEY".into()))
            }
            _ => Ok(()),
        }
    }

    // -------------------------------------------------------------------------
    // SYSTEM PROMPT METHOD
    // -------------------------------------------------------------------------

    /// Generate the system prompt for the agent.
    ///
    /// The system prompt tells the LLM:
    /// - Who it is (a CLI agent)
    /// - Where it is (the working directory)
    /// - How to behave (use tools, spawn subagents)
    ///
    /// If a project context file exists, it will be appended to the system prompt.
    ///
    /// # Returns
    ///
    /// A formatted system prompt string.
    pub fn system_prompt(&self, include_sub_agent_tool: bool) -> String {
        let mut prompt = crate::prompts::render_system_prompt(
            &self.workdir.display().to_string(),
            include_sub_agent_tool,
        );

        // Append project context if available
        if let Some(ctx) = crate::context::ProjectContext::load(&self.workdir) {
            prompt.push_str(&ctx.to_prompt_section());
        }

        prompt
    }

    // -------------------------------------------------------------------------
    // FILE LOADING METHODS (Phase 2)
    // -------------------------------------------------------------------------

    /// Load configuration from a JSON file.
    ///
    /// The file should contain a JSON object with any subset of Config fields.
    /// Fields not present in the file will use their default values.
    ///
    /// # Example file format
    ///
    /// ```json
    /// {
    ///   "model": "claude-sonnet-4-5-20250929",
    ///   "timeout_seconds": 120,
    ///   "max_output_bytes": 100000
    /// }
    /// ```
    pub fn load_from_file(path: &std::path::Path) -> Result<Self> {
        if !path.exists() {
            return Err(AgentError::Config(format!(
                "Config file not found: {}",
                path.display()
            )));
        }

        let content = std::fs::read_to_string(path).map_err(|e| {
            AgentError::Config(format!(
                "Failed to read config file {}: {}",
                path.display(),
                e
            ))
        })?;

        let json: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
            AgentError::Config(format!(
                "Invalid JSON in config file {}: {}",
                path.display(),
                e
            ))
        })?;

        // Start with defaults and overlay the file values
        let mut config = Config::default();

        if let Some(provider) = json.get("provider").and_then(|v| v.as_str()) {
            config.provider = match provider.to_lowercase().as_str() {
                "openai" => Provider::OpenAI,
                _ => Provider::Anthropic,
            };
        }

        if let Some(api_key) = json.get("api_key").and_then(|v| v.as_str()) {
            config.api_key = api_key.to_string();
        }

        if let Some(base_url) = json.get("base_url").and_then(|v| v.as_str()) {
            config.base_url = Some(base_url.to_string());
        }

        if let Some(model) = json.get("model").and_then(|v| v.as_str()) {
            config.model = model.to_string();
        }

        if let Some(workdir) = json.get("workdir").and_then(|v| v.as_str()) {
            config.workdir = PathBuf::from(workdir);
        }

        if let Some(timeout) = json.get("timeout_seconds").and_then(|v| v.as_u64()) {
            config.timeout_seconds = timeout;
        }

        if let Some(max_bytes) = json.get("max_output_bytes").and_then(|v| v.as_u64()) {
            config.max_output_bytes = max_bytes as usize;
        }

        if let Some(blocked) = json.get("blocked_commands").and_then(|v| v.as_array()) {
            config.blocked_commands = blocked
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }

        if let Some(log_dir) = json.get("session_log_dir").and_then(|v| v.as_str()) {
            config.session_log_dir = Some(PathBuf::from(log_dir));
        }

        if let Some(compress) = json.get("session_log_compress").and_then(|v| v.as_bool()) {
            config.session_log_compress = compress;
        }

        if let Some(max_subagent_depth) = json.get("max_subagent_depth").and_then(|v| v.as_u64()) {
            config.max_subagent_depth = max_subagent_depth as usize;
        }

        if let Some(permission_mode) = json.get("permission_mode").and_then(|v| v.as_str()) {
            if let Some(mode) = PermissionMode::parse(permission_mode) {
                config.permission_mode = mode;
            }
        }

        Ok(config)
    }

    /// Load configuration with a hierarchy of sources.
    ///
    /// Priority order (highest to lowest):
    /// 1. Environment variables
    /// 2. Project settings (.amadeus/settings.json in workdir)
    /// 3. User global settings (~/.amadeus/settings.json)
    /// 4. Default values
    ///
    /// # Arguments
    ///
    /// * `workdir` - The working directory to use for project settings
    ///
    /// # Returns
    ///
    /// A merged Config with values from all applicable sources.
    pub fn load_with_hierarchy(workdir: &std::path::Path) -> Result<Self> {
        Self::load_with_hierarchy_internal(workdir, true)
    }

    fn load_with_hierarchy_internal(
        workdir: &std::path::Path,
        validate_credentials: bool,
    ) -> Result<Self> {
        let mut config = Config {
            workdir: workdir.to_path_buf(),
            ..Config::default()
        };

        if let Some(user_config_path) = Self::global_settings_path() {
            if user_config_path.exists() {
                if let Ok(user_config) = Self::load_from_file(&user_config_path) {
                    config = config.merge(user_config);
                }
            }
        }

        let project_config_path = config.workspace_settings_path();
        if project_config_path.exists() {
            if let Ok(project_config) = Self::load_from_file(&project_config_path) {
                config = config.merge(project_config);
            }
        }

        config = config.merge_env();
        config.workdir = workdir.to_path_buf();
        if validate_credentials {
            config.validate_credentials()?;
        }
        Ok(config)
    }

    /// Merge another config into this one.
    ///
    /// Values from `other` that are non-empty/non-None will override
    /// values in `self`.
    pub fn merge(self, other: Self) -> Self {
        Self {
            provider: if other.provider != Provider::Anthropic || other.api_key != self.api_key {
                other.provider
            } else {
                self.provider
            },
            api_key: if !other.api_key.is_empty() {
                other.api_key
            } else {
                self.api_key
            },
            base_url: other.base_url.or(self.base_url),
            model: if other.model != "claude-sonnet-4-5-20250929" || self.model.is_empty() {
                other.model
            } else {
                self.model
            },
            workdir: if other.workdir != Path::new(".")
                && other.workdir != std::env::current_dir().unwrap_or_default()
            {
                other.workdir
            } else {
                self.workdir
            },
            timeout_seconds: if other.timeout_seconds != 300 {
                other.timeout_seconds
            } else {
                self.timeout_seconds
            },
            max_output_bytes: if other.max_output_bytes != 50_000 {
                other.max_output_bytes
            } else {
                self.max_output_bytes
            },
            blocked_commands: if !other.blocked_commands.is_empty()
                && other.blocked_commands != vec!["rm -rf /".to_string()]
            {
                other.blocked_commands
            } else {
                self.blocked_commands
            },
            session_log_dir: other.session_log_dir.or(self.session_log_dir),
            session_log_compress: other.session_log_compress || self.session_log_compress,
            context_window_size: if other.context_window_size != 200_000 {
                other.context_window_size
            } else {
                self.context_window_size
            },
            auto_compact: if !other.auto_compact {
                other.auto_compact
            } else {
                self.auto_compact
            },
            compact_threshold_percent: if other.compact_threshold_percent != 75 {
                other.compact_threshold_percent
            } else {
                self.compact_threshold_percent
            },
            compact_preserve_recent: if other.compact_preserve_recent != 6 {
                other.compact_preserve_recent
            } else {
                self.compact_preserve_recent
            },
            max_subagent_depth: if other.max_subagent_depth != 2 {
                other.max_subagent_depth
            } else {
                self.max_subagent_depth
            },
            permission_mode: if other.permission_mode != PermissionMode::WorkspaceWrite {
                other.permission_mode
            } else {
                self.permission_mode
            },
        }
    }

    /// Merge environment variables into this config.
    ///
    /// Environment variables take highest priority.
    pub fn merge_env(mut self) -> Self {
        // Provider
        if let Ok(provider) = env::var("PROVIDER") {
            self.provider = match provider.to_lowercase().as_str() {
                "openai" => Provider::OpenAI,
                _ => Provider::Anthropic,
            };
        }

        match &self.provider {
            Provider::Anthropic => {
                if let Ok(key) = env::var("ANTHROPIC_API_KEY") {
                    self.api_key = key;
                }
                if let Ok(url) = env::var("ANTHROPIC_BASE_URL") {
                    self.base_url = Some(url);
                }
            }
            Provider::OpenAI => {
                if let Ok(key) = env::var("OPENAI_API_KEY") {
                    self.api_key = key;
                }
                if let Ok(url) = env::var("OPENAI_BASE_URL") {
                    self.base_url = Some(url);
                }
            }
        }

        if let Ok(model) = env::var("MODEL_ID") {
            self.model = model;
        }

        if let Ok(max_bytes) = env::var("MAX_OUTPUT_BYTES") {
            if let Ok(bytes) = max_bytes.parse::<usize>() {
                self.max_output_bytes = bytes;
            }
        }

        if let Ok(blocked) = env::var("BLOCKED_COMMANDS") {
            self.blocked_commands = blocked
                .split(',')
                .map(|cmd| cmd.trim().to_string())
                .filter(|cmd| !cmd.is_empty())
                .collect();
        }

        if let Ok(log_dir) = env::var("SESSION_LOG_DIR") {
            self.session_log_dir = Some(PathBuf::from(log_dir));
        }

        if let Ok(compress) = env::var("SESSION_LOG_COMPRESS") {
            if let Ok(value) = compress.parse::<bool>() {
                self.session_log_compress = value;
            }
        }

        if let Ok(timeout) = env::var("TIMEOUT_SECONDS") {
            if let Ok(seconds) = timeout.parse::<u64>() {
                self.timeout_seconds = seconds;
            }
        }

        if let Ok(auto_compact) = env::var("AUTO_COMPACT") {
            if let Ok(value) = auto_compact.parse::<bool>() {
                self.auto_compact = value;
            }
        }

        if let Ok(threshold) = env::var("COMPACT_THRESHOLD_PERCENT") {
            if let Ok(value) = threshold.parse::<u8>() {
                self.compact_threshold_percent = value;
            }
        }

        if let Ok(preserve) = env::var("COMPACT_PRESERVE_RECENT") {
            if let Ok(value) = preserve.parse::<usize>() {
                self.compact_preserve_recent = value;
            }
        }

        if let Ok(max_depth) = env::var("MAX_SUBAGENT_DEPTH") {
            if let Ok(value) = max_depth.parse::<usize>() {
                self.max_subagent_depth = value;
            }
        }

        if let Ok(permission_mode) = env::var("PERMISSION_MODE") {
            if let Some(mode) = PermissionMode::parse(&permission_mode) {
                self.permission_mode = mode;
            }
        }

        self
    }

    /// Create a CompactionConfig from this Config.
    pub fn to_compaction_config(&self) -> super::compaction::CompactionConfig {
        super::compaction::CompactionConfig {
            threshold_percent: self.compact_threshold_percent,
            target_percent: 40, // Target 40% after compaction
            preserve_recent: self.compact_preserve_recent,
            use_llm_summary: true,
            max_summary_chars: 2000,
            min_messages: 10,
            max_tool_result_chars: 5000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::tempdir;

    fn restore_env(key: &str, value: Option<String>) {
        match value {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
    }

    #[test]
    fn load_with_hierarchy_prefers_workspace_settings() {
        let temp = tempdir().unwrap();
        let workdir = temp.path().join("workspace");
        let workspace_root = workdir.join(".amadeus");
        std::fs::create_dir_all(&workspace_root).unwrap();

        let provider = env::var("PROVIDER").ok();
        let home = env::var("HOME").ok();

        env::remove_var("PROVIDER");

        let fake_home = temp.path().join("home");
        let global_root = fake_home.join(".amadeus");
        std::fs::create_dir_all(&global_root).unwrap();
        std::fs::write(
            global_root.join("settings.json"),
            r#"{"model":"global-model","timeout_seconds":120,"api_key":"global-key"}"#,
        )
        .unwrap();
        std::fs::write(
            workspace_root.join("settings.json"),
            r#"{"model":"workspace-model","timeout_seconds":45,"api_key":"workspace-key"}"#,
        )
        .unwrap();
        env::set_var("HOME", &fake_home);

        let config = Config::load_with_hierarchy(&workdir).unwrap();

        assert_eq!(config.model, "workspace-model");
        assert_eq!(config.timeout_seconds, 45);
        assert_eq!(
            config.workspace_settings_path(),
            workspace_root.join("settings.json")
        );
        assert_eq!(
            config.workspace_hooks_path(),
            workspace_root.join("hook.json")
        );
        assert_eq!(config.agents_dir(), workspace_root.join("agents"));
        assert_eq!(config.skills_dir(), workspace_root.join("skills"));

        restore_env("PROVIDER", provider);
        restore_env("HOME", home);
    }

    #[test]
    fn load_with_hierarchy_ignores_workspace_env_file() {
        let temp = tempdir().unwrap();
        let workdir = temp.path().join("workspace");
        let workspace_root = workdir.join(".amadeus");
        std::fs::create_dir_all(&workspace_root).unwrap();
        std::fs::write(workspace_root.join("env"), "MODEL_ID=env-model\n").unwrap();

        let provider = env::var("PROVIDER").ok();
        let anthropic_key = env::var("ANTHROPIC_API_KEY").ok();
        let model = env::var("MODEL_ID").ok();

        env::remove_var("PROVIDER");
        env::set_var("ANTHROPIC_API_KEY", "env-test-key");
        env::remove_var("MODEL_ID");

        let config = Config::load_with_hierarchy(&workdir).unwrap();

        assert_eq!(config.model, DEFAULT_MODEL);

        restore_env("PROVIDER", provider);
        restore_env("ANTHROPIC_API_KEY", anthropic_key);
        restore_env("MODEL_ID", model);
    }

    #[test]
    fn load_from_file_reads_permission_mode() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("settings.json");
        std::fs::write(&path, r#"{"api_key":"x","permission_mode":"read-only"}"#).unwrap();

        let config = Config::load_from_file(&path).unwrap();

        assert_eq!(config.permission_mode, PermissionMode::ReadOnly);
    }
}
