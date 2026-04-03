#![allow(dead_code)]

use futures::StreamExt;
use std::collections::VecDeque;
use std::sync::Arc;

use amadeus::agent::config::Config;
use amadeus::agent::events::{AgentEvent, ApprovalDecision};
use amadeus::agent::loop_agent::{create_approval_channels, Agent};
use amadeus::agent::messages::Message;
use amadeus::error::Result;

use super::timeline::EventTimeline;
use super::{builder::ApprovalScript, Scenario};

pub struct ScenarioRunner {
    scenario_name: String,
    initial_user_prompt: Option<String>,
    approvals: VecDeque<ApprovalScript>,
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
            initial_user_prompt: scenario.initial_user_prompt,
            approvals: scenario.approvals,
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
        mut self,
        client: C,
    ) -> Result<EventTimeline> {
        let agent = Agent::builder(client, self.config)
            .with_default_tools()
            .build();

        let history = agent.history();

        if let Some(prompt) = &self.initial_user_prompt {
            let mut history_guard = history.write().await;
            history_guard.push(Message::user(prompt));
        }

        let mut timeline = EventTimeline::new();
        let (channels, approval_handle) = create_approval_channels();
        let stream = agent.run_stream_with_approval(Some(channels));

        let mut stream = std::pin::pin!(stream);
        while let Some(event) = stream.next().await {
            match event {
                Ok(event) => {
                    timeline.push(event.clone());

                    if let AgentEvent::ApprovalRequired { request } = &event {
                        let decision = if let Some(script) = self.approvals.pop_front() {
                            assert_eq!(
                                script.tool, request.tool,
                                "Scenario '{}' approval script expected tool '{}', got '{}'",
                                self.scenario_name, script.tool, request.tool
                            );

                            if script.approve {
                                ApprovalDecision::Approve
                            } else {
                                ApprovalDecision::Deny
                            }
                        } else {
                            ApprovalDecision::Deny
                        };

                        approval_handle
                            .decision_tx
                            .send((request.id.clone(), decision))
                            .await
                            .expect("approval decision channel should remain open");

                        timeline.push_approval_decision(
                            request.id.clone(),
                            request.tool.clone(),
                            decision,
                        );
                    }

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
