use crate::error::{AgentError, Result};
use std::env;
use std::path::PathBuf;

pub struct Config {
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: String,
    pub workdir: PathBuf,
    pub timeout_seconds: u64,
}

impl Config {
    pub fn load() -> Result<Self> {
        dotenvy::dotenv().ok();

        let api_key = env::var("ANTHROPIC_API_KEY")
            .map_err(|_| AgentError::MissingEnvVar("ANTHROPIC_API_KEY".into()))?;

        let base_url = env::var("ANTHROPIC_BASE_URL").ok();
        let model =
            env::var("MODEL_ID").unwrap_or_else(|_| "claude-sonnet-4-5-20250929".to_string());

        Ok(Self {
            api_key,
            base_url,
            model,
            workdir: env::current_dir()?,
            timeout_seconds: 300,
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
