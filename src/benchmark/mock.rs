use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::Stream;
use tokio::sync::Mutex;

use crate::agent::messages::{ContentBlock, Message};
use crate::benchmark::case::{MockScript, MockStep};
use crate::client::{LLMClient, StreamEvent};
use crate::error::{AgentError, Result};

#[derive(Debug)]
pub struct BenchmarkMockClient {
    steps: Arc<Mutex<VecDeque<MockStep>>>,
}

impl BenchmarkMockClient {
    pub fn new(script: MockScript) -> Self {
        Self {
            steps: Arc::new(Mutex::new(script.steps.into_iter().collect())),
        }
    }
}

impl Clone for BenchmarkMockClient {
    fn clone(&self) -> Self {
        Self {
            steps: Arc::clone(&self.steps),
        }
    }
}

#[async_trait]
impl LLMClient for BenchmarkMockClient {
    async fn create_message(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)> {
        Ok(("end_turn".to_string(), Vec::new()))
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
            if let Some(error) = step.error {
                return Err(AgentError::Api(error));
            }

            let events: Vec<StreamEvent> = step.events.into_iter().map(Into::into).collect();
            if let Some(delay_ms) = step.delay_ms {
                let stream = async_stream::try_stream! {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    for event in events {
                        yield event;
                    }
                };
                Ok(Box::pin(stream))
            } else {
                let stream = futures::stream::iter(events.into_iter().map(Ok));
                Ok(Box::pin(stream))
            }
        } else {
            let stream =
                futures::stream::iter(vec![Ok(StreamEvent::StopReason("end_turn".to_string()))]);
            Ok(Box::pin(stream))
        }
    }
}
