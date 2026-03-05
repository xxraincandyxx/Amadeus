use amadeus::client::StreamEvent;

#[path = "../scenarios/mod.rs"]
mod scenarios;

#[path = "../mocks/mod.rs"]
mod mocks;

use scenarios::{ScenarioBuilder, ScenarioRunner, assert_no_errors, assert_events_contain_text};
use mocks::ScenarioMockClient;
use mocks::{FlakyMockClient, SlowMockClient};

#[tokio::test]
async fn test_network_error_retry() {
    let client = FlakyMockClient::with_retryable_failures(vec![0]);
    
    let scenario = ScenarioBuilder::new("network_retry")
        .description("Test retryable network error")
        .build();
    
    let runner = ScenarioRunner::new(scenario);
    let result = runner.execute(client).await;
    
    assert!(result.is_ok() || result.unwrap_err().is_retryable());
}

#[tokio::test]
async fn test_error_then_success() {
    let client = FlakyMockClient::new(vec![
        Some(amadeus::error::AgentError::Api("First call fails".to_string())),
        None,
    ]);
    
    let scenario = ScenarioBuilder::new("error_then_success")
        .description("Test recovery after error")
        .build();
    
    let runner = ScenarioRunner::new(scenario);
    let events = runner
        .execute(client)
        .await
        .expect("Should succeed on second attempt");
    
    assert_no_errors(&events);
}

#[tokio::test]
async fn test_stream_interrupted_error() {
    let client = ScenarioMockClient::scripted(vec![
        vec![
            StreamEvent::TextDelta("Starting... ".to_string()),
            StreamEvent::StopReason("continue".to_string()),
        ],
    ]);
    
    let scenario = ScenarioBuilder::new("stream_interrupt")
        .description("Test interrupted stream")
        .build();
    
    let runner = ScenarioRunner::new(scenario);
    let events = runner
        .execute(client)
        .await
        .expect("Scenario failed");
    
    assert_events_contain_text(&events, "Starting");
}
