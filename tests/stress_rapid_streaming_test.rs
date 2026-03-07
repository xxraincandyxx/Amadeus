use amadeus::client::StreamEvent;

#[path = "scenarios/mod.rs"]
mod scenarios;

#[path = "mocks/mod.rs"]
mod mocks;

use mocks::ScenarioMockClient;
use scenarios::{assert_events_contain_text, ScenarioBuilder, ScenarioRunner};

#[tokio::test]
async fn stress_10k_chars_rapid_streaming() {
    let chunks: Vec<StreamEvent> = (0..100)
        .map(|i| StreamEvent::TextDelta(format!("Chunk {}: {}", i, "Lorem ipsum ".repeat(10))))
        .chain(std::iter::once(StreamEvent::StopReason(
            "end_turn".to_string(),
        )))
        .collect();

    let client = ScenarioMockClient::scripted(vec![chunks]);

    let scenario = ScenarioBuilder::new("10k_streaming")
        .description("Stress test: 10k chars rapid streaming")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let (_events, text) = runner
        .execute_and_collect_text(client)
        .await
        .expect("Scenario failed");

    assert!(text.len() > 10000, "Should accumulate all characters");
    assert!(text.contains("Chunk 99"));
}

#[tokio::test]
async fn stress_100_consecutive_chunks() {
    let chunks: Vec<StreamEvent> = (0..100)
        .map(|i| StreamEvent::TextDelta(format!("Message {} ", i)))
        .chain(std::iter::once(StreamEvent::StopReason(
            "end_turn".to_string(),
        )))
        .collect();

    let client = ScenarioMockClient::scripted(vec![chunks]);

    let scenario = ScenarioBuilder::new("100_chunks")
        .description("Stress test: 100 consecutive chunks")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let events = runner.execute(client).await.expect("Scenario failed");

    assert_events_contain_text(&events, "Message 99");
}

#[tokio::test]
async fn stress_unicode_heavy_streaming() {
    let chunks: Vec<StreamEvent> = (0..50)
        .map(|i| {
            StreamEvent::TextDelta(format!("Message {}: \u{4f60}\u{597d}\u{4e16}\u{754c} ", i))
        })
        .chain(std::iter::once(StreamEvent::StopReason(
            "end_turn".to_string(),
        )))
        .collect();

    let client = ScenarioMockClient::scripted(vec![chunks]);

    let scenario = ScenarioBuilder::new("unicode_streaming")
        .description("Stress test: Unicode heavy streaming")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let (_events, text) = runner
        .execute_and_collect_text(client)
        .await
        .expect("Scenario failed");

    assert!(text.contains("\u{4f60}\u{597d}\u{4e16}\u{754c}"));
    assert!(text.contains("Message 49"));
}
