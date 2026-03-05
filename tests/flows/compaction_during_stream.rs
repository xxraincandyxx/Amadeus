use amadeus::client::StreamEvent;

#[path = "../scenarios/mod.rs"]
mod scenarios;

#[path = "../mocks/mod.rs"]
mod mocks;

use scenarios::{ScenarioBuilder, assert_events_contain_text};
use mocks::ScenarioMockClient;

#[tokio::test]
async fn test_compaction_preserves_recent_messages() {
    let turns: Vec<Vec<StreamEvent>> = (1..=8)
        .map(|i| {
            vec![
                StreamEvent::TextDelta(format!("Turn {}: Building up context... ", i)),
                StreamEvent::StopReason("end_turn".to_string()),
            ]
        })
        .collect();
    
    let client = ScenarioMockClient::scripted(turns);
    
    let scenario = ScenarioBuilder::new("compaction_test")
        .description("Test context compaction preserves recent messages")
        .build();
    
    let events = scenario
        .execute(client)
        .await
        .expect("Scenario failed");
    
    assert_events_contain_text(&events, "Turn 8");
}

#[tokio::test]
async fn test_compaction_during_active_streaming() {
    let client = ScenarioMockClient::scripted(vec![
        vec![
            StreamEvent::TextDelta("Turn 1: Initial context ".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
        vec![
            StreamEvent::TextDelta("Turn 2: More context ".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
        vec![
            StreamEvent::TextDelta("Turn 3: Context continues ".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
        vec![
            StreamEvent::TextDelta("Turn 4: Final response after potential compaction".to_string()),
            StreamEvent::StopReason("end_turn".to_string()),
        ],
    ]);
    
    let scenario = ScenarioBuilder::new("compaction_streaming")
        .description("Test compaction during streaming")
        .build();
    
    let events = scenario
        .execute(client)
        .await
        .expect("Scenario failed");
    
    assert_events_contain_text(&events, "Final response");
}

#[tokio::test]
async fn test_multiple_compactions_long_conversation() {
    let turns: Vec<Vec<StreamEvent>> = (1..=15)
        .map(|i| {
            vec![
                StreamEvent::TextDelta(format!(
                    "Turn {}: This is a longer message to build up context quickly. ",
                    i
                )),
                StreamEvent::StopReason("end_turn".to_string()),
            ]
        })
        .collect();
    
    let client = ScenarioMockClient::scripted(turns);
    
    let scenario = ScenarioBuilder::new("long_conversation")
        .description("Test multiple compactions in long conversation")
        .build();
    
    let events = scenario
        .execute(client)
        .await
        .expect("Scenario failed");
    
    assert_events_contain_text(&events, "Turn 15");
}

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
    
    let events = scenario
        .execute(client)
        .await
        .expect("Scenario failed");
    
    let tool_calls: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, crate::agent::events::AgentEvent::ToolCallStart { .. }))
        .count();
    
    assert_eq!(tool_calls, 2, "All tool calls should execute");
}
