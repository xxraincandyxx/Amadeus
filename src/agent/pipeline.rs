use std::sync::Arc;
use std::time::Duration;

use futures::Stream;
use tokio::sync::RwLock;

use crate::client::LLMClient;
use crate::core::Workspace;
use crate::error::Result;

use super::agent::Agent;
use super::agent_config::AgentConfig;

#[derive(Debug, Clone)]
pub struct StageConfig {
    pub name: String,
    pub agent: AgentConfig,
    pub timeout: Duration,
    pub retry_count: usize,
    pub retry_delay: Duration,
}

impl StageConfig {
    pub fn new(name: impl Into<String>, agent: AgentConfig) -> Self {
        Self {
            name: name.into(),
            agent,
            timeout: Duration::from_secs(300),
            retry_count: 0,
            retry_delay: Duration::from_secs(1),
        }
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn retry(mut self, count: usize, delay: Duration) -> Self {
        self.retry_count = count;
        self.retry_delay = delay;
        self
    }
}

#[derive(Debug, Clone)]
pub struct StageResult {
    pub stage_name: String,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub attempts: usize,
    pub duration: Duration,
}

#[derive(Debug, Clone)]
pub struct PipelineResult {
    pub stages: Vec<StageResult>,
    pub final_output: Option<String>,
    pub success: bool,
    pub total_duration: Duration,
}

#[derive(Debug, Clone)]
pub enum PipelineEvent {
    StageStart {
        name: String,
    },
    StageComplete {
        name: String,
        result: StageResult,
    },
    StageError {
        name: String,
        error: String,
        attempt: usize,
    },
    PipelineComplete {
        result: PipelineResult,
    },
}

pub struct Pipeline<C: LLMClient> {
    stages: Vec<StageConfig>,
    workspace: Arc<RwLock<Workspace>>,
    client: C,
}

impl<C: LLMClient + Clone + 'static> Pipeline<C> {
    pub fn new(workspace: Arc<RwLock<Workspace>>, client: C) -> Self {
        Self {
            stages: Vec::new(),
            workspace,
            client,
        }
    }

    pub fn stage(mut self, config: StageConfig) -> Self {
        self.stages.push(config);
        self
    }

    pub fn add_stage(&mut self, config: StageConfig) {
        self.stages.push(config);
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub async fn run(&mut self, input: serde_json::Value) -> Result<PipelineResult> {
        let start = std::time::Instant::now();
        let mut stages_results = Vec::new();
        let mut current_input = input;
        let mut success = true;
        let mut final_output = None;

        for stage_config in &self.stages {
            let stage_start = std::time::Instant::now();
            let mut attempts = 0;
            let mut stage_success = false;
            let mut stage_output = None;
            let mut stage_error = None;

            let prompt = if current_input.is_null() {
                stage_config.agent.get_system_prompt()
            } else {
                format!(
                    "{}\n\nInput data: {}",
                    stage_config.agent.get_system_prompt(),
                    serde_json::to_string(&current_input).unwrap_or_default()
                )
            };

            for attempt in 0..=stage_config.retry_count {
                attempts = attempt + 1;

                let mut agent = Agent::new(
                    self.client.clone(),
                    stage_config.agent.clone(),
                    self.workspace.clone(),
                );

                match tokio::time::timeout(stage_config.timeout, agent.run(&prompt)).await {
                    Ok(Ok(result)) => {
                        current_input =
                            serde_json::to_value(&result.text).unwrap_or(serde_json::Value::Null);
                        stage_output = Some(result.text);
                        stage_success = true;
                        break;
                    }
                    Ok(Err(e)) => {
                        if attempt < stage_config.retry_count {
                            tokio::time::sleep(stage_config.retry_delay).await;
                        } else {
                            stage_error = Some(e.to_string());
                        }
                    }
                    Err(_) => {
                        if attempt < stage_config.retry_count {
                            tokio::time::sleep(stage_config.retry_delay).await;
                        } else {
                            stage_error =
                                Some(format!("Timeout after {}s", stage_config.timeout.as_secs()));
                        }
                    }
                }
            }

            let duration = stage_start.elapsed();

            let result = StageResult {
                stage_name: stage_config.name.clone(),
                success: stage_success,
                output: stage_output.clone(),
                error: stage_error,
                attempts,
                duration,
            };

            if stage_success {
                final_output = stage_output;
            } else {
                success = false;
            }

            stages_results.push(result);

            if !success {
                break;
            }
        }

        Ok(PipelineResult {
            stages: stages_results,
            final_output,
            success,
            total_duration: start.elapsed(),
        })
    }

    pub fn run_stream(self, input: serde_json::Value) -> impl Stream<Item = PipelineEvent> + Send {
        async_stream::stream! {
            let start = std::time::Instant::now();
            let mut stages_results = Vec::new();
            let mut current_input = input;
            let mut success = true;
            let mut final_output = None;

            for stage_config in self.stages {
                yield PipelineEvent::StageStart {
                    name: stage_config.name.clone(),
                };

                let stage_start = std::time::Instant::now();
                let mut attempts = 0;
                let mut stage_success = false;
                let mut stage_output = None;
                let mut stage_error = None;

                let prompt = if current_input.is_null() {
                    stage_config.agent.get_system_prompt()
                } else {
                    format!(
                        "{}\n\nInput data: {}",
                        stage_config.agent.get_system_prompt(),
                        serde_json::to_string(&current_input).unwrap_or_default()
                    )
                };

                for attempt in 0..=stage_config.retry_count {
                    attempts = attempt + 1;

                    let mut agent = Agent::new(
                        self.client.clone(),
                        stage_config.agent.clone(),
                        self.workspace.clone(),
                    );

                    match tokio::time::timeout(stage_config.timeout, agent.run(&prompt)).await {
                        Ok(Ok(result)) => {
                            current_input = serde_json::to_value(&result.text).unwrap_or(serde_json::Value::Null);
                            stage_output = Some(result.text);
                            stage_success = true;
                            break;
                        }
                        Ok(Err(e)) => {
                            yield PipelineEvent::StageError {
                                name: stage_config.name.clone(),
                                error: e.to_string(),
                                attempt: attempts,
                            };
                            if attempt < stage_config.retry_count {
                                tokio::time::sleep(stage_config.retry_delay).await;
                            } else {
                                stage_error = Some(e.to_string());
                            }
                        }
                        Err(_) => {
                            yield PipelineEvent::StageError {
                                name: stage_config.name.clone(),
                                error: "Timeout".to_string(),
                                attempt: attempts,
                            };
                            if attempt < stage_config.retry_count {
                                tokio::time::sleep(stage_config.retry_delay).await;
                            } else {
                                stage_error = Some(format!("Timeout after {}s", stage_config.timeout.as_secs()));
                            }
                        }
                    }
                }

                let duration = stage_start.elapsed();

                if stage_success {
                    final_output = stage_output.clone();
                } else {
                    success = false;
                }

                let result = StageResult {
                    stage_name: stage_config.name.clone(),
                    success: stage_success,
                    output: stage_output,
                    error: stage_error,
                    attempts,
                    duration,
                };

                yield PipelineEvent::StageComplete {
                    name: stage_config.name.clone(),
                    result: result.clone(),
                };

                stages_results.push(result);

                if !success {
                    break;
                }
            }

            yield PipelineEvent::PipelineComplete {
                result: PipelineResult {
                    stages: stages_results,
                    final_output,
                    success,
                    total_duration: start.elapsed(),
                },
            };
        }
    }
}
