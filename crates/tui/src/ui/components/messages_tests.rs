// @amadeus-header
// summary: Unit tests for transcript grouping, markdown rendering, visibility, and message component state.
// layer: test
// status: test-only
// feature_flags:
// - tui
// provides:
// - module: crate::ui::components::messages::tests
// uses:
// - type: crate::ui::components::messages::MessagesComponent
// invariants:
// - Assertions preserve current transcript rendering and grouping behavior.
// side_effects: none
// tests:
// - cmd: cargo test -p tui --all-features
// @end-amadeus-header

use super::*;
use crate::{ContextEntry, ContextSection, ContextSectionGroup};
use ratatui::{backend::TestBackend, Terminal};

#[test]
fn test_messages_new() {
    let messages = MessagesComponent::new();
    assert!(messages.is_empty());
    assert_eq!(messages.len(), 0);
    assert_eq!(messages.turn_counter, 0);
}

#[test]
fn test_add_user_message() {
    let mut messages = MessagesComponent::new();
    messages.add_user("Hello".to_string());

    assert_eq!(messages.len(), 1);
    assert_eq!(messages.turn_counter, 1);
}

#[test]
fn test_add_assistant_message() {
    let mut messages = MessagesComponent::new();
    messages.add_user("Hello".to_string());
    messages.add_assistant("Hi there!".to_string());

    assert_eq!(messages.len(), 2);
    // Turn counter should still be 1 (same turn)
    assert_eq!(messages.turn_counter, 1);
}

#[test]
fn test_turn_counter_increments() {
    let mut messages = MessagesComponent::new();

    // First turn
    messages.add_user("Q1".to_string());
    assert_eq!(messages.turn_counter, 1);

    messages.add_assistant("A1".to_string());

    // Second turn
    messages.add_user("Q2".to_string());
    assert_eq!(messages.turn_counter, 2);

    messages.add_assistant("A2".to_string());
}

#[test]
fn test_update_streaming_text() {
    let mut messages = MessagesComponent::new();
    messages.add_user("Hello".to_string());

    messages.update_streaming_text("Partial response");
    assert!(messages.streaming_text.is_some());

    messages.finalize_assistant("Full response".to_string());
    assert!(messages.streaming_text.is_none());
}

#[test]
fn test_thinking_delta() {
    let mut messages = MessagesComponent::new();
    messages.add_user("Hello".to_string());

    messages.update_thinking("Thinking about this...");
    assert!(messages.streaming_thinking.is_some());

    messages.update_thinking(" Still thinking...");
    let thinking = messages.streaming_thinking.as_ref().unwrap();
    assert!(thinking.contains("Thinking"));
    assert!(thinking.contains("Still"));
}

#[test]
fn test_finalize_thinking() {
    let mut messages = MessagesComponent::new();
    messages.add_user("Hello".to_string());

    messages.update_thinking("My reasoning process");
    messages.finalize_thinking();

    assert!(messages.streaming_thinking.is_none());
    assert_eq!(messages.len(), 2);
    assert!(matches!(
        messages.items.last(),
        Some(HistoryItem::Thinking {
            is_collapsed: true,
            ..
        })
    ));
    assert_eq!(messages.active_thinking_duration, Duration::from_secs(1));
}

#[test]
fn thinking_summary_is_collapsed_and_reports_duration() {
    let item = HistoryItem::Thinking {
        content: "private reasoning".to_string(),
        timestamp: Instant::now(),
        turn: 1,
        is_collapsed: true,
    };

    let mut messages = MessagesComponent::new();
    messages.items.push(item);
    messages.active_thinking_index = Some(0);
    messages.active_thinking_duration = Duration::from_secs(17);

    let backend = TestBackend::new(80, 4);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| messages.render_thought_panel(frame, frame.area()))
        .expect("render should succeed");
    let buffer = terminal.backend().buffer();
    let rendered = (0..buffer.area.height)
        .flat_map(|y| (0..buffer.area.width).map(move |x| buffer[(x, y)].symbol().to_string()))
        .collect::<String>();

    assert!(rendered.contains("Thought for 17 seconds"));
    assert!(!rendered.contains("private reasoning"));
}

#[test]
fn active_thinking_toggles_between_collapsed_and_expanded() {
    let mut messages = MessagesComponent::new();
    messages.add_user("Hello".to_string());
    messages.update_thinking("private reasoning");
    messages.finalize_thinking();

    assert_eq!(messages.thought_panel_height(80, 12), 2);
    assert!(messages.toggle_active_thinking());
    assert!(messages.thought_panel_height(80, 12) > 2);
    assert!(messages.toggle_active_thinking());
    assert_eq!(messages.thought_panel_height(80, 12), 2);
}

