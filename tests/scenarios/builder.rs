use amadeus::client::StreamEvent;
use amadeus::error::AgentError;
use serde_json::Value;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct ScenarioStep {
    pub delay_ms: Option<u64>,
    pub events: Vec<StreamEvent>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Scenario {
    pub name: String,
    pub description: String,
    pub steps: VecDeque<ScenarioStep>,
}

impl Scenario {
    pub fn execute<C: amadeus::client::LLMClient + Clone + 'static>(
        self,
        client: C,
    ) -> impl std::future::Future<
        Output = amadeus::error::Result<Vec<amadeus::agent::events::AgentEvent>>,
    > + Send {
        super::runner::ScenarioRunner::new(self).execute(client)
    }

    pub fn execute_and_collect_text<C: amadeus::client::LLMClient + Clone + 'static>(
        self,
        client: C,
    ) -> impl std::future::Future<
        Output = amadeus::error::Result<(Vec<amadeus::agent::events::AgentEvent>, String)>,
    > + Send {
        super::runner::ScenarioRunner::new(self).execute_and_collect_text(client)
    }
}

pub struct ScenarioBuilder {
    name: String,
    description: String,
    steps: Vec<ScenarioStep>,
}

impl ScenarioBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            steps: Vec::new(),
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn user_says(mut self, text: &str) -> Self {
        self.steps.push(ScenarioStep {
            delay_ms: None,
            events: vec![StreamEvent::TextDelta(text.to_string())],
            error: None,
        });
        self
    }

    pub fn agent_responds(mut self, text: &str) -> Self {
        self.steps.push(ScenarioStep {
            delay_ms: None,
            events: vec![
                StreamEvent::TextDelta(text.to_string()),
                StreamEvent::StopReason("end_turn".to_string()),
            ],
            error: None,
        });
        self
    }

    pub fn agent_calls_tool(mut self, tool: &str, args: Value) -> Self {
        let tool_id = format!("tool_{}", self.steps.len());
        self.steps.push(ScenarioStep {
            delay_ms: None,
            events: vec![
                StreamEvent::ToolCallStart {
                    id: tool_id.clone(),
                    name: tool.to_string(),
                },
                StreamEvent::ToolCallDelta {
                    arguments: args.to_string(),
                },
                StreamEvent::ToolCallDone(tool_id),
                StreamEvent::StopReason("tool_use".to_string()),
            ],
            error: None,
        });
        self
    }

    pub fn tool_returns(mut self, result: &str) -> Self {
        self.steps.push(ScenarioStep {
            delay_ms: None,
            events: vec![
                StreamEvent::TextDelta(format!("Tool result: {}", result)),
                StreamEvent::StopReason("end_turn".to_string()),
            ],
            error: None,
        });
        self
    }

    pub fn user_approves(self) -> Self {
        self
    }

    pub fn user_denies(self) -> Self {
        self
    }

    pub fn expect_error(mut self, error_msg: &str) -> Self {
        if let Some(last_step) = self.steps.last_mut() {
            last_step.error = Some(error_msg.to_string());
        }
        self
    }

    pub fn with_delay(mut self, ms: u64) -> Self {
        if let Some(last_step) = self.steps.last_mut() {
            last_step.delay_ms = Some(ms);
        }
        self
    }

    pub fn raw_events(mut self, events: Vec<StreamEvent>) -> Self {
        self.steps.push(ScenarioStep {
            delay_ms: None,
            events,
            error: None,
        });
        self
    }

    pub fn build(self) -> Scenario {
        Scenario {
            name: self.name,
            description: self.description,
            steps: self.steps.into_iter().collect(),
        }
    }
}

impl Default for ScenarioBuilder {
    fn default() -> Self {
        Self::new("unnamed_scenario")
    }
}
