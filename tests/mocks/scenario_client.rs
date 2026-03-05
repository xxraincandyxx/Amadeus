use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::time::sleep;

use amadeus::client::{LLMClient, StreamEvent};
use amadeus::agent::messages::Message;
use amadeus::error::{AgentError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioDefinition {
    pub name: String,
    pub description: String,
    pub steps: Vec<ScenarioStepDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioStepDef {
    pub delay_ms: Option<u64>,
    pub events: Vec<StreamEventDef>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEventDef {
    TextDelta { text: String },
    ThinkingDelta { text: String },
    ToolCallStart { id: String, name: String },
    ToolCallDelta { arguments: String },
    ToolCallDone { id: String },
    StopReason { reason: String },
    TokenUsage { input_tokens: u32, output_tokens: u32 },
}

impl From<StreamEventDef> for StreamEvent {
    fn from(def: StreamEventDef) -> Self {
        match def {
            StreamEventDef::TextDelta { text } => StreamEvent::TextDelta(text),
            StreamEventDef::ThinkingDelta { text } => StreamEvent::ThinkingDelta(text),
            StreamEventDef::ToolCallStart { id, name } => StreamEvent::ToolCallStart { id, name },
            StreamEventDef::ToolCallDelta { arguments } => StreamEvent::ToolCallDelta { arguments },
            StreamEventDef::ToolCallDone { id } => StreamEvent::ToolCallDone(id),
            StreamEventDef::StopReason { reason } => StreamEvent::StopReason(reason),
            StreamEventDef::TokenUsage { input_tokens, output_tokens } => StreamEvent::TokenUsage { input_tokens, output_tokens },
        }
    }
}

impl From<StreamEvent> for StreamEventDef {
    fn from(event: StreamEvent) -> Self {
        match event {
            StreamEvent::TextDelta(text) => StreamEventDef::TextDelta { text },
            StreamEvent::ThinkingDelta(text) => StreamEventDef::ThinkingDelta { text },
            StreamEvent::ToolCallStart { id, name } => StreamEventDef::ToolCallStart { id, name },
            StreamEvent::ToolCallDelta { arguments } => StreamEventDef::ToolCallDelta { arguments },
            StreamEvent::ToolCallDone(id) => StreamEventDef::ToolCallDone { id },
            StreamEvent::StopReason(reason) => StreamEventDef::StopReason { reason },
            StreamEvent::TokenUsage { input_tokens, output_tokens } => StreamEventDef::TokenUsage { input_tokens, output_tokens },
        }
    }
}

pub struct ScenarioMockClient {
    steps: Arc<Mutex<VecDeque<ScenarioStepDef>>>,
}

impl ScenarioMockClient {
    pub fn new() -> Self {
        Self {
            steps: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
    
    pub fn from_json(json: &str) -> Result<Self> {
        let def: ScenarioDefinition = serde_json::from_str(json)
            .map_err(|e| AgentError::Serde(e))?;
        
        Ok(Self {
            steps: Arc::new(Mutex::new(def.steps.into_iter().collect())),
        })
    }
    
    pub fn scripted(event_batches: Vec<Vec<StreamEvent>>) -> Self {
        let steps: Vec<ScenarioStepDef> = event_batches
            .into_iter()
            .map(|events| ScenarioStepDef {
                delay_ms: None,
                events: events.into_iter().map(|e| e.into()).collect(),
                error: None,
            })
            .collect();
        
        Self {
            steps: Arc::new(Mutex::new(steps.into_iter().collect())),
        }
    }
    
    pub fn from_steps(steps: Vec<ScenarioStepDef>) -> Self {
        Self {
            steps: Arc::new(Mutex::new(steps.into_iter().collect())),
        }
    }
}

impl Default for ScenarioMockClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ScenarioMockClient {
    fn clone(&self) -> Self {
        Self {
            steps: Arc::clone(&self.steps),
        }
    }
}

#[async_trait]
impl LLMClient for ScenarioMockClient {
    async fn create_message(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<(String, Vec<amadeus::agent::messages::ContentBlock>)> {
        Ok(("end_turn".to_string(), vec![]))
    }
    
    async fn create_message_stream(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let mut steps = self.steps.lock().await;
        
        if let Some(step) = steps.pop_front() {
            if let Some(error_msg) = step.error {
                return Err(AgentError::Api(error_msg));
            }
            
            let events: Vec<StreamEvent> = step.events.into_iter().map(|e| e.into()).collect();
            
            if let Some(ms) = step.delay_ms {
                let delayed_events = events.clone();
                let stream = async_stream::try_stream! {
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                    for event in delayed_events {
                        yield event;
                    }
                };
                Ok(Box::pin(stream))
            } else {
                let stream = futures::stream::iter(events.into_iter().map(Ok));
                Ok(Box::pin(stream))
            }
        } else {
            Ok(Box::pin(futures::stream::iter(vec![
                Ok(StreamEvent::StopReason("end_turn".to_string()))
            ])))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_scenario_from_json() {
        let json = r#"{
            "name": "test_scenario",
            "description": "A test scenario",
            "steps": [
                {
                    "delay_ms": null,
                    "events": [
                        {"type": "text_delta", "text": "Hello"},
                        {"type": "stop_reason", "reason": "end_turn"}
                    ],
                    "error": null
                }
            ]
        }"#;
        
        let client = ScenarioMockClient::from_json(json).unwrap();
        let steps = client.steps.lock().await;
        assert_eq!(steps.len(), 1);
    }
    
    #[tokio::test]
    async fn test_scenario_scripted() {
        let client = ScenarioMockClient::scripted(vec![
            vec![
                StreamEvent::TextDelta("Test".to_string()),
                StreamEvent::StopReason("end_turn".to_string()),
            ],
        ]);
        
        let mut steps = client.steps.lock().await;
        assert_eq!(steps.len(), 1);
        let step = steps.pop_front().unwrap();
        assert_eq!(step.events.len(), 2);
    }
}
