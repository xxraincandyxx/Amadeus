use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::RwLock;

use crate::agent::config::Config;
use crate::agent::loop_agent::Agent;
use crate::client::LLMClient;
use crate::hooks::HookRegistry;
use crate::policy::Policy;
use crate::tools::registry::ToolRegistry;
use crate::tools::schema::sub_agnet_tool;
use crate::tools::tool_trait::Tool;
use crate::Result;

const SUB_AGNET_MAX_TURNS: usize = 30;

#[derive(Debug, Deserialize)]
struct SubAgnetInput {
    prompt: String,
    #[allow(dead_code)]
    description: Option<String>,
}

pub struct SubAgnetTool<C: LLMClient> {
    client: C,
    config: Arc<Config>,
    hooks: HookRegistry,
    policy: Arc<RwLock<Policy>>,
    child_tools: ToolRegistry,
}

impl<C: LLMClient + Clone + 'static> SubAgnetTool<C> {
    pub fn new(
        client: C,
        config: Arc<Config>,
        hooks: HookRegistry,
        policy: Arc<RwLock<Policy>>,
    ) -> Self {
        let child_tools = ToolRegistry::with_sub_agnet_child_defaults(&config);

        Self {
            client,
            config,
            hooks,
            policy,
            child_tools,
        }
    }
}

#[async_trait]
impl<C: LLMClient + Clone + 'static> Tool for SubAgnetTool<C> {
    fn name(&self) -> &'static str {
        "sub_agnet"
    }

    fn schema(&self) -> &'static Value {
        sub_agnet_tool()
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let parsed: SubAgnetInput =
            serde_json::from_value(input).map_err(|e| crate::error::AgentError::ToolInput {
                tool: self.name().to_string(),
                reason: e.to_string(),
            })?;

        let policy = self.policy.read().await.clone();
        let child = Agent::builder(self.client.clone(), Arc::clone(&self.config))
            .with_tools(self.child_tools.clone())
            .with_hooks(self.hooks.clone())
            .with_policy(policy)
            .build();

        let result = child
            .run_with_turn_limit(&parsed.prompt, SUB_AGNET_MAX_TURNS)
            .await?;

        if result.text.is_empty() {
            Ok("(no summary)".to_string())
        } else {
            Ok(result.text)
        }
    }
}
