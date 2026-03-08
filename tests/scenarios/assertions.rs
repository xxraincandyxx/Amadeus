#![allow(dead_code)]

use amadeus::agent::events::AgentEvent;

use super::timeline::EventTimeline;

// =============================================================================
// Legacy assertion functions (operate on raw Vec<AgentEvent>)
// =============================================================================

pub fn assert_events_contain_text(events: &[AgentEvent], expected: &str) {
    let text = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::TextDelta { delta } => Some(delta.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");

    assert!(
        text.contains(expected),
        "Expected text '{}' not found in events. Actual text: '{}'",
        expected,
        text
    );
}

pub fn assert_tool_call_count(events: &[AgentEvent], expected: usize) {
    let count = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::ToolStart { .. }))
        .count();

    assert_eq!(
        count, expected,
        "Expected {} tool calls, found {}",
        expected, count
    );
}

pub fn assert_tool_call_order(events: &[AgentEvent], expected_tools: &[&str]) {
    let actual_tools: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::ToolStart { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(actual_tools, expected_tools, "Tool call order mismatch");
}

pub fn assert_no_errors(events: &[AgentEvent]) {
    let errors: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::Error { message } => Some(message.clone()),
            _ => None,
        })
        .collect();

    assert!(errors.is_empty(), "Events contain errors: {:?}", errors);
}

pub fn assert_streaming_monotonic(events: &[AgentEvent]) {
    let mut found_done = false;

    for event in events {
        match event {
            AgentEvent::Done { .. } => found_done = true,
            AgentEvent::TextDelta { .. } | AgentEvent::ToolStart { .. } => {
                assert!(!found_done, "Found streaming event after Done marker");
            }
            _ => {}
        }
    }
}

pub fn assert_response_length(events: &[AgentEvent], min_chars: usize, max_chars: usize) {
    let text_len = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::TextDelta { delta } => Some(delta.len()),
            _ => None,
        })
        .sum::<usize>();

    assert!(
        text_len >= min_chars && text_len <= max_chars,
        "Response length {} not in range [{}, {}]",
        text_len,
        min_chars,
        max_chars
    );
}

pub fn assert_event_sequence(events: &[AgentEvent], expected_sequence: &[&str]) {
    let actual_sequence: Vec<String> = events
        .iter()
        .map(|e| match e {
            AgentEvent::TextDelta { .. } => "text".to_string(),
            AgentEvent::ThinkingDelta { .. } => "thinking".to_string(),
            AgentEvent::ThinkingComplete { .. } => "thinking_complete".to_string(),
            AgentEvent::ToolStart { name, .. } => format!("tool_start:{}", name),
            AgentEvent::ToolInputDelta { .. } => "tool_input".to_string(),
            AgentEvent::ToolOutputDelta { .. } => "tool_output".to_string(),
            AgentEvent::ToolComplete { name, .. } => format!("tool_complete:{}", name),
            AgentEvent::ApprovalRequired { request } => format!("approval:{}", request.tool),
            AgentEvent::TokenUsage { .. } => "token_usage".to_string(),
            AgentEvent::ToolProgress { .. } => "tool_progress".to_string(),
            AgentEvent::Compaction { .. } => "compaction".to_string(),
            AgentEvent::Done { .. } => "done".to_string(),
            AgentEvent::Error { .. } => "error".to_string(),
            AgentEvent::SessionSaved { .. } => "session_saved".to_string(),
        })
        .collect();

    assert_eq!(
        actual_sequence, expected_sequence,
        "Event sequence mismatch"
    );
}

pub fn assert_contains_approval_request(events: &[AgentEvent]) {
    let has_approval = events
        .iter()
        .any(|e| matches!(e, AgentEvent::ApprovalRequired { .. }));

    assert!(has_approval, "Expected approval request in events");
}

// =============================================================================
// Timeline-based assertions (richer, type-safe)
// =============================================================================

pub fn assert_timeline_text_contains(tl: &EventTimeline, expected: &str) {
    let text = tl.full_text();
    assert!(
        text.contains(expected),
        "Expected text '{}' not found. Actual: '{}'",
        expected,
        text
    );
}

