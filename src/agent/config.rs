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
//! | `USE_STREAMING` | No | `false` | Enable streaming responses |
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
use crate::error::{AgentError, Result};

// Standard library's environment variable module
use std::env;

// PathBuf - an owned, heap-allocated path (like String but for file paths)
// Why PathBuf instead of String?
// - Handles path separators correctly (/ on Unix, \ on Windows)
// - Provides path manipulation methods (join, parent, etc.)
// - More type-safe (can't accidentally put non-path strings in it)
use std::path::PathBuf;

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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug)]
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

    // Whether to use streaming responses
    // bool = true or false
    //
    // true = get responses as they're generated (faster feedback)
    // false = wait for complete response (simpler)
    pub use_streaming: bool,
}

/*
 * ============================================================================
 * CONFIG IMPLEMENTATION
 * ============================================================================
 */

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
        // PARSE STREAMING FLAG
        // ---------------------------------------------------------------------

        // Get USE_STREAMING environment variable
        // Default to false if not set or if parsing fails
        //
        // Breaking down the chain:
        // 1. env::var("USE_STREAMING") -> Result<String, VarError>
        // 2. .ok() -> Option<String>
        // 3. .and_then(|s| s.parse::<bool>().ok()) -> Option<bool>
        // 4. .unwrap_or(false) -> bool
        let use_streaming = env::var("USE_STREAMING")
            // Convert Result to Option (ignore errors)
            .ok()
            // If we have a string, try to parse it as bool
            //
            // `and_then` takes a closure that returns Option
            // If input is Some(value), call closure with value
            // If input is None, return None
            //
            // `s.parse::<bool>()` tries to parse string as bool
            // - "true" -> Ok(true)
            // - "false" -> Ok(false)
            // - anything else -> Err
            //
            // The ::<bool> is a "turbofish" - explicit type parameter
            // Tells parse() what type to parse to
            //
            // .ok() converts the Result to Option
            .and_then(|s| s.parse::<bool>().ok())
            // If None (not set or parse failed), use false as default
            .unwrap_or(false);

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

            use_streaming, // Same as: use_streaming: use_streaming
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
    /// # Returns
    ///
    /// A formatted system prompt string.

    // `pub` - Public
    // `fn` - Function
    // `&self` - Borrows self (this is a method, not a static function)
    // `-> String` - Returns an owned String
    //
    // Note: No `async` - string formatting is synchronous
    pub fn system_prompt(&self) -> String {
        // `format!` is a macro that creates a String
        // Similar to println! but returns String instead of printing
        //
        // {} are placeholders that get replaced with values
        // self.workdir.display() formats the PathBuf for display
        //
        // Why .display()?
        // - PathBuf can contain non-UTF8 characters on some systems
        // - .display() returns a Display wrapper that handles this
        format!(
            // The system prompt template
            // Multi-line string literal (can span multiple lines)
            // \n is a newline character
            "You are a CLI agent at {}. Solve problems using bash commands.\n\n\
             Rules:\n\
             - Prefer tools over prose. Act first, explain briefly after.\n\
             - Read files: cat, grep, find, rg, ls, head, tail\n\
             - Write files: echo '...' > file, sed -i, or cat << 'EOF' > file\n\
             - Subagent: For complex subtasks, spawn a subagent to keep context clean:\n\
               cargo run -- 'explore src/ and summarize the architecture'\n\n\
             When to use subagent:\n\
             - Task requires reading many files (isolate the exploration)\n\
             - Task is independent and self-contained\n\
             - You want to avoid polluting current conversation with intermediate details\n\n\
             The subagent runs in isolation and returns only its final summary.",
            // This argument replaces the first {} in the format string
            // .display() makes the path displayable as text
            self.workdir.display()
        )
    }
}
