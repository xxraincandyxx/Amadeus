use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::client::LLMClient;
use crate::core::id::AgentId;
use crate::core::Workspace;
use crate::error::Result;

use super::agent::Agent;
use super::agent_config::AgentConfig;
use super::RunResult;

#[derive(Debug, Clone)]
pub enum Topology {
    FullMesh,
    Star { center: AgentId },
    Ring,
    Custom(HashMap<AgentId, Vec<AgentId>>),
}

pub struct MeshConfig {
    pub topology: Topology,
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            topology: Topology::FullMesh,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeshMessage {
    pub from: AgentId,
    pub to: AgentId,
    pub content: String,
}

pub struct MeshResult {
    pub messages_sent: usize,
    pub results: HashMap<AgentId, Result<RunResult>>,
}

pub struct Mesh<C: LLMClient> {
    agents: HashMap<AgentId, AgentConfig>,
    topology: Topology,
    workspace: Arc<RwLock<Workspace>>,
    client: C,
    message_log: Vec<MeshMessage>,
}

impl<C: LLMClient + Clone + 'static> Mesh<C> {
    pub fn new(workspace: Arc<RwLock<Workspace>>, client: C) -> Self {
        Self {
            agents: HashMap::new(),
            topology: Topology::FullMesh,
            workspace,
            client,
            message_log: Vec::new(),
        }
    }

    pub fn topology(mut self, topology: Topology) -> Self {
        self.topology = topology;
        self
    }

    pub fn add(mut self, id: impl Into<String>, config: AgentConfig) -> Self {
        let config = if let Some(_id) = config.id {
            config
        } else {
            let parsed: Option<AgentId> = id.into().parse().ok();
            let agent_id = parsed.unwrap_or_else(AgentId::new);
            AgentConfig::default()
                .id(agent_id)
                .role(config.role)
        };
        let agent_id = config.id.unwrap_or_else(AgentId::new);
        self.agents.insert(agent_id, config.id(agent_id));
        self
    }

    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    pub fn agent_ids(&self) -> Vec<AgentId> {
        self.agents.keys().copied().collect()
    }

    pub fn get_connections(&self, agent_id: AgentId) -> Vec<AgentId> {
        match &self.topology {
            Topology::FullMesh => self.agents.keys().copied().filter(|id| *id != agent_id).collect(),
            Topology::Star { center } => {
                if agent_id == *center {
                    self.agents.keys().copied().filter(|id| *id != agent_id).collect()
                } else {
                    vec![*center]
                }
            }
            Topology::Ring => {
                let ids: Vec<AgentId> = self.agents.keys().copied().collect();
                if let Some(idx) = ids.iter().position(|id| *id == agent_id) {
                    let next = (idx + 1) % ids.len();
                    vec![ids[next]]
                } else {
                    Vec::new()
                }
            }
            Topology::Custom(adj) => adj.get(&agent_id).cloned().unwrap_or_default(),
        }
    }

    pub async fn send(&mut self, from: AgentId, to: AgentId, msg: &str) -> Result<()> {
        if !self.agents.contains_key(&from) || !self.agents.contains_key(&to) {
            return Err(crate::error::AgentError::Api("Agent not found".to_string()));
        }

        let connections = self.get_connections(from);
        if !connections.contains(&to) {
            return Err(crate::error::AgentError::Api(
                "Agents not connected in topology".to_string(),
            ));
        }

        self.message_log.push(MeshMessage {
            from,
            to,
            content: msg.to_string(),
        });

        let config = self.agents.get(&to).cloned().unwrap_or_default();
        let mut agent = Agent::new(self.client.clone(), config, self.workspace.clone());

        agent.run(&format!("Message from {}: {}", from, msg)).await?;

        Ok(())
    }

    pub async fn broadcast(&mut self, from: AgentId, msg: &str) -> Result<usize> {
        let connections = self.get_connections(from);
        let mut sent = 0;

        for to in connections {
            if self.send(from, to, msg).await.is_ok() {
                sent += 1;
            }
        }

        Ok(sent)
    }

    pub async fn run(&mut self, initial: Option<(AgentId, &str)>) -> Result<MeshResult> {
        let mut results = HashMap::new();

        if let Some((starter_id, prompt)) = initial {
            if let Some(config) = self.agents.get(&starter_id).cloned() {
                let mut agent = Agent::new(self.client.clone(), config, self.workspace.clone());
                let result = agent.run(prompt).await;
                results.insert(starter_id, result);
            }
        }

        Ok(MeshResult {
            messages_sent: self.message_log.len(),
            results,
        })
    }

    pub fn message_log(&self) -> &[MeshMessage] {
        &self.message_log
    }
}
