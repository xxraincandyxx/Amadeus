#![allow(dead_code)]
// @amadeus-header
// summary: Test mock implementation for scenario client.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::mocks::scenario_client
// - type: tests::mocks::scenario_client::ScenarioDefinition
// - type: tests::mocks::scenario_client::ScenarioStepDef
// - type: tests::mocks::scenario_client::StreamEventDef
// - type: tests::mocks::scenario_client::CapturedRequest
// - type: tests::mocks::scenario_client::ScenarioMockClient
// uses:
// - module: amadeus::agent::messages::Message
// - module: amadeus::client
// - module: amadeus::error
// - runtime: tokio async runtime
// - protocol: serde serialization
// - runtime: futures streams
// invariants:
// - Assertions stay aligned with current user-visible behavior.
// side_effects: none
// tests:
// - cmd: cargo test scenario_client --features full
// @end-amadeus-header

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use amadeus::agent::messages::Message;
use amadeus::client::{LLMClient, StreamEvent};
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
    TextDelta {
        text: String,
    },
    ThinkingDelta {
        text: String,
    },
    ToolCallStart {
        id: String,
        name: String,
    },
    ToolCallDelta {
        arguments: String,
    },
    ToolCallDone {
        id: String,
    },
    StopReason {
        reason: String,
    },
    TokenUsage {
        input_tokens: u32,
        output_tokens: u32,
    },
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
            StreamEventDef::TokenUsage {
                input_tokens,
                output_tokens,
            } => StreamEvent::TokenUsage {
                input_tokens,
                output_tokens,
            },
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
            StreamEvent::TokenUsage {
                input_tokens,
                output_tokens,
            } => StreamEventDef::TokenUsage {
                input_tokens,
                output_tokens,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CapturedRequest {
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<serde_json::Value>,
    pub max_tokens: u32,
}

#[derive(Clone)]
pub struct ScenarioMockClient {
    steps: Arc<Mutex<VecDeque<ScenarioStepDef>>>,
    captured_requests: Arc<Mutex<Vec<CapturedRequest>>>,
}

impl ScenarioMockClient {
    pub fn new() -> Self {
        Self {
            steps: Arc::new(Mutex::new(VecDeque::new())),
            captured_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let def: ScenarioDefinition = serde_json::from_str(json).map_err(AgentError::Serde)?;

        Ok(Self {
            steps: Arc::new(Mutex::new(def.steps.into_iter().collect())),
            captured_requests: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn scripted(event_batches: Vec<Vec<StreamEvent>>) -> Self {
        let steps: VecDeque<ScenarioStepDef> = event_batches
            .into_iter()
            .map(|events| ScenarioStepDef {
                delay_ms: None,
                events: events.into_iter().map(Into::into).collect(),
                error: None,
            })
            .collect();

        Self {
            steps: Arc::new(Mutex::new(steps)),
            captured_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn from_steps(steps: Vec<ScenarioStepDef>) -> Self {
        Self {
            steps: Arc::new(Mutex::new(steps.into_iter().collect())),
            captured_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn remaining_steps(&self) -> usize {
        self.steps.try_lock().map(|s| s.len()).unwrap_or(0)
    }

    pub fn captured_requests(&self) -> Vec<CapturedRequest> {
        self.captured_requests
            .try_lock()
            .map(|r| r.clone())
            .unwrap_or_default()
    }

    pub fn request_count(&self) -> usize {
        self.captured_requests
            .try_lock()
            .map(|r| r.len())
            .unwrap_or(0)
    }

    pub fn nth_request(&self, n: usize) -> Option<CapturedRequest> {
        self.captured_requests
            .try_lock()
            .ok()
            .and_then(|r| r.get(n).cloned())
    }

    pub fn last_request(&self) -> Option<CapturedRequest> {
        self.captured_requests
            .try_lock()
            .ok()
            .and_then(|r| r.last().cloned())
    }

    pub fn last_messages(&self) -> Vec<Message> {
        self.captured_requests
            .try_lock()
            .ok()
            .and_then(|r| r.last().map(|req| req.messages.clone()))
            .unwrap_or_default()
    }
}

impl Default for ScenarioMockClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LLMClient for ScenarioMockClient {
    async fn create_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        max_tokens: u32,
    ) -> Result<(String, Vec<amadeus::agent::messages::ContentBlock>)> {
        self.captured_requests.lock().await.push(CapturedRequest {
            system: system.to_string(),
            messages: messages.to_vec(),
            tools: tools.to_vec(),
            max_tokens,
        });
        Ok(("end_turn".to_string(), vec![]))
    }

    async fn create_message_stream(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        self.captured_requests.lock().await.push(CapturedRequest {
            system: system.to_string(),
            messages: messages.to_vec(),
            tools: tools.to_vec(),
            max_tokens,
        });

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
            Ok(Box::pin(futures::stream::iter(vec![Ok(
                StreamEvent::StopReason("end_turn".to_string()),
            )])))
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
        let client = ScenarioMockClient::scripted(vec![vec![
            StreamEvent::TextDelta("Test".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ]]);

        let mut steps = client.steps.lock().await;
        assert_eq!(steps.len(), 1);
        let step = steps.pop_front().unwrap();
        assert_eq!(step.events.len(), 2);
    }

    #[tokio::test]
    async fn test_request_capture() {
        let client = ScenarioMockClient::scripted(vec![vec![
            StreamEvent::TextDelta("Hello".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ]]);

        let _ = client
            .create_message_stream("system prompt", &[], &[], 1000)
            .await
            .unwrap();

        assert_eq!(client.request_count(), 1);
        let req = client.nth_request(0).unwrap();
        assert_eq!(req.system, "system prompt");
        assert_eq!(req.max_tokens, 1000);
    }
}