#[test]
fn thinking_panel_is_not_duplicated_in_terminal_scrollback() {
    let mut messages = MessagesComponent::new();
    messages.add_user("Hello".to_string());
    messages.take_unrendered_lines(80);
    messages.update_thinking("private reasoning");
    messages.finalize_thinking();

    assert!(messages.take_unrendered_lines(80).is_empty());
}

#[test]
fn test_compression_item_pending() {
    let compression = CompressionItem::pending();
    assert!(compression.is_pending);
    assert_eq!(compression.status, CompressionStatus::Pending);
}

#[test]
fn test_compression_item_completed() {
    let compression = CompressionItem::completed(1000, 500);
    assert!(!compression.is_pending);
    assert_eq!(compression.status, CompressionStatus::Compressed);
    assert_eq!(compression.original_token_count, Some(1000));
    assert_eq!(compression.new_token_count, Some(500));
}

#[test]
fn test_compression_item_not_beneficial() {
    let compression = CompressionItem::not_beneficial(100);
    assert!(!compression.is_pending);
    assert_eq!(compression.status, CompressionStatus::NotBeneficial);
}

#[test]
fn test_compression_item_failed() {
    let compression = CompressionItem::failed("Test error".to_string());
    assert!(!compression.is_pending);
    assert_eq!(compression.status, CompressionStatus::Failed);
    assert_eq!(compression.error_message, Some("Test error".to_string()));
}

#[test]
fn test_compression_item_noop() {
    let compression = CompressionItem::noop();
    assert!(!compression.is_pending);
    assert_eq!(compression.status, CompressionStatus::Noop);
}

#[test]
fn test_start_compression() {
    let mut messages = MessagesComponent::new();
    messages.start_compression();

    assert!(messages.pending_compression.is_some());
    assert!(messages.is_compression_pending());
}

#[test]
fn test_complete_compression() {
    let mut messages = MessagesComponent::new();
    messages.start_compression();
    messages.complete_compression(1000, 500);

    // After completion, pending_compression is still set (showing result)
    // until the transition timeout (1.5 seconds in real time)
    assert!(messages.pending_compression.is_some());
    assert!(messages.compaction_animator.is_showing_result());

    // Verify the result is stored correctly
    let result = messages.compaction_animator.result().unwrap();
    assert_eq!(result.original_tokens, 1000);
    assert_eq!(result.new_tokens, 500);
}

#[test]
fn test_pending_compression_renders_animation() {
    let backend = TestBackend::new(80, 8);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut messages = MessagesComponent::new();
    messages.start_compression();
    messages.tick();

    terminal
        .draw(|frame| messages.render(frame, frame.area()))
        .expect("render should succeed");

    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert!(rendered.contains("Compacting context"));
    assert!(rendered.contains("%"));
}

#[test]
fn test_context_report_renders_sections_and_suggestions() {
    let mut messages = MessagesComponent::new();
    messages.add_context_report(
        crate::ui::components::ContextInfo {
            model_name: "claude-sonnet".to_string(),
            context_window_size: 200_000,
            system_prompt_tokens: 6_300,
            system_tools_tokens: 12_900,
            additional_tools_tokens: 1_200,
            memory_files_tokens: 886,
            conversation_tokens: 81_200,
            sections: vec![
                ContextSection {
                    title: "Tools".to_string(),
                    command_hint: Some("/help".to_string()),
                    groups: vec![
                        ContextSectionGroup {
                            title: Some("Core".to_string()),
                            entries: vec![ContextEntry {
                                label: "bash".to_string(),
                                tokens: 1_000,
                            }],
                        },
                        ContextSectionGroup {
                            title: Some("Additional".to_string()),
                            entries: vec![ContextEntry {
                                label: "search_docs".to_string(),
                                tokens: 200,
                            }],
                        },
                    ],
                },
                ContextSection {
                    title: "Skills Inventory".to_string(),
                    command_hint: Some("/skills".to_string()),
                    groups: vec![ContextSectionGroup {
                        title: Some("Project".to_string()),
                        entries: vec![ContextEntry {
                            label: "skills/review/SKILL.md".to_string(),
                            tokens: 130,
                        }],
                    }],
                },
            ],
            suggestions: vec![
                "Messages dominate the live window.".to_string(),
                "Skills and custom agents below are inventory only.".to_string(),
            ],
        },
        3,
    );

    let lines = messages.take_unrendered_lines(100);
    let rendered = lines
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("[3] /context"));
    assert!(rendered.contains("Additional tools"));
    assert!(rendered.contains("Skills Inventory"));
    assert!(rendered.contains("search_docs"));
    assert!(rendered.contains("Suggestions"));
    assert!(rendered.contains("inventory only"));
}

#[test]
fn test_get_compression_text_compressed() {
    let compression = CompressionItem::completed(1000, 500);
    let text = MessagesComponent::get_compression_text(&compression);

    assert!(text.contains("1000"));
    assert!(text.contains("500"));
    assert!(text.contains("50%")); // 500/1000 = 50% saved
}

