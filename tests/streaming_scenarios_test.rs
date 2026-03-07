use amadeus::client::StreamEvent;

#[path = "scenarios/mod.rs"]
mod scenarios;

#[path = "mocks/mod.rs"]
mod mocks;

use mocks::FlakyMockClient;
use mocks::ScenarioMockClient;
use mocks::SlowMockClient;
use scenarios::{ScenarioBuilder, ScenarioRunner};

#[tokio::test]
async fn test_streaming_single_continuous_block() {
    let client = ScenarioMockClient::scripted(vec![vec![
        StreamEvent::TextDelta("This is a streaming response.".to_string()),
        StreamEvent::StopReason("end_turn".to_string()),
    ]]);

    let scenario = ScenarioBuilder::new("streaming_test")
        .description("Test streaming text accumulation")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let (_events, text) = runner
        .execute_and_collect_text(client)
        .await
        .expect("Scenario execution failed");

    assert_eq!(text, "This is a streaming response.");
}

#[tokio::test]
async fn test_streaming_interrupted_by_tool_call() {
    let client = ScenarioMockClient::scripted(vec![
        vec![
            StreamEvent::TextDelta("Reading file... ".to_string()),
            StreamEvent::ToolCallStart {
                id: "tool_1".to_string(),
                name: "read_file".to_string(),
            },
            StreamEvent::ToolCallDelta {
                arguments: r#"{"path":"main.rs"}"#.to_string(),
            },
            StreamEvent::ToolCallDone("tool_1".to_string()),
            StreamEvent::StopReason("tool_use".to_string()),
        ],
        vec![
            StreamEvent::TextDelta("File contents: ... ".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
    ]);

    let scenario = ScenarioBuilder::new("tool_interrupt")
        .description("Test streaming interrupted by tool call")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let events = runner
        .execute(client)
        .await
        .expect("Scenario execution failed");

    let text_events = events
        .iter()
        .filter(|e| matches!(e, amadeus::agent::events::AgentEvent::TextDelta { .. }))
        .count();

    assert_eq!(text_events, 2);
}

#[tokio::test]
async fn test_done_result_excludes_pre_tool_text() {
    let client = ScenarioMockClient::scripted(vec![
        vec![
            StreamEvent::TextDelta("Let me check... ".to_string()),
            StreamEvent::ToolCallStart {
                id: "tool_1".to_string(),
                name: "bash".to_string(),
            },
            StreamEvent::ToolCallDelta {
                arguments: r#"{"command":"printf ok"}"#.to_string(),
            },
            StreamEvent::ToolCallDone("tool_1".to_string()),
            StreamEvent::StopReason("tool_use".to_string()),
        ],
        vec![
            StreamEvent::TextDelta("Done checking.".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
    ]);

    let scenario = ScenarioBuilder::new("tool_result_text")
        .description("Final result text should not duplicate pre-tool narration")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let timeline = runner
        .execute_timeline(client)
        .await
        .expect("Scenario execution failed");

    let result = timeline.run_result().expect("missing run result");

    assert_eq!(timeline.full_text(), "Let me check... Done checking.");
    assert_eq!(result.text, "Done checking.");
}

#[tokio::test]
async fn test_streaming_very_long_response() {
    let long_text = (0..100)
        .map(|i| format!("Chunk {} ", i))
        .collect::<String>();

    let client = ScenarioMockClient::scripted(vec![vec![
        StreamEvent::TextDelta(long_text.clone()),
        StreamEvent::StopReason("end_turn".to_string()),
    ]]);

    let scenario = ScenarioBuilder::new("long_streaming")
        .description("Test very long streaming response")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let (_events, text) = runner
        .execute_and_collect_text(client)
        .await
        .expect("Scenario execution failed");

    assert_eq!(text.len(), long_text.len());
}

#[tokio::test]
async fn test_streaming_rapid_chunks() {
    let all_chunks: String = (0..20).map(|i| format!("Chunk{} ", i)).collect();

    let client = ScenarioMockClient::scripted(vec![vec![
        StreamEvent::TextDelta(all_chunks.clone()),
        StreamEvent::StopReason("end_turn".to_string()),
    ]]);

    let scenario = ScenarioBuilder::new("rapid_chunks")
        .description("Test rapid consecutive chunks")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let (_events, text) = runner
        .execute_and_collect_text(client)
        .await
        .expect("Scenario execution failed");

    assert!(text.starts_with("Chunk0"));
    assert!(text.contains("Chunk19"));
}

#[tokio::test]
async fn test_streaming_with_delays() {
    use std::time::Duration;

    let start = std::time::Instant::now();

    let client = SlowMockClient::slow();

    let scenario = ScenarioBuilder::new("slow_streaming")
        .description("Test streaming with delays")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let events = runner
        .execute(client)
        .await
        .expect("Scenario execution failed");

    let duration = start.elapsed();

    assert!(duration >= Duration::from_millis(500));
    assert!(!events.is_empty());
}

#[tokio::test]
async fn test_streaming_very_slow() {
    use std::time::Duration;

    let start = std::time::Instant::now();
    let client = SlowMockClient::very_slow();

    let scenario = ScenarioBuilder::new("very_slow_streaming")
        .description("Test streaming with very long delays")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let events = runner
        .execute(client)
        .await
        .expect("Scenario execution failed");

    let duration = start.elapsed();

    assert!(duration >= Duration::from_millis(2000));
    assert!(!events.is_empty());
}

#[tokio::test]
async fn test_streaming_error_recovery() {
    let client = FlakyMockClient::with_failures(vec![0]);

    let scenario = ScenarioBuilder::new("error_streaming")
        .description("Test streaming error handling")
        .build();

    let runner = ScenarioRunner::new(scenario);
    let result = runner.execute(client).await;

    assert!(
        result.is_err()
            || result
                .unwrap()
                .iter()
                .any(|e| { matches!(e, amadeus::agent::events::AgentEvent::Error { .. }) })
    );
}
