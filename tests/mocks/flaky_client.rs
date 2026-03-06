use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures::Stream;

use amadeus::agent::messages::Message;
use amadeus::client::{LLMClient, StreamEvent};
use amadeus::error::{AgentError, Result};

pub struct FlakyMockClient {
    failure_schedule: Vec<Option<String>>,
    call_count: Arc<AtomicUsize>,
}

impl FlakyMockClient {
    pub fn new(failure_schedule: Vec<Option<String>>) -> Self {
        Self {
            failure_schedule,
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn with_failures(turn_numbers: Vec<usize>) -> Self {
        let max_turn = turn_numbers.iter().max().copied().unwrap_or(0);
        let mut schedule = vec![None; max_turn + 1];

        for turn in turn_numbers {
            schedule[turn] = Some("Simulated failure".to_string());
        }

        Self::new(schedule)
    }

    pub fn with_retryable_failures(turn_numbers: Vec<usize>) -> Self {
        let max_turn = turn_numbers.iter().max().copied().unwrap_or(0);
        let mut schedule = vec![None; max_turn + 1];

        for turn in turn_numbers {
            schedule[turn] = Some("503 Service Unavailable".to_string());
        }

        Self::new(schedule)
    }
}

impl Clone for FlakyMockClient {
    fn clone(&self) -> Self {
        Self {
            failure_schedule: self.failure_schedule.clone(),
            call_count: Arc::clone(&self.call_count),
        }
    }
}

#[async_trait]
impl LLMClient for FlakyMockClient {
    async fn create_message(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<(String, Vec<amadeus::agent::messages::ContentBlock>)> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);

        if let Some(Some(error_msg)) = self.failure_schedule.get(count) {
            Err(AgentError::Api(error_msg.clone()))
        } else {
            Ok(("end_turn".to_string(), vec![]))
        }
    }

    async fn create_message_stream(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);

        if let Some(Some(error_msg)) = self.failure_schedule.get(count) {
            Err(AgentError::Api(error_msg.clone()))
        } else {
            let events = vec![
                Ok(StreamEvent::TextDelta("Success".to_string())),
                Ok(StreamEvent::StopReason("end_turn".to_string())),
            ];
            Ok(Box::pin(futures::stream::iter(events)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_flaky_client_succeeds() {
        let client = FlakyMockClient::new(vec![None, None]);

        let result = client.create_message("", &[], &[], 100).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_flaky_client_fails() {
        let client = FlakyMockClient::with_failures(vec![0]);

        let result = client.create_message("", &[], &[], 100).await;
        assert!(result.is_err());
    }
}
