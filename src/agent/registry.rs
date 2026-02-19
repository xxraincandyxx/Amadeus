use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::client::LLMClient;
use crate::core::event::Event;
use crate::core::id::AgentId;
use crate::core::Workspace;
use crate::error::Result;

use super::agent::Agent;
use super::agent_config::{AgentConfig, AgentMeta, AgentStats, AgentStatus};

pub struct AgentRegistry<C: LLMClient> {
    workspace: Arc<RwLock<Workspace>>,
    client: C,
    agents: HashMap<AgentId, AgentMeta>,
    default_config: AgentConfig,
}

impl<C: LLMClient + Clone + 'static> AgentRegistry<C> {
    pub fn new(workspace: Arc<RwLock<Workspace>>, client: C) -> Self {
        Self {
            workspace,
            client,
            agents: HashMap::new(),
            default_config: AgentConfig::default(),
        }
    }

    pub fn with_default_config(mut self, config: AgentConfig) -> Self {
        self.default_config = config;
        self
    }

    pub async fn spawn(&mut self, config: AgentConfig) -> Result<AgentId> {
        let id = config.id.unwrap_or_else(AgentId::new);
        let role = config.role.clone();

        let meta = AgentMeta {
            id,
            config: config.clone(),
            status: AgentStatus::Idle,
            stats: Default::default(),
        };

        {
            let mut ws = self.workspace.write().await;
            ws.append_event(Event::AgentSpawned {
                id,
                role,
                config: serde_json::to_value(&config).unwrap_or(serde_json::Value::Null),
            });
        }

        self.agents.insert(id, meta);
        Ok(id)
    }

    pub async fn spawn_default(&mut self) -> Result<AgentId> {
        self.spawn(self.default_config.clone()).await
    }

    pub fn get(&self, id: AgentId) -> Option<&AgentMeta> {
        self.agents.get(&id)
    }

    pub fn get_mut(&mut self, id: AgentId) -> Option<&mut AgentMeta> {
        self.agents.get_mut(&id)
    }

    pub fn list(&self) -> Vec<&AgentMeta> {
        self.agents.values().collect()
    }

    pub fn list_by_status(&self, status: AgentStatus) -> Vec<&AgentMeta> {
        self.agents
            .values()
            .filter(|m| m.status == status)
            .collect()
    }

    pub async fn terminate(&mut self, id: AgentId, reason: crate::core::event::TerminationReason) -> Result<()> {
        if let Some(_meta) = self.agents.remove(&id) {
            let mut ws = self.workspace.write().await;
            ws.append_event(Event::AgentTerminated { id, reason });
        }
        Ok(())
    }

    pub fn count(&self) -> usize {
        self.agents.len()
    }

    pub fn count_by_status(&self, status: AgentStatus) -> usize {
        self.agents.values().filter(|m| m.status == status).count()
    }

    pub fn create_agent(&self, id: AgentId) -> Option<Agent<C>> {
        let meta = self.agents.get(&id)?;
        Some(Agent::new(
            self.client.clone(),
            meta.config.clone(),
            self.workspace.clone(),
        ))
    }

    pub fn update_status(&mut self, id: AgentId, status: AgentStatus) {
        if let Some(meta) = self.agents.get_mut(&id) {
            meta.status = status;
        }
    }

    pub fn update_stats(&mut self, id: AgentId, stats: AgentStats) {
        if let Some(meta) = self.agents.get_mut(&id) {
            meta.stats = stats;
        }
    }
}
