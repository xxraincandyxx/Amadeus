//! # Error Types
//!
//! Custom error handling for the Claude agent using `thiserror`.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("API error: {0}")]
    Api(String),

    #[error("API request failed: {0}")]
    ApiRequest(#[from] reqwest::Error),

    #[error("Command execution failed: {0}")]
    Command(String),

    #[error("Command timed out after {0}s")]
    Timeout(u64),

    #[error("Tool '{0}' not found")]
    ToolNotFound(String),

    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Environment variable '{0}' not set")]
    MissingEnvVar(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Invalid provider: {0}")]
    InvalidProvider(String),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Stream ended unexpectedly")]
    StreamEndedUnexpectedly,

    #[error("Path escapes workspace: {0}")]
    PathEscape(PathBuf),

    #[error("Tool input validation failed for '{tool}': {reason}")]
    ToolInput { tool: String, reason: String },

    #[error("Tool execution failed: {0}")]
    ToolExecution(String),

    #[error("Command blocked: {0}")]
    CommandBlocked(String),

    #[error("Text not found in {path}: {snippet}")]
    TextNotFound { path: String, snippet: String },

    #[error("Lock error: {0}")]
    Lock(String),

    #[error(
        "File '{path}' has been modified since it was last read.\n\
             Last modification: {modified_at}\n\
             Last read: {read_at}\n\
             Please re-read the file before modifying it."
    )]
    FileModified {
        path: String,
        read_at: String,
        modified_at: String,
    },

    #[error("Task join error: {0}")]
    JoinError(String),
}

impl AgentError {
    pub fn is_retryable(&self) -> bool {
        match self {
            AgentError::ApiRequest(_) => true,
            AgentError::Timeout(_) => true,
            AgentError::Lock(_) => true,
            AgentError::StreamError(_) => true,
            AgentError::Api(msg) => {
                let msg_lower = msg.to_lowercase();
                msg_lower.contains("rate limit")
                    || msg_lower.contains("overload")
                    || msg_lower.contains("timeout")
                    || msg_lower.contains("connection")
                    || msg_lower.contains("503")
                    || msg_lower.contains("502")
                    || msg_lower.contains("429")
            }
            AgentError::InvalidResponse(msg) => {
                msg.contains("429") || msg.contains("503") || msg.contains("502")
            }
            _ => false,
        }
    }
}

impl From<tokio::task::JoinError> for AgentError {
    fn from(e: tokio::task::JoinError) -> Self {
        AgentError::JoinError(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AgentError>;
