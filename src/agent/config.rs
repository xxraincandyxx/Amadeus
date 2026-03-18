//! # Configuration
//!
//! Load and manage agent configuration from environment variables.
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
//! // Load from .env file and environment
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
            model: "claude-sonnet-4-5-20250929".to_string(),
            workdir: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            timeout_seconds: 300,
            max_output_bytes: 50_000,
            blocked_commands: vec!["rm -rf /".to_string()],
            session_log_dir: None,
            session_log_compress: false,
            context_window_size: 200_000,
            auto_compact: true,
            compact_threshold_percent: 75,
            compact_preserve_recent: 6,
            max_subagent_depth: 2,
        }
    }
}

impl Config {
    // -------------------------------------------------------------------------
    // LOAD METHOD
    // -------------------------------------------------------------------------

    /// Load configuration from environment variables.
    ///
    /// This method:
    /// 1. Loads .env file (if present) using dotenvy
    /// 2. Reads environment variables
    /// 3. Falls back to defaults for optional values
    /// 4. Returns error if required values are missing
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
        // ---------------------------------------------------------------------
        // LOAD .ENV FILE
        // ---------------------------------------------------------------------

        // dotenvy::dotenv() loads a .env file from the current directory
        // It sets environment variables from the file
        //
        // .ok() converts Result to Option:
        // - Ok(value) -> Some(value)
        // - Err(_) -> None
        //
        // We use .ok() because we don't care if .env doesn't exist
        // (env vars might already be set by the shell)
        //
        // Equivalent to:
        //   match dotenvy::dotenv() {
        //       Ok(_) => {},  // File loaded, ignore result
        //       Err(_) => {}, // File doesn't exist, that's fine
        //   }
        dotenvy::dotenv().ok();

        // ---------------------------------------------------------------------
        // DETERMINE PROVIDER
        // ---------------------------------------------------------------------

        // Read the PROVIDER environment variable
        //
        // `env::var("PROVIDER")` returns Result<String, VarError>
        // - Ok(value) if the variable is set
        // - Err(VarError) if not set
        //
        // `.as_deref()` converts Result<String, E> to Result<&str, E>
        // This lets us compare the string contents without ownership issues
        //
        // `.as_deref()` is like:
        //   match env::var("PROVIDER") {
        //       Ok(s) => Ok(&s[..]),  // Borrow the String as &str
        //       Err(e) => Err(e),
        //   }
        let provider = match env::var("PROVIDER").as_deref() {
            // If PROVIDER is "openai" or "OpenAI", use OpenAI provider
            // The `|` is "or" pattern - matches either value
            Ok("openai") | Ok("OpenAI") => Provider::OpenAI,

            // For any other value (including unset), default to Anthropic
            // `_` is the "wildcard pattern" - matches anything
            _ => Provider::Anthropic,
        };

        // ---------------------------------------------------------------------
        // GET API KEY
        // ---------------------------------------------------------------------

        // Get the appropriate API key based on provider
        //
        // `match &provider` borrows provider (doesn't move it)
        // We need to use provider again later, so we borrow instead of move
        let api_key = match &provider {
            // If using Anthropic, get ANTHROPIC_API_KEY
            Provider::Anthropic => {
                // Try to get the environment variable
                env::var("ANTHROPIC_API_KEY")
                    // If it fails, convert the error to AgentError::MissingEnvVar
                    //
                    // `map_err` transforms the error type:
                    // - Takes a closure: |_| AgentError::MissingEnvVar(...)
                    // - _ ignores the original error (we don't need its details)
                    // - Returns our custom error instead
                    //
                    // The `?` operator then:
                    // - If Ok(value): extract the value and continue
                    // - If Err(e): return Err(e) from this function immediately
                    .map_err(|_| AgentError::MissingEnvVar("ANTHROPIC_API_KEY".into()))?
            }

            // If using OpenAI, get OPENAI_API_KEY
            // Same pattern as above
            Provider::OpenAI => env::var("OPENAI_API_KEY")
                .map_err(|_| AgentError::MissingEnvVar("OPENAI_API_KEY".into()))?,
        };

        // ---------------------------------------------------------------------
        // GET OPTIONAL BASE URL
        // ---------------------------------------------------------------------

