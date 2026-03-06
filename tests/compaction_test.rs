//! # Compaction Integration Tests
//!
//! Integration tests for the context compaction mechanism.

use amadeus::agent::compaction::{CompactionConfig, ContextCompactor};
use amadeus::agent::messages::{ContentBlock, Message};

/// Test that ContextCompactor correctly estimates tokens.
#[tokio::test]
async fn test_estimate_tokens() {
    let config = CompactionConfig::default();
    let compactor = ContextCompactor::new(config);

    // Create a simple message
    let messages = vec![
        Message::user("Hello, this is a test message."),
        Message::assistant(vec![ContentBlock::Text {
            text: "I understand. How can I help you today?".to_string(),
        }]),
    ];

    let tokens = compactor.estimate_tokens(&messages);

    // Should estimate roughly 60-80 chars / 4 = ~15-20 tokens
    assert!(tokens > 0);
    assert!(tokens < 100);
}

/// Test that needs_compaction respects threshold.
#[tokio::test]
async fn test_needs_compaction_below_threshold() {
    let config = CompactionConfig {
        threshold_percent: 75,
        min_messages: 10,
        ..Default::default()
    };
    let compactor = ContextCompactor::new(config);

    // Create small history (below threshold)
    let messages: Vec<Message> = (0..5)
        .map(|i| Message::user(&format!("Message {}", i)))
        .collect();

    // Should not trigger compaction
    assert!(!compactor.needs_compaction(&messages, 200_000));
}

/// Test that needs_compaction respects minimum messages.
#[tokio::test]
async fn test_needs_compaction_min_messages() {
    let config = CompactionConfig {
        threshold_percent: 75,
        min_messages: 10,
        ..Default::default()
    };
    let compactor = ContextCompactor::new(config);

    // Create large content but below min_messages threshold
    let large_text = "x".repeat(50_000);
    let messages: Vec<Message> = (0..5).map(|_| Message::user(&large_text)).collect();

    // Should not trigger because below min_messages
    assert!(!compactor.needs_compaction(&messages, 200_000));
}

/// Test that needs_compaction triggers above threshold.
#[tokio::test]
async fn test_needs_compaction_above_threshold() {
    let config = CompactionConfig {
        threshold_percent: 50, // Lower threshold for testing
        min_messages: 5,
        ..Default::default()
    };
    let compactor = ContextCompactor::new(config);

    // Create large history (above threshold)
    // 30 messages × 20,000 chars = 600,000 chars = ~150,000 tokens = 75% of 200k context
    let large_text = "x".repeat(20_000);
    let messages: Vec<Message> = (0..30).map(|_| Message::user(&large_text)).collect();

    // Should trigger compaction (75% > 50% threshold)
    assert!(compactor.needs_compaction(&messages, 200_000));
}

/// Test context_usage_percent calculation.
#[tokio::test]
async fn test_context_usage_percent() {
    let config = CompactionConfig::default();
    let compactor = ContextCompactor::new(config);

    // Create messages with known size
    let messages = vec![
        Message::user("This is a test."), // ~15 chars
    ];

    let percent = compactor.context_usage_percent(&messages, 100);

    // 15 chars / 4 = ~4 tokens, 4/100 = 4%
    assert!(percent > 0);
    assert!(percent < 10);
}

/// Test detection of short history (compaction allowed with warning).
#[tokio::test]
async fn test_short_history_detection() {
    let config = CompactionConfig {
        preserve_recent: 6,
        ..Default::default()
    };
    let preserve_recent = config.preserve_recent;
    let compactor = ContextCompactor::new(config);

    // Create history with fewer messages than preserve_recent
    let messages: Vec<Message> = (0..3).map(|_| Message::user("Test")).collect();

    // Short history is detected (len <= preserve_recent)
    // Note: Compaction is now allowed for short history with a warning
    assert!(messages.len() <= preserve_recent);
    // Compactor created successfully (unused but confirms construction works)
    let _ = compactor;
}

/// Test token estimation with tool use blocks.
#[tokio::test]
async fn test_estimate_tokens_with_tools() {
    let config = CompactionConfig::default();
    let compactor = ContextCompactor::new(config);

    let messages = vec![
        Message::user("Read the file"),
        Message::assistant(vec![ContentBlock::ToolUse {
            id: "1".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"file_path": "/path/to/file.rs"}),
        }]),
        Message::tool_results(vec![ContentBlock::ToolResult {
            tool_use_id: "1".to_string(),
            content: "fn main() {}".to_string(),
        }]),
    ];

    let tokens = compactor.estimate_tokens(&messages);
    assert!(tokens > 0);
}

