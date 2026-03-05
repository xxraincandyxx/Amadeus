use std::pin::Pin;
use std::time::Duration;

use async_trait::async_trait;
use futures::Stream;
use tokio::time::sleep;

use amadeus::client::{LLMClient, StreamEvent};
use amadeus::agent::messages::Message;
use amadeus::error::Result;

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

impl Clone for SlowMockClient {
    fn clone(&self) -> Self {
        Self {
            base_delay_ms: self.base_delay_ms,
            delta_delay_ms: self.delta_delay_ms,
        }
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
        sleep(Duration::from_millis(self.base_delay_ms)).await;
        Ok(("end_turn".to_string(), vec![]))
    }
    
    async fn create_message_stream(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
        tokio::time::sleep(Duration::from_millis(self.base_delay_ms)).await;
        
        let events = vec![
            Ok(StreamEvent::TextDelta("Slow ".to_string())),
            Ok(StreamEvent::TextDelta("streaming ".to_string())),
            Ok(StreamEvent::TextDelta("response".to_string())),
            Ok(StreamEvent::StopReason("end_turn".to_string())),
        ];
        
        let base_delay = self.base_delay_ms;
        tokio::time::sleep(Duration::from_millis(base_delay)).await;
        
        Ok(Box::pin(futures::stream::iter(events)))
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
        let stream = client.create_message_stream("", &[], &[], 100).await.unwrap();
        
        let events: Vec<_> = stream.collect().await;
        let duration = start.elapsed();
        
        assert!(duration >= Duration::from_millis(100));
        assert_eq!(events.len(), 4);
    }
}