        // Get base URL for the selected provider (optional)
        //
        // match &provider again - we're comparing the same value
        // But since we borrowed it before (match &provider), it's still valid
        let base_url = match &provider {
            // For Anthropic, try to get ANTHROPIC_BASE_URL
            //
            // `.ok()` converts Result to Option:
            // - Ok(value) -> Some(value)
            // - Err(_) -> None
            //
            // This is perfect for optional config - if not set, we get None
            Provider::Anthropic => env::var("ANTHROPIC_BASE_URL").ok(),

            // Same for OpenAI
            Provider::OpenAI => env::var("OPENAI_BASE_URL").ok(),
        };

        // ---------------------------------------------------------------------
        // GET MODEL (WITH DEFAULTS)
        // ---------------------------------------------------------------------

        // Get model ID, with provider-specific defaults
        //
        // `unwrap_or_else()` is used to provide a default value
        // It takes a closure (anonymous function) that generates the default
        //
        // Signature: fn unwrap_or_else<E, F>(self, f: F) -> T
        //   where F: FnOnce(E) -> T
        //
        // If the Result is Ok, return the value
        // If Err, call the closure to generate a default
        let model = env::var("MODEL_ID").unwrap_or_else(|_| {
            // The closure receives the error (which we ignore with _)
            // It must return a String (the default model)

            // Match on provider to choose appropriate default
            match &provider {
                // Default Anthropic model
                // `.to_string()` converts &str to String
                Provider::Anthropic => "claude-sonnet-4-5-20250929".to_string(),

                // Default OpenAI model
                Provider::OpenAI => "gpt-4".to_string(),
            }
        });

        // ---------------------------------------------------------------------
        // PARSE MAX OUTPUT SIZE (v2)
        // ---------------------------------------------------------------------

        // Get MAX_OUTPUT_BYTES environment variable
        // Default to 50,000 bytes (50KB) if not set or parsing fails
        //
        // This limits tool output size to prevent context window overflow
        let max_output_bytes = env::var("MAX_OUTPUT_BYTES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(50_000);

        // ---------------------------------------------------------------------
        // PARSE BLOCKED COMMANDS (v2)
        // ---------------------------------------------------------------------

        // Get BLOCKED_COMMANDS environment variable
        // Comma-separated list of blocked command patterns
        //
        // Default: ["rm -rf /"] (most dangerous command)
        // Example: BLOCKED_COMMANDS="rm -rf /,sudo,mkfs"
        let blocked_commands = env::var("BLOCKED_COMMANDS")
            .ok()
            .map(|s| {
                s.split(',')
                    .map(|cmd| cmd.trim().to_string())
                    .filter(|cmd| !cmd.is_empty())
                    .collect()
            })
            .unwrap_or_else(|| vec!["rm -rf /".to_string()]);

        // ---------------------------------------------------------------------
        // PARSE SESSION LOG DIR
        // ---------------------------------------------------------------------

        let session_log_dir = env::var("SESSION_LOG_DIR").ok().map(PathBuf::from);
        let session_log_compress = env::var("SESSION_LOG_COMPRESS")
            .ok()
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);

        // ---------------------------------------------------------------------
        // PARSE CONTEXT WINDOW SIZE
        // ---------------------------------------------------------------------

        // Get CONTEXT_WINDOW_SIZE environment variable
        // Default to 200,000 tokens (Claude's context window)
        // Common values: 128000 (GPT-4), 200000 (Claude), 1000000 (Gemini 1.5)
        let context_window_size = env::var("CONTEXT_WINDOW_SIZE")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(200_000);

        // ---------------------------------------------------------------------
        // PARSE COMPACTION SETTINGS
        // ---------------------------------------------------------------------

        // Enable/disable automatic compaction
        let auto_compact = env::var("AUTO_COMPACT")
            .ok()
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(true);

        // Threshold percentage for compaction trigger
        let compact_threshold_percent = env::var("COMPACT_THRESHOLD_PERCENT")
            .ok()
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(75);

        // Number of recent messages to preserve
        let compact_preserve_recent = env::var("COMPACT_PRESERVE_RECENT")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(6);

        // ---------------------------------------------------------------------
        // PARSE SUB-AGENT DEPTH
        // ---------------------------------------------------------------------

        let max_subagent_depth = env::var("MAX_SUBAGENT_DEPTH")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(2);

        // ---------------------------------------------------------------------
        // BUILD AND RETURN CONFIG
        // ---------------------------------------------------------------------

