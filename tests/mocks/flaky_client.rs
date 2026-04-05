#![allow(dead_code)]
// @amadeus-header
// summary: Test mock implementation for flaky client.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::mocks::flaky_client
// - type: tests::mocks::flaky_client::FlakyMockClient
// uses:
// - module: amadeus::agent::messages::Message
// - module: amadeus::client
// - module: amadeus::error
// - runtime: futures streams
// invariants:
// - Assertions stay aligned with current user-visible behavior.
// side_effects: none
// tests:
// - cmd: cargo test flaky_client --features full
// @end-amadeus-header

use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures::Stream;

use amadeus::agent::messages::Message;
use amadeus::client::{LLMClient, StreamEvent};
use amadeus::error::{AgentError, Result};

#[derive(Clone)]
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

    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    fn check_failure(&self) -> std::result::Result<(), AgentError> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        if let Some(Some(error_msg)) = self.failure_schedule.get(count) {
            Err(AgentError::Api(error_msg.clone()))
        } else {
            Ok(())
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
        self.check_failure()?;
        Ok(("end_turn".to_string(), vec![]))
    }

    async fn create_message_stream(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        self.check_failure()?;
        let events = vec![
            Ok(StreamEvent::TextDelta("Success".to_string())),
            Ok(StreamEvent::StopReason("end_turn".to_string())),
        ];
        Ok(Box::pin(futures::stream::iter(events)))
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
        assert_eq!(client.call_count(), 1);
    }

    #[tokio::test]
    async fn test_flaky_client_fails() {
        let client = FlakyMockClient::with_failures(vec![0]);

        let result = client.create_message("", &[], &[], 100).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_flaky_client_fail_then_succeed() {
        let client = FlakyMockClient::with_failures(vec![0]);

        let r1 = client.create_message_stream("", &[], &[], 100).await;
        assert!(r1.is_err());

        let r2 = client.create_message_stream("", &[], &[], 100).await;
        assert!(r2.is_ok());
    }
}