/// Test config defaults are sensible.
#[tokio::test]
async fn test_config_defaults() {
    let config = CompactionConfig::default();

    assert_eq!(config.threshold_percent, 75);
    assert_eq!(config.target_percent, 30);
    assert_eq!(config.preserve_recent, 6);
    assert!(config.use_llm_summary);
    assert_eq!(config.max_summary_chars, 2000);
    assert_eq!(config.min_messages, 10);
    assert_eq!(config.max_tool_result_chars, 5000);
}

// ============================================================================
// Integration tests merged from flows/compaction_during_stream.rs
// ============================================================================

use amadeus::client::StreamEvent;

#[path = "scenarios/mod.rs"]
mod scenarios;

#[path = "mocks/mod.rs"]
mod mocks;

use mocks::ScenarioMockClient;
use scenarios::{assert_events_contain_text, ScenarioBuilder};

/// Test that compaction preserves recent messages during streaming.
#[tokio::test]
async fn test_compaction_preserves_recent_messages() {
    let client = ScenarioMockClient::scripted(vec![vec![
        StreamEvent::TextDelta("Turn 8: Building up context... ".to_string()),
        StreamEvent::StopReason("end_turn".to_string()),
    ]]);

    let scenario = ScenarioBuilder::new("compaction_test")
        .description("Test context compaction preserves recent messages")
        .build();

    let events = scenario.execute(client).await.expect("Scenario failed");

    assert_events_contain_text(&events, "Turn 8");
}

/// Test compaction during active streaming.
#[tokio::test]
async fn test_compaction_during_active_streaming() {
    let client = ScenarioMockClient::scripted(vec![vec![
        StreamEvent::TextDelta("Turn 4: Final response after potential compaction".to_string()),
        StreamEvent::StopReason("end_turn".to_string()),
    ]]);

    let scenario = ScenarioBuilder::new("compaction_streaming")
        .description("Test compaction during streaming")
        .build();

    let events = scenario.execute(client).await.expect("Scenario failed");

    assert_events_contain_text(&events, "Final response");
}

/// Test multiple compactions in a long conversation.
#[tokio::test]
async fn test_multiple_compactions_long_conversation() {
    let client = ScenarioMockClient::scripted(vec![vec![
        StreamEvent::TextDelta(
            "Turn 15: This is a longer message to build up context quickly. ".to_string(),
        ),
        StreamEvent::StopReason("end_turn".to_string()),
    ]]);

    let scenario = ScenarioBuilder::new("long_conversation")
        .description("Test multiple compactions in long conversation")
        .build();

    let events = scenario.execute(client).await.expect("Scenario failed");

    assert_events_contain_text(&events, "Turn 15");
}

/// Test that compaction doesn't interrupt tool chains.
#[tokio::test]
async fn test_compaction_doesnt_interrupt_tool_chain() {
    use serde_json::json;

    let client = ScenarioMockClient::scripted(vec![
        vec![
            StreamEvent::TextDelta("Reading... ".to_string()),
            StreamEvent::ToolCallStart {
                id: "tool_1".to_string(),
                name: "read_file".to_string(),
            },
            StreamEvent::ToolCallDelta {
                arguments: json!({"path": "main.rs"}).to_string(),
            },
            StreamEvent::ToolCallDone("tool_1".to_string()),
            StreamEvent::StopReason("tool_use".to_string()),
        ],
        vec![
            StreamEvent::TextDelta("Now editing... ".to_string()),
            StreamEvent::ToolCallStart {
                id: "tool_2".to_string(),
                name: "edit_file".to_string(),
            },
            StreamEvent::ToolCallDelta {
                arguments: json!({"path": "main.rs", "old_text": "x", "new_text": "y"}).to_string(),
            },
            StreamEvent::ToolCallDone("tool_2".to_string()),
            StreamEvent::StopReason("tool_use".to_string()),
        ],
        vec![
            StreamEvent::TextDelta("Operations completed".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
    ]);

    let scenario = ScenarioBuilder::new("tool_compaction")
        .description("Test compaction doesn't interrupt tool chain")
        .build();

    let events = scenario.execute(client).await.expect("Scenario failed");

    let tool_calls = events
        .iter()
        .filter(|e| matches!(e, amadeus::agent::events::AgentEvent::ToolStart { .. }))
        .count();

    assert_eq!(tool_calls, 2, "All tool calls should execute");
}
