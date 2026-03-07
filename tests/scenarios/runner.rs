#![allow(dead_code)]

use futures::StreamExt;
use std::sync::Arc;

use amadeus::agent::config::Config;
use amadeus::agent::events::AgentEvent;
use amadeus::agent::loop_agent::Agent;
use amadeus::error::Result;

use super::Scenario;

pub struct ScenarioRunner {
    scenario_name: String,
    config: Arc<Config>,
}

impl ScenarioRunner {
    pub fn new(scenario: Scenario) -> Self {
        let config = Arc::new(Config {
            api_key: "test-key".to_string(),
            model: "test-model".to_string(),
            workdir: std::path::PathBuf::from("/tmp"),
            timeout_seconds: 10,
            ..Config::default()
        });

        Self {
            scenario_name: scenario.name,
            config,
        }
    }

    pub fn with_config(mut self, config: Arc<Config>) -> Self {
        self.config = config;
        self
    }

    pub async fn execute<C: amadeus::client::LLMClient + Clone + 'static>(
        self,
        client: C,
    ) -> Result<Vec<AgentEvent>> {
        let agent = Agent::builder(client, self.config)
            .with_default_tools()
            .build();

        let mut events = Vec::new();
        let stream = agent.run_stream();

        let mut stream = std::pin::pin!(stream);
        while let Some(event) = stream.next().await {
            match event {
                Ok(event) => {
                    events.push(event.clone());

                    if matches!(event, AgentEvent::Done { .. } | AgentEvent::Error { .. }) {
                        break;
                    }
                }
                Err(e) => {
                    events.push(AgentEvent::Error {
                        message: e.to_string(),
                    });
                    break;
                }
            }
        }

        Ok(events)
    }

    pub async fn execute_and_collect_text<C: amadeus::client::LLMClient + Clone + 'static>(
        self,
        client: C,
    ) -> Result<(Vec<AgentEvent>, String)> {
        let events = self.execute(client).await?;

        let text = events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::TextDelta { delta } => Some(delta.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        Ok((events, text))
    }
}
