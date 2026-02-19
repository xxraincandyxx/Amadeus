# Fix OpenAI Tool Result Message Format

## Problem

The OpenAI client sends tool results incorrectly. OpenAI's Chat Completions API requires tool results to be separate messages with `role: "tool"`, but the current implementation sends them as content blocks in a user message.

Current (incorrect) format:
```json
{"role": "user", "content": [{"type": "tool_result", "tool_use_id": "...", "content": "..."}]}
```

Required OpenAI format:
```json
{"role": "tool", "tool_call_id": "...", "content": "..."}
```

This causes API errors when the agent tries to continue the conversation after executing a tool.

## Solution

Rewrite `prepare_messages` in `src/client/openai.rs` (lines 145-198) to properly format messages for OpenAI:

1. **Tool results** → separate messages with `role: "tool"` and `tool_call_id`
2. **Tool uses (assistant)** → messages with `tool_calls` array
3. **Simple text** → standard messages with string content

## Implementation Details

### Changes to `prepare_messages` function:

```rust
pub fn prepare_messages(system: &str, messages: &[Message]) -> Vec<Value> {
    let mut result = vec![serde_json::json!({"role": "system", "content": system})];

    for msg in messages {
        let has_tool_results = msg.content.iter().any(|b| matches!(b, ContentBlock::ToolResult { .. }));
        
        if has_tool_results {
            // Each tool result becomes a separate message with role "tool"
            for block in &msg.content {
                if let ContentBlock::ToolResult { tool_use_id, content } = block {
                    result.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_use_id,
                        "content": content
                    }));
                }
            }
        } else {
            // Extract text content
            let text_content: Vec<&str> = msg.content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            
            // Extract tool calls (for assistant messages)
            let tool_calls: Vec<Value> = msg.content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolUse { id, name, input } => {
                        Some(serde_json::json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": serde_json::to_string(input).unwrap_or_default()
                            }
                        }))
                    }
                    _ => None,
                })
                .collect();

            if msg.role == "assistant" && !tool_calls.is_empty() {
                // Assistant message with tool calls
                let content_str = if text_content.is_empty() { 
                    Value::Null 
                } else { 
                    Value::String(text_content.join("")) 
                };
                result.push(serde_json::json!({
                    "role": "assistant",
                    "content": content_str,
                    "tool_calls": tool_calls
                }));
            } else if !text_content.is_empty() {
                // Simple text message
                result.push(serde_json::json!({
                    "role": msg.role,
                    "content": text_content.join("")
                }));
            }
        }
    }

    result
}
```

## Testing

After applying the fix:
1. Run `cargo run`
2. Send a message that triggers tool use (e.g., "list files")
3. Verify tool executes and response comes back without error
4. Continue conversation to ensure history is maintained correctly
