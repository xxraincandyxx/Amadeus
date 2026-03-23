//! Test Scenarios
//!
//! Pre-built scripted scenarios for reproducible TUI tests.

use amadeus::agent::messages::{ContentBlock, Message};
use amadeus::client::LLMClient;
use amadeus::client::StreamEvent;
use amadeus::error::Result;
use async_trait::async_trait;
use futures::Stream;
use serde_json::json;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// A scripted scenario for testing
#[derive(Debug, Clone)]
pub struct Scenario {
    pub name: String,
    pub description: String,
    pub events: Vec<Vec<StreamEvent>>,
}

impl Scenario {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            events: Vec::new(),
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn add_turn(mut self, events: Vec<StreamEvent>) -> Self {
        self.events.push(events);
        self
    }
}

/// Simple streaming text response
pub fn simple_text(text: &str) -> Scenario {
    Scenario::new("simple_text")
        .description("Simple streaming text response")
        .add_turn(vec![
            StreamEvent::TextDelta(text.to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ])
}

/// Text streamed token by token
pub fn streaming_text(text: &str) -> Scenario {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut events = Vec::new();

    for word in &words {
        events.push(StreamEvent::TextDelta(format!("{} ", word)));
    }
    events.push(StreamEvent::StopReason("end_turn".to_string()));

    Scenario::new("streaming_text")
        .description("Text streamed word by word")
        .add_turn(events)
}

/// Response with a tool call (ends with tool_use stop reason)
pub fn with_tool_call(command: &str, _output: &str) -> Scenario {
    Scenario::new("with_tool_call")
        .description("Response that includes a tool call")
        .add_turn(vec![
            StreamEvent::TextDelta(format!("Running: {}...", command)),
            StreamEvent::ToolCallStart {
                id: "tool_1".to_string(),
                name: "bash".to_string(),
            },
            StreamEvent::ToolCallDelta {
                arguments: json!({"command": command}).to_string(),
            },
            StreamEvent::ToolCallDone("tool_1".to_string()),
            StreamEvent::StopReason("tool_use".to_string()),
        ])
}

/// Response that requires approval
pub fn requiring_approval(tool: &str, command: &str) -> Scenario {
    Scenario::new("requiring_approval")
        .description("Response that requires user approval")
        .add_turn(vec![
            StreamEvent::TextDelta(format!("I need to run: {}", command)),
            StreamEvent::ToolCallStart {
                id: "tool_1".to_string(),
                name: tool.to_string(),
            },
            StreamEvent::ToolCallDelta {
                arguments: json!({"command": command}).to_string(),
            },
            StreamEvent::ToolCallDone("tool_1".to_string()),
            StreamEvent::StopReason("tool_use".to_string()),
        ])
}

/// Empty response (no output)
pub fn empty() -> Scenario {
    Scenario::new("empty")
        .description("Empty response")
        .add_turn(vec![StreamEvent::StopReason("end_turn".to_string())])
}

/// Long text for testing scrolling
pub fn long_text(len: usize) -> Scenario {
    let text = "Lorem ipsum dolor sit amet. ".repeat(len / 30);
    Scenario::new("long_text")
        .description(format!("Long text of {} chars", len))
        .add_turn(vec![
            StreamEvent::TextDelta(text),
            StreamEvent::StopReason("end_turn".to_string()),
        ])
}

// ============================================================================
// Mock Client
// ============================================================================

/// A mock LLM client that executes scripted scenarios
#[derive(Clone)]
pub struct MockScenarioClient {
    scenario: Scenario,
    turn_index: Arc<Mutex<usize>>,
    event_index: Arc<Mutex<usize>>,
}

impl MockScenarioClient {
    pub fn new(scenario: Scenario) -> Self {
        Self {
            scenario,
            turn_index: Arc::new(Mutex::new(0)),
            event_index: Arc::new(Mutex::new(0)),
        }
    }

    pub fn simple_text(text: &str) -> Self {
        Self::new(simple_text(text))
    }

    pub fn streaming_text(text: &str) -> Self {
        Self::new(streaming_text(text))
    }

    pub fn with_tool_call(command: &str, output: &str) -> Self {
        Self::new(with_tool_call(command, output))
    }
}

#[async_trait]
impl LLMClient for MockScenarioClient {
    async fn create_message(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)> {
        let mut turn_idx = self.turn_index.lock().unwrap();
        if *turn_idx >= self.scenario.events.len() {
            return Ok(("end_turn".to_string(), vec![]));
        }

        let mut event_idx = self.event_index.lock().unwrap();
        let events = &self.scenario.events[*turn_idx];
        let mut blocks = Vec::new();

        for event in events {
            match event {
                StreamEvent::TextDelta(text) => {
                    blocks.push(ContentBlock::Text { text: text.clone() });
                }
                StreamEvent::ToolCallStart { id, name, .. } => {
                    blocks.push(ContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: serde_json::Value::Object(Default::default()),
                    });
                }
                StreamEvent::ToolCallDelta { arguments, .. } => {
                    if let Some(ContentBlock::ToolUse { ref mut input, .. }) = blocks.last_mut() {
                        *input = serde_json::from_str(arguments).unwrap_or_default();
                    }
                }
                StreamEvent::ToolCallDone(_) => {}
                StreamEvent::StopReason(reason) => {
                    *turn_idx += 1;
                    *event_idx = 0;
                    return Ok((reason.clone(), blocks));
                }
                _ => {}
            }
        }

        *event_idx += 1;
        Ok(("end_turn".to_string(), blocks))
    }

    async fn create_message_stream(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let mut turn_idx = self.turn_index.lock().unwrap();
        if *turn_idx >= self.scenario.events.len() {
            let stream =
                futures::stream::iter(vec![Ok(StreamEvent::StopReason("end_turn".to_string()))]);
            return Ok(Box::pin(stream));
        }

        let events = self.scenario.events[*turn_idx].clone();
        *turn_idx += 1;

        let stream = futures::stream::iter(events.into_iter().map(Ok));
        Ok(Box::pin(stream))
    }
}