        // Create a new Config instance
        // This is a "struct expression" - initializes all fields
        Ok(Self {
            provider, // Field shorthand: provider: provider
            api_key,  // Same as: api_key: api_key
            base_url, // Same as: base_url: base_url
            model,    // Same as: model: model

            // Get current working directory
            //
            // `env::current_dir()` returns Result<PathBuf, io::Error>
            // The `?` operator:
            // - If Ok(path): extract the PathBuf and continue
            // - If Err(e): convert to AgentError::Io and return early
            //
            // Note: AgentError has #[from] for io::Error, so conversion
            // from io::Error to AgentError::Io is automatic
            workdir: env::current_dir()?,

            // Hardcoded timeout of 300 seconds (5 minutes)
            // Could be made configurable via env var if needed
            timeout_seconds: 300,

            // v2: Tool settings
            max_output_bytes,
            blocked_commands,

            session_log_dir,
            session_log_compress,

            // Context window management
            context_window_size,

            // Compaction settings
            auto_compact,
            compact_threshold_percent,
            compact_preserve_recent,

            // Sub-agent settings
            max_subagent_depth,
        })
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
    pub fn system_prompt(&self, include_sub_agnet_tool: bool) -> String {
        let mut prompt = crate::prompts::render_system_prompt(
            &self.workdir.display().to_string(),
            include_sub_agnet_tool,
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
        // Start with defaults
        let mut config = Config {
            workdir: workdir.to_path_buf(),
            ..Config::default()
        };

        // 1. Load user global settings (~/.amadeus/settings.json)
        if let Some(home_dir) = dirs::home_dir() {
            let user_config_path = home_dir.join(".amadeus/settings.json");
            if user_config_path.exists() {
                if let Ok(user_config) = Self::load_from_file(&user_config_path) {
                    config = config.merge(user_config);
                }
            }
        }

        // 2. Load project settings (<workdir>/.amadeus/settings.json)
        let project_config_path = workdir.join(".amadeus/settings.json");
        if project_config_path.exists() {
            if let Ok(project_config) = Self::load_from_file(&project_config_path) {
                config = config.merge(project_config);
            }
        }

        // 3. Environment variables (highest priority)
        config = config.merge_env();

        // Ensure workdir is set correctly
        config.workdir = workdir.to_path_buf();

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
        }
    }

    /// Merge environment variables into this config.
    ///
    /// Environment variables take highest priority.
    pub fn merge_env(mut self) -> Self {
        // Load .env file if present
        dotenvy::dotenv().ok();

        // Provider
        if let Ok(provider) = env::var("PROVIDER") {
            self.provider = match provider.to_lowercase().as_str() {
                "openai" => Provider::OpenAI,
                _ => Provider::Anthropic,
            };
        }

        // API keys
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

        // Model
        if let Ok(model) = env::var("MODEL_ID") {
            self.model = model;
        }

        // Tool settings
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

        // Logging settings
        if let Ok(log_dir) = env::var("SESSION_LOG_DIR") {
            self.session_log_dir = Some(PathBuf::from(log_dir));
        }

        if let Ok(compress) = env::var("SESSION_LOG_COMPRESS") {
            if let Ok(b) = compress.parse::<bool>() {
                self.session_log_compress = b;
            }
        }

        // Timeout
        if let Ok(timeout) = env::var("TIMEOUT_SECONDS") {
            if let Ok(secs) = timeout.parse::<u64>() {
                self.timeout_seconds = secs;
            }
        }

        // Compaction settings
        if let Ok(auto_compact) = env::var("AUTO_COMPACT") {
            if let Ok(b) = auto_compact.parse::<bool>() {
                self.auto_compact = b;
            }
        }

        if let Ok(threshold) = env::var("COMPACT_THRESHOLD_PERCENT") {
            if let Ok(p) = threshold.parse::<u8>() {
                self.compact_threshold_percent = p;
            }
        }

        if let Ok(preserve) = env::var("COMPACT_PRESERVE_RECENT") {
            if let Ok(n) = preserve.parse::<usize>() {
                self.compact_preserve_recent = n;
            }
        }

        if let Ok(max_depth) = env::var("MAX_SUBAGENT_DEPTH") {
            if let Ok(n) = max_depth.parse::<usize>() {
                self.max_subagent_depth = n;
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
