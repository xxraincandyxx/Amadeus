use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use crate::client::LLMClient;
use crate::core::id::AgentId;
use crate::core::Workspace;
use crate::error::Result;

use super::agent::Agent;
use super::agent_config::AgentConfig;
use super::RunResult;

#[derive(Debug, Clone)]
pub enum StopCondition {
    FirstSuccess,
    FirstComplete,
    AllComplete,
    Vote {
        voters: Vec<AgentId>,
        threshold: f32,
    },
}

pub struct RaceConfig {
    pub stop_on: StopCondition,
    pub timeout: Duration,
    pub return_all: bool,
}

impl Default for RaceConfig {
    fn default() -> Self {
        Self {
            stop_on: StopCondition::FirstSuccess,
            timeout: Duration::from_secs(300),
            return_all: false,
        }
    }
}

pub struct RaceResult {
    pub winner: Option<AgentId>,
    pub winner_result: Option<RunResult>,
    pub winner_error: Option<String>,
    pub all_successes: HashMap<AgentId, RunResult>,
    pub all_errors: HashMap<AgentId, String>,
    pub ranking: Vec<(AgentId, Duration)>,
    pub cancelled: Vec<AgentId>,
}

impl RaceResult {
    pub fn is_success(&self) -> bool {
        self.winner_result.is_some()
    }
}

pub struct Race<C: LLMClient> {
    agents: Vec<AgentConfig>,
    config: RaceConfig,
    workspace: Arc<RwLock<Workspace>>,
    client: C,
}

impl<C: LLMClient + Clone + 'static> Race<C> {
    pub fn new(workspace: Arc<RwLock<Workspace>>, client: C) -> Self {
        Self {
            agents: Vec::new(),
            config: RaceConfig::default(),
            workspace,
            client,
        }
    }

    pub fn with_agent(mut self, config: AgentConfig) -> Self {
        self.agents.push(config);
        self
    }

    pub fn stop_on(mut self, condition: StopCondition) -> Self {
        self.config.stop_on = condition;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    pub fn return_all(mut self, return_all: bool) -> Self {
        self.config.return_all = return_all;
        self
    }

    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    pub async fn run(&mut self, task: &str) -> Result<RaceResult> {
        let mut all_successes: HashMap<AgentId, RunResult> = HashMap::new();
        let mut all_errors: HashMap<AgentId, String> = HashMap::new();
        let mut ranking: Vec<(AgentId, Duration)> = Vec::new();
        let mut winner: Option<AgentId> = None;
        let mut winner_result: Option<RunResult> = None;
        let mut winner_error: Option<String> = None;

        for agent_config in &self.agents {
            if winner.is_some() && !self.config.return_all {
                break;
            }

            let agent_id = agent_config.id.unwrap_or_else(AgentId::new);
            let start = Instant::now();

            let mut agent = Agent::new(
                self.client.clone(),
                agent_config.clone(),
                self.workspace.clone(),
            );

            let result: Result<RunResult> =
                match tokio::time::timeout(self.config.timeout, agent.run(task)).await {
                    Ok(inner) => inner,
                    Err(_) => Err(crate::error::AgentError::Timeout(
                        self.config.timeout.as_secs(),
                    )),
                };

            let duration = start.elapsed();

            match &result {
                Ok(run_result) => {
                    ranking.push((agent_id, duration));
                    all_successes.insert(agent_id, run_result.clone());
                    match &self.config.stop_on {
                        StopCondition::FirstSuccess => {
                            if winner.is_none() {
                                winner = Some(agent_id);
                                winner_result = Some(run_result.clone());
                                if !self.config.return_all {
                                    break;
                                }
                            }
                        }
                        StopCondition::FirstComplete => {
                            if winner.is_none() {
                                winner = Some(agent_id);
                                winner_result = Some(run_result.clone());
                                if !self.config.return_all {
                                    break;
                                }
                            }
                        }
                        StopCondition::AllComplete | StopCondition::Vote { .. } => {}
                    }
                }
                Err(e) => {
                    ranking.push((agent_id, duration));
                    all_errors.insert(agent_id, e.to_string());
                    if matches!(self.config.stop_on, StopCondition::FirstComplete)
                        && winner.is_none()
                    {
                        winner = Some(agent_id);
                        winner_error = Some(e.to_string());
                        if !self.config.return_all {
                            break;
                        }
                    }
                }
            }
        }

        ranking.sort_by_key(|(_, d)| *d);

        let cancelled: Vec<AgentId> = if winner.is_some() && !self.config.return_all {
            self.agents
                .iter()
                .skip(ranking.len())
                .filter_map(|c| c.id)
                .collect()
        } else {
            Vec::new()
        };

        Ok(RaceResult {
            winner,
            winner_result,
            winner_error,
            all_successes,
            all_errors,
            ranking,
            cancelled,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_race_config_default() {
        let config = RaceConfig::default();
        assert!(matches!(config.stop_on, StopCondition::FirstSuccess));
        assert_eq!(config.timeout, Duration::from_secs(300));
        assert!(!config.return_all);
    }

    #[test]
    fn test_race_result() {
        let result = RaceResult {
            winner: None,
            winner_result: None,
            winner_error: None,
            all_successes: HashMap::new(),
            all_errors: HashMap::new(),
            ranking: Vec::new(),
            cancelled: Vec::new(),
        };
        assert!(!result.is_success());
    }
}
