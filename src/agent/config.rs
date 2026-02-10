use crate::error::{AgentError, Result};
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    Anthropic,
    OpenAI,
}

#[derive(Debug)]
pub struct Config {
    pub provider: Provider,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: String,
    pub workdir: PathBuf,
    pub timeout_seconds: u64,
    pub use_streaming: bool,
}

impl Config {
    pub fn load() -> Result<Self> {
        dotenvy::dotenv().ok();

        let provider = match env::var("PROVIDER").as_deref() {
            Ok("openai") | Ok("OpenAI") => Provider::OpenAI,
            _ => Provider::Anthropic,
        };

        let api_key = match &provider {
            Provider::Anthropic => env::var("ANTHROPIC_API_KEY")
                .map_err(|_| AgentError::MissingEnvVar("ANTHROPIC_API_KEY".into()))?,
            Provider::OpenAI => env::var("OPENAI_API_KEY")
                .map_err(|_| AgentError::MissingEnvVar("OPENAI_API_KEY".into()))?,
        };

        let base_url = match &provider {
            Provider::Anthropic => env::var("ANTHROPIC_BASE_URL").ok(),
            Provider::OpenAI => env::var("OPENAI_BASE_URL").ok(),
        };

        let model = env::var("MODEL_ID").unwrap_or_else(|_| match &provider {
            Provider::Anthropic => "claude-sonnet-4-5-20250929".to_string(),
            Provider::OpenAI => "gpt-4".to_string(),
        });

        let use_streaming = env::var("USE_STREAMING")
            .ok()
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);

        Ok(Self {
            provider,
            api_key,
            base_url,
            model,
            workdir: env::current_dir()?,
            timeout_seconds: 300,
            use_streaming,
        })
    }

    pub fn system_prompt(&self) -> String {
        format!(
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
            self.workdir.display()
        )
    }
}
