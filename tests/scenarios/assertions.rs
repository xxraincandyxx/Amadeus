use amadeus::agent::events::AgentEvent;

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
            AgentEvent::ToolStart { name, .. } => format!("tool:{}", name),
            AgentEvent::ToolComplete { .. } => "tool_result".to_string(),
            AgentEvent::Done { .. } => "done".to_string(),
            AgentEvent::Error { .. } => "error".to_string(),
            _ => "other".to_string(),
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
