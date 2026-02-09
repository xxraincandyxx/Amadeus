use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("API request failed: {0}")]
    Api(#[from] reqwest::Error),

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
}

pub type Result<T> = std::result::Result<T, AgentError>;
