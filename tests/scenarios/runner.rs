#![allow(dead_code)]

use futures::StreamExt;
use std::sync::Arc;

use amadeus::agent::config::Config;
use amadeus::agent::events::AgentEvent;
use amadeus::agent::loop_agent::Agent;
use amadeus::error::Result;

use super::timeline::EventTimeline;
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
        let timeline = self.execute_timeline(client).await?;
        Ok(timeline.raw_events())
    }

    pub async fn execute_and_collect_text<C: amadeus::client::LLMClient + Clone + 'static>(
        self,
        client: C,
    ) -> Result<(Vec<AgentEvent>, String)> {
        let timeline = self.execute_timeline(client).await?;
        let text = timeline.full_text();
        Ok((timeline.raw_events(), text))
    }

    pub async fn execute_timeline<C: amadeus::client::LLMClient + Clone + 'static>(
        self,
        client: C,
    ) -> Result<EventTimeline> {
        let agent = Agent::builder(client, self.config)
            .with_default_tools()
            .build();

        let history = agent.history();
        let mut timeline = EventTimeline::new();
        let stream = agent.run_stream();

        let mut stream = std::pin::pin!(stream);
        while let Some(event) = stream.next().await {
            match event {
                Ok(event) => {
                    timeline.push(event.clone());

                    if matches!(event, AgentEvent::Done { .. } | AgentEvent::Error { .. }) {
                        break;
                    }
                }
                Err(e) => {
                    timeline.push(AgentEvent::Error {
                        message: e.to_string(),
                    });
                    break;
                }
            }
        }

        let history_guard = history.read().await;
        timeline.set_history_snapshot(history_guard.clone());

        Ok(timeline)
    }
}
