use amadeus::client::StreamEvent;

#[path = "scenarios/mod.rs"]
mod scenarios;

#[path = "mocks/mod.rs"]
mod mocks;

use mocks::ScenarioMockClient;
use scenarios::{assert_events_contain_text, ScenarioBuilder, ScenarioRunner};

#[tokio::test]
async fn stress_10k_chars_rapid_streaming() {
    let all_chunks: String = (0..100)
        .map(|i| format!("Chunk {}: {}", i, "Lorem ipsum ".repeat(10)))
        .collect();

    let client = ScenarioMockClient::scripted(vec![vec![
        StreamEvent::TextDelta(all_chunks.clone()),
        StreamEvent::StopReason("end_turn".to_string()),
    ]]);

    let scenario = ScenarioBuilder::new("10k_streaming")
        .description("Stress test: 10k chars rapid streaming")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let (_events, text) = runner
        .execute_and_collect_text(client)
        .await
        .expect("Scenario failed");

    assert!(text.len() > 10000, "Should accumulate all characters");
}

#[tokio::test]
async fn stress_100_consecutive_chunks() {
    let all_chunks: String = (0..100).map(|i| format!("Message {} ", i)).collect();

    let client = ScenarioMockClient::scripted(vec![vec![
        StreamEvent::TextDelta(all_chunks.clone()),
        StreamEvent::StopReason("end_turn".to_string()),
    ]]);

    let scenario = ScenarioBuilder::new("100_chunks")
        .description("Stress test: 100 consecutive chunks")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let events = runner.execute(client).await.expect("Scenario failed");

    assert_events_contain_text(&events, "Message 99");
}

#[tokio::test]
async fn stress_unicode_heavy_streaming() {
    let chunks: Vec<Vec<StreamEvent>> = (0..50)
        .map(|i| {
            vec![
                StreamEvent::TextDelta(format!("Message {}: 你好世界 ", i)),
                StreamEvent::StopReason("continue".to_string()),
            ]
        })
        .collect();

    let client = ScenarioMockClient::scripted(chunks);

    let scenario = ScenarioBuilder::new("unicode_streaming")
        .description("Stress test: Unicode heavy streaming")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let (_events, text) = runner
        .execute_and_collect_text(client)
        .await
        .expect("Scenario failed");

    assert!(text.contains("你好世界"));
}