pub fn assert_timeline_tool_count(tl: &EventTimeline, expected: usize) {
    let actual = tl.tool_count();
    assert_eq!(
        actual, expected,
        "Expected {} tool calls, found {}",
        expected, actual
    );
}

pub fn assert_timeline_tool_names(tl: &EventTimeline, expected: &[&str]) {
    let actual = tl.tool_names();
    let expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    assert_eq!(actual, expected, "Tool name mismatch");
}

pub fn assert_timeline_no_errors(tl: &EventTimeline) {
    let errors = tl.errors();
    assert!(errors.is_empty(), "Timeline contains errors: {:?}", errors);
}

pub fn assert_timeline_has_thinking(tl: &EventTimeline) {
    assert!(tl.has_thinking(), "Expected thinking content in timeline");
}

pub fn assert_timeline_thinking_contains(tl: &EventTimeline, expected: &str) {
    let thinking = tl.full_thinking();
    assert!(
        thinking.contains(expected),
        "Expected thinking '{}' not found. Actual: '{}'",
        expected,
        thinking
    );
}

pub fn assert_timeline_has_approval(tl: &EventTimeline) {
    assert!(
        tl.has_approval_requests(),
        "Expected approval request in timeline"
    );
}

pub fn assert_timeline_approval_for_tool(tl: &EventTimeline, tool_name: &str) {
    let approvals = tl.approval_requests();
    let found = approvals.iter().any(|a| a.tool == tool_name);
    assert!(
        found,
        "Expected approval for tool '{}', found approvals for: {:?}",
        tool_name,
        approvals.iter().map(|a| &a.tool).collect::<Vec<_>>()
    );
}

pub fn assert_timeline_has_token_usage(tl: &EventTimeline) {
    assert!(
        !tl.token_usage_events().is_empty(),
        "Expected token usage events in timeline"
    );
}

pub fn assert_timeline_had_compaction(tl: &EventTimeline) {
    assert!(tl.had_compaction(), "Expected compaction event in timeline");
}

pub fn assert_timeline_no_tool_errors(tl: &EventTimeline) {
    let errors = tl.tool_errors();
    assert!(
        errors.is_empty(),
        "Expected no tool errors, found: {:?}",
        errors
            .iter()
            .map(|e| format!("{}:{}", e.name, e.output))
            .collect::<Vec<_>>()
    );
}

pub fn assert_timeline_tool_output_contains(tl: &EventTimeline, tool_name: &str, expected: &str) {
    let completions = tl.tool_completions();
    let matching: Vec<_> = completions.iter().filter(|c| c.name == tool_name).collect();
    assert!(
        !matching.is_empty(),
        "No tool completions found for '{}'",
        tool_name
    );
    let found = matching.iter().any(|c| c.output.contains(expected));
    assert!(
        found,
        "Expected output containing '{}' for tool '{}', found: {:?}",
        expected,
        tool_name,
        matching.iter().map(|c| &c.output).collect::<Vec<_>>()
    );
}

pub fn assert_timeline_is_done(tl: &EventTimeline) {
    assert!(tl.is_done(), "Expected Done event in timeline");
}

pub fn assert_timeline_event_labels(tl: &EventTimeline, expected: &[&str]) {
    let actual = tl.event_labels();
    let expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    assert_eq!(actual, expected, "Event label sequence mismatch");
}

pub fn assert_timeline_history_len(tl: &EventTimeline, expected: usize) {
    let actual = tl.history_len();
    assert_eq!(
        actual, expected,
        "Expected history length {}, got {}",
        expected, actual
    );
}

pub fn assert_timeline_history_roles(tl: &EventTimeline, expected: &[&str]) {
    let actual = tl.history_roles();
    let expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    assert_eq!(actual, expected, "History role sequence mismatch");
}

pub fn assert_timeline_duration_at_least(tl: &EventTimeline, min_ms: u64) {
    let actual = tl.total_duration().as_millis() as u64;
    assert!(
        actual >= min_ms,
        "Expected duration >= {}ms, got {}ms",
        min_ms,
        actual
    );
}