#[test]
fn test_get_compression_text_failed() {
    let compression = CompressionItem::failed("Network error".to_string());
    let text = MessagesComponent::get_compression_text(&compression);

    assert!(text.contains("failed"));
    assert!(text.contains("Network error"));
}

#[test]
fn test_tool_tracking() {
    let mut messages = MessagesComponent::new();
    messages.add_user("Test".to_string());

    messages.start_tool("tool_1".to_string(), "bash".to_string(), None);
    assert!(messages.pending_tool_group.is_some());

    messages.complete_tool("tool_1", "output".to_string(), false, None);
}

#[test]
fn test_collapse_expand_tools() {
    let mut messages = MessagesComponent::new();
    messages.add_user("Test".to_string());
    messages.start_tool("t1".to_string(), "bash".to_string(), None);
    messages.complete_tool("t1", "output".to_string(), false, None);
    messages.finalize_pending_tool_group();

    // These should not panic
    messages.collapse_all_tools();
    messages.expand_all_tools();
}

#[test]
fn test_take_unrendered_lines_skips_streamed_assistant_after_tool_group() {
    let mut messages = MessagesComponent::new();
    messages.add_user("Test".to_string());
    messages.start_tool("t1".to_string(), "todo".to_string(), None);
    messages.complete_tool("t1", "[x] #1: Hello world".to_string(), false, None);

    messages.note_stream_chunk_rendered();
    messages.finalize_assistant("Tool handled".to_string());

    let tool_lines = messages.take_unrendered_lines(100);
    assert!(!tool_lines.is_empty());
    let rendered = tool_lines
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("todo"));
    assert!(!rendered.contains("Tool handled"));

    let assistant_lines = messages.take_unrendered_lines(100);
    assert!(assistant_lines.is_empty());
}

#[test]
fn test_take_unrendered_lines_prepends_dashboard_before_first_turn() {
    let mut messages = MessagesComponent::new();
    messages.add_user("Hello".to_string());

    let lines = messages.take_unrendered_lines(100);
    let rendered = lines
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Amadeus v0.1.0"));
    assert!(!rendered.contains("Tips for getting started"));
    assert!(!rendered.contains("/help"));
    assert!(rendered.contains("turn 1"));
    assert!(rendered.contains("Hello"));
}

#[test]
fn test_take_unrendered_lines_emits_startup_dashboard_once() {
    let mut messages = MessagesComponent::new();

    let first = messages.take_unrendered_lines(100);
    let first_rendered = first
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(first_rendered.contains("Amadeus v0.1.0"));

    let second = messages.take_unrendered_lines(100);
    assert!(second.is_empty());
}

#[test]
fn test_session_switch_replay_prepends_dashboard_before_existing_history() {
    let mut messages = MessagesComponent::new();
    messages.add_user("hello?".to_string());
    messages.add_assistant("Hello!".to_string());

    let initial_lines = messages.take_unrendered_lines(100);
    assert!(!initial_lines.is_empty());

    messages.reset_scrollback_cursor_for_session_switch();

    let replayed = messages.take_unrendered_lines(100);
    let rendered = replayed
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Amadeus v0.1.0"));
    assert!(rendered.contains("turn 1"));
    assert!(rendered.contains("hello?"));
    assert!(rendered.contains("Hello!"));
}

#[test]
fn test_flush_completed_pending_tool_group_moves_tool_group_into_history() {
    let mut messages = MessagesComponent::new();
    messages.add_user("Test".to_string());
    messages.start_tool("t1".to_string(), "todo".to_string(), None);

    assert!(!messages.has_completed_pending_tool_group());

    messages.complete_tool("t1", "[x] #1: Hello world".to_string(), false, None);

    assert!(messages.has_completed_pending_tool_group());
    assert!(messages.flush_completed_pending_tool_group());
    assert!(messages.pending_tool_group.is_none());
    assert!(matches!(
        messages.items.last(),
        Some(HistoryItem::ToolGroup { .. })
    ));
}

#[test]
fn test_history_item_turn_tracking() {
    let mut messages = MessagesComponent::new();

    messages.add_user("Q1".to_string());
    messages.add_assistant("A1".to_string());
    messages.add_user("Q2".to_string());
    messages.add_assistant("A2".to_string());

    // Check turn numbers
    match &messages.items[0] {
        HistoryItem::User { turn, .. } => assert_eq!(*turn, 1),
        _ => panic!("Expected user message"),
    }

    match &messages.items[1] {
        HistoryItem::Assistant { turn, .. } => assert_eq!(*turn, 1),
        _ => panic!("Expected assistant message"),
    }

    match &messages.items[2] {
        HistoryItem::User { turn, .. } => assert_eq!(*turn, 2),
        _ => panic!("Expected user message"),
    }
}

#[test]
fn test_messages_default() {
    let messages = MessagesComponent::default();
    assert!(messages.is_empty());
}
