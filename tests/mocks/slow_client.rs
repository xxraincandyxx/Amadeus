#![allow(dead_code)]

use std::pin::Pin;
use std::time::Duration;

use async_trait::async_trait;
use futures::Stream;

use amadeus::agent::messages::Message;
use amadeus::client::{LLMClient, StreamEvent};
use amadeus::error::Result;

#[derive(Clone)]
pub struct SlowMockClient {
    base_delay_ms: u64,
    delta_delay_ms: u64,
}

impl SlowMockClient {
    pub fn new(base_delay_ms: u64, delta_delay_ms: u64) -> Self {
        Self {
            base_delay_ms,
            delta_delay_ms,
        }
    }

    pub fn slow() -> Self {
        Self::new(500, 50)
    }

    pub fn very_slow() -> Self {
        Self::new(2000, 200)
    }
}

#[async_trait]
impl LLMClient for SlowMockClient {
    async fn create_message(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<(String, Vec<amadeus::agent::messages::ContentBlock>)> {
        tokio::time::sleep(Duration::from_millis(self.base_delay_ms)).await;
        Ok(("end_turn".to_string(), vec![]))
    }

    async fn create_message_stream(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        let base_delay = self.base_delay_ms;
        let delta_delay = self.delta_delay_ms;

        let stream = async_stream::try_stream! {
            tokio::time::sleep(Duration::from_millis(base_delay)).await;

            let chunks = ["Slow ", "streaming ", "response"];
            for chunk in chunks {
                tokio::time::sleep(Duration::from_millis(delta_delay)).await;
                yield StreamEvent::TextDelta(chunk.to_string());
            }
            yield StreamEvent::StopReason("end_turn".to_string());
        };

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn test_slow_client_delays() {
        let client = SlowMockClient::new(100, 50);

        let start = std::time::Instant::now();
        let stream = client
            .create_message_stream("", &[], &[], 100)
            .await
            .unwrap();

        let events: Vec<_> = stream.collect().await;
        let duration = start.elapsed();

        assert!(duration >= Duration::from_millis(100));
        assert_eq!(events.len(), 4);
    }
}
