//! # Error Types
//!
//! Custom error handling for the Claude agent using `thiserror`.
//!
//! ## Overview
//!
//! This module defines:
//! - `AgentError`: An enum of all possible error variants
//! - `Result<T>`: A type alias for `std::result::Result<T, AgentError>`
//!
//! ## Usage
//!
//! ```rust,ignore
//! use crate::error::{AgentError, Result};
//!
//! fn do_something() -> Result<String> {
//!     // The ? operator automatically converts errors
//!     let content = std::fs::read_to_string("file.txt")?;
//!     Ok(content)
//! }
//! ```
//!
//! ## Error Conversion
//!
//! The `#[from]` attribute enables automatic conversion from underlying
//! error types using the `?` operator:
//!
//! - `reqwest::Error` → `AgentError::Api`
//! - `serde_json::Error` → `AgentError::Serde`
//! - `std::io::Error` → `AgentError::Io`

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 *
 * The `use` keyword brings items from other crates/modules into scope.
 * `thiserror::Error` is a derive macro that auto-implements the Error trait.
 */

// Import the Error derive macro from the thiserror crate.
// thiserror is a popular crate that simplifies error type definitions.
// The `#[derive(Error)]` attribute auto-generates std::error::Error impl.
use thiserror::Error;

/*
 * ============================================================================
 * AGENT ERROR ENUM
 * ============================================================================
 *
 * An enum is a type that can be one of several variants.
 * Each variant can hold different types of data.
 *
 * Unlike structs (which have ALL fields), enum variants hold ONLY their
 * declared data - it's an "OR" relationship, not "AND".
 *
 * This enum represents ALL possible errors in our application.
 * Having a single error type makes error handling consistent.
 */

// `#[derive(Debug)]` - Auto-generates debug formatting ( {:?} )
//   This allows printing the enum with println!("{:?}", error)
//
// `#[derive(Error)]` - From the `thiserror` crate
//   Auto-implements `std::error::Error` trait
//   Also processes `#[error("...")]` attributes for Display impl
#[derive(Debug, Error)]
pub enum AgentError {
    // -------------------------------------------------------------------------
    // CONFIGURATION ERRORS
    // -------------------------------------------------------------------------

    // Variant: Config
    // Data: String (owned heap-allocated text)
    //
    // `#[error("...")]` defines how this variant displays as a string.
    // {0} is a placeholder that gets replaced with the first field (the String)
    // Example output: "Configuration error: missing API key"
    #[error("Configuration error: {0}")]
    Config(String),

    // -------------------------------------------------------------------------
    // API ERRORS
    // -------------------------------------------------------------------------

    // Variant: Api
    // Data: reqwest::Error (an error from the HTTP client library)
    //
    // `#[from]` is a SPECIAL attribute that enables automatic conversion.
    // If a function returns `Result<T, reqwest::Error>`, using `?` will
    // automatically wrap it in `AgentError::Api(...)`.
    //
    // Without #[from], you'd need to write:
    //   .map_err(|e| AgentError::Api(e))?
    //
    // With #[from], you can just write:
    //   ?  (and the conversion happens automatically)
    #[error("API request failed: {0}")]
    Api(#[from] reqwest::Error),

    // -------------------------------------------------------------------------
    // COMMAND EXECUTION ERRORS
    // -------------------------------------------------------------------------

    // Variant: Command
    // Used when a shell command fails (non-zero exit, permission denied, etc.)
    // The String holds the error message describing what went wrong.
    #[error("Command execution failed: {0}")]
    Command(String),

    // Variant: Timeout
    // Used when a command takes longer than the allowed time.
    // The u64 holds the timeout duration in seconds.
    //
    // Note: u64 is an unsigned 64-bit integer (0 to 18,446,744,073,709,551,615)
    #[error("Command timed out after {0}s")]
    Timeout(u64),

    // -------------------------------------------------------------------------
    // TOOL ERRORS
    // -------------------------------------------------------------------------

    // Variant: ToolNotFound
    // Used when the agent tries to call a tool that doesn't exist.
    // The String holds the name of the requested (but missing) tool.
    #[error("Tool '{0}' not found")]
    ToolNotFound(String),

    // -------------------------------------------------------------------------
    // DATA PARSING ERRORS
    // -------------------------------------------------------------------------

    // Variant: Serde
    // Used when JSON serialization or deserialization fails.
    // serde_json::Error contains details about what couldn't be parsed.
    //
    // `#[from]` enables automatic conversion from serde_json::Error
    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),

    // Variant: Json
    // Used for JSON parsing errors with custom messages.
    #[error("JSON error: {0}")]
    Json(String),

    // -------------------------------------------------------------------------
    // I/O ERRORS
    // -------------------------------------------------------------------------

    // Variant: Io
    // Used for file system errors (file not found, permission denied, etc.)
    // std::io::Error is Rust's standard I/O error type.
    //
    // `#[from]` enables automatic conversion from std::io::Error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // -------------------------------------------------------------------------
    // ENVIRONMENT ERRORS
    // -------------------------------------------------------------------------

    // Variant: MissingEnvVar
    // Used when a required environment variable isn't set.
    // The String holds the name of the missing variable.
    #[error("Environment variable '{0}' not set")]
    MissingEnvVar(String),

    // -------------------------------------------------------------------------
    // RESPONSE ERRORS
    // -------------------------------------------------------------------------

    // Variant: InvalidResponse
    // Used when the API returns unexpected or malformed data.
    // The String describes what was wrong with the response.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    // Variant: InvalidProvider
    // Used when an unknown provider name is specified.
    // The String holds the invalid provider name that was given.
    #[error("Invalid provider: {0}")]
    InvalidProvider(String),

    // -------------------------------------------------------------------------
    // STREAMING ERRORS
    // -------------------------------------------------------------------------

    // Variant: StreamError
    // Used when streaming response parsing fails.
    // The String describes the streaming error.
    #[error("Stream error: {0}")]
    StreamError(String),

    // -------------------------------------------------------------------------
    // OTHER ERRORS
    // -------------------------------------------------------------------------

    // Variant: Other
    // Used for miscellaneous errors that don't fit other categories.
    #[error("{0}")]
    Other(String),
}

/*
 * ============================================================================
 * RESULT TYPE ALIAS
 * ============================================================================
 *
 * A type alias creates a shorter name for a complex type.
 * This is purely for convenience - no new type is created.
 *
 * Without this alias, function signatures would look like:
 *   fn load_config() -> std::result::Result<Config, AgentError>
 *
 * With the alias, they become:
 *   fn load_config() -> Result<Config>
 *
 * The <T> in `Result<T>` means "Result containing any type T"
 * This is a "generic type alias" - it works with any T.
 */

// Create a type alias `Result<T>` that expands to
// `std::result::Result<T, AgentError>`
//
// Breaking down the parts:
// - `pub`: Makes this alias visible outside the module
// - `type`: Keyword for creating a type alias
// - `Result<T>`: The new name (generic over T)
// - `=`: "equals" - defines what the alias expands to
// - `std::result::Result`: The standard library Result type
// - `<T, AgentError>`: Generic parameters (success type T, error type AgentError)
pub type Result<T> = std::result::Result<T, AgentError>;
