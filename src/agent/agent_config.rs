use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::core::id::AgentId;

use super::config::{Config, Provider};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: Option<AgentId>,
    pub role: String,
    pub system_prompt: Option<String>,
    pub provider: Provider,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: String,
    pub tools: Option<Vec<String>>,
    pub max_tool_calls: usize,
    pub timeout_seconds: u64,
    pub priority: u8,
    pub restart_policy: RestartPolicy,
    pub workdir: std::path::PathBuf,
    pub max_output_bytes: usize,
    pub blocked_commands: Vec<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            id: None,
            role: "default".to_string(),
            system_prompt: None,
            provider: Provider::Anthropic,
            api_key: String::new(),
            base_url: None,
            model: "claude-sonnet-4-5-20250929".to_string(),
            tools: None,
            max_tool_calls: 100,
            timeout_seconds: 300,
            priority: 0,
            restart_policy: RestartPolicy::Never,
            workdir: std::env::current_dir().unwrap_or_default(),
            max_output_bytes: 50_000,
            blocked_commands: vec!["rm -rf /".to_string()],
        }
    }
}

impl AgentConfig {
    pub fn new(role: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            ..Default::default()
        }
    }

    pub fn from_env() -> crate::error::Result<Self> {
        let config = Config::load()?;
        Ok(Self::from_config(config))
    }

    pub fn from_config(config: Config) -> Self {
        Self {
            id: None,
            role: "default".to_string(),
            system_prompt: None,
            provider: config.provider,
            api_key: config.api_key,
            base_url: config.base_url,
            model: config.model,
            tools: None,
            max_tool_calls: 100,
            timeout_seconds: config.timeout_seconds,
            priority: 0,
            restart_policy: RestartPolicy::Never,
            workdir: config.workdir,
            max_output_bytes: config.max_output_bytes,
            blocked_commands: config.blocked_commands,
        }
    }

    pub fn id(mut self, id: AgentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn role(mut self, role: impl Into<String>) -> Self {
        self.role = role.into();
        self
    }

    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn tools(mut self, tools: Vec<String>) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    pub fn timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    pub fn timeout_duration(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }

    pub fn get_system_prompt(&self) -> String {
        if let Some(ref prompt) = self.system_prompt {
            prompt.clone()
        } else {
            format!(
                "You are a CLI agent at {}.\n\n\
                 Loop: think briefly -> use tools -> report results.\n\n\
                 Rules:\n\
                 - Prefer tools over prose. Act, don't just explain.\n\
                 - Never invent file paths. Use bash ls/find first if unsure.\n\
                 - Make minimal changes. Don't over-engineer.\n\
                 - After finishing, summarize what changed.",
                self.workdir.display()
            )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestartPolicy {
    Never,
    OnFailure,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AgentStatus {
    #[default]
    Idle,
    Thinking,
    ExecutingTool,
    Waiting,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentStats {
    pub total_tool_calls: usize,
    pub successful_tool_calls: usize,
    pub failed_tool_calls: usize,
    pub total_tokens: usize,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct AgentMeta {
    pub id: AgentId,
    pub config: AgentConfig,
    pub status: AgentStatus,
    pub stats: AgentStats,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_builder() {
        let config = AgentConfig::new("test-agent")
            .model("claude-3")
            .priority(10)
            .timeout(60);

        assert_eq!(config.role, "test-agent");
        assert_eq!(config.model, "claude-3");
        assert_eq!(config.priority, 10);
        assert_eq!(config.timeout_seconds, 60);
    }

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.role, "default");
        assert_eq!(config.max_tool_calls, 100);
        assert_eq!(config.restart_policy, RestartPolicy::Never);
    }
}
