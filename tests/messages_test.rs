use claude_agent::agent::messages::{ContentBlock, Message};
use serde_json::json;

#[test]
fn test_message_user_creation() {
    let msg = Message::user("Hello, world!");

    assert_eq!(msg.role, "user");
    assert_eq!(msg.content.len(), 1);

    if let ContentBlock::Text { text } = &msg.content[0] {
        assert_eq!(text, "Hello, world!");
    } else {
        panic!("Expected text content block");
    }
}

#[test]
fn test_message_assistant_creation() {
    let content = vec![ContentBlock::Text {
        text: "Hi there!".to_string(),
    }];
    let msg = Message::assistant(content);

    assert_eq!(msg.role, "assistant");
    assert_eq!(msg.content.len(), 1);
}

#[test]
fn test_message_multiple_content_blocks() {
    let content = vec![
        ContentBlock::Text {
            text: "I'll help you.".to_string(),
        },
        ContentBlock::ToolUse {
            id: "tool_123".to_string(),
            name: "bash".to_string(),
            input: json!({"command": "ls -la"}),
        },
    ];
    let msg = Message::assistant(content);

    assert_eq!(msg.content.len(), 2);
}

#[test]
fn test_content_block_serialization_text() {
    let block = ContentBlock::Text {
        text: "Hello".to_string(),
    };

    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["type"], "text");
    assert_eq!(json["text"], "Hello");
}

#[test]
fn test_content_block_serialization_tool_use() {
    let block = ContentBlock::ToolUse {
        id: "tool_123".to_string(),
        name: "bash".to_string(),
        input: json!({"command": "echo hello"}),
    };

    let json_val = serde_json::to_value(&block).unwrap();
    assert_eq!(json_val["type"], "tool_use");
    assert_eq!(json_val["id"], "tool_123");
    assert_eq!(json_val["name"], "bash");
    assert_eq!(json_val["input"]["command"], "echo hello");
}

#[test]
fn test_content_block_serialization_tool_result() {
    let block = ContentBlock::ToolResult {
        tool_use_id: "tool_123".to_string(),
        content: "output".to_string(),
    };

    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["type"], "tool_result");
    assert_eq!(json["tool_use_id"], "tool_123");
    assert_eq!(json["content"], "output");
}

#[test]
fn test_content_block_deserialization_text() {
    let json_val = json!({
        "type": "text",
        "text": "Hello, world!"
    });

    let block: ContentBlock = serde_json::from_value(json_val).unwrap();

    if let ContentBlock::Text { text } = block {
        assert_eq!(text, "Hello, world!");
    } else {
        panic!("Expected text content block");
    }
}

#[test]
fn test_content_block_deserialization_tool_use() {
    let json_val = json!({
        "type": "tool_use",
        "id": "tool_123",
        "name": "bash",
        "input": {
            "command": "ls -la"
        }
    });

    let block: ContentBlock = serde_json::from_value(json_val).unwrap();

    if let ContentBlock::ToolUse { id, name, input } = block {
        assert_eq!(id, "tool_123");
        assert_eq!(name, "bash");
        assert_eq!(input["command"], "ls -la");
    } else {
        panic!("Expected tool_use content block");
    }
}

#[test]
fn test_content_block_deserialization_tool_result() {
    let json_val = json!({
        "type": "tool_result",
        "tool_use_id": "tool_123",
        "content": "Command output"
    });

    let block: ContentBlock = serde_json::from_value(json_val).unwrap();

    if let ContentBlock::ToolResult {
        tool_use_id,
        content,
    } = block
    {
        assert_eq!(tool_use_id, "tool_123");
        assert_eq!(content, "Command output");
    } else {
        panic!("Expected tool_result content block");
    }
}

#[test]
fn test_message_serialization() {
    let msg = Message::user("Hello");

    let json = serde_json::to_value(&msg).unwrap();
    assert_eq!(json["role"], "user");
    assert!(json["content"].is_array());
}

#[test]
fn test_message_deserialization() {
    let json_val = json!({
        "role": "user",
        "content": [
            {
                "type": "text",
                "text": "Hello"
            }
        ]
    });

    let msg: Message = serde_json::from_value(json_val).unwrap();
    assert_eq!(msg.role, "user");
    assert_eq!(msg.content.len(), 1);
}

#[test]
fn test_message_with_empty_content() {
    let msg = Message::assistant(Vec::new());
    assert_eq!(msg.content.len(), 0);
}

#[test]
fn test_message_clone() {
    let msg = Message::user("Test");
    let cloned = msg.clone();
    assert_eq!(msg.role, cloned.role);
    assert_eq!(msg.content.len(), cloned.content.len());
}

#[test]
fn test_message_debug_format() {
    let msg = Message::user("Test");
    let debug_str = format!("{:?}", msg);
    assert!(debug_str.contains("Message"));
    assert!(debug_str.contains("user"));
}

#[test]
fn test_content_block_debug_format() {
    let block = ContentBlock::Text {
        text: "Test".to_string(),
    };
    let debug_str = format!("{:?}", block);
    assert!(debug_str.contains("Text"));
}

#[test]
fn test_round_trip_serialization_message() {
    let original = Message::user("Hello, world!");
    let json_val = serde_json::to_value(&original).unwrap();
    let deserialized: Message = serde_json::from_value(json_val).unwrap();

    assert_eq!(original.role, deserialized.role);
    assert_eq!(original.content.len(), deserialized.content.len());
}

#[test]
fn test_complex_message_serialization() {
    let content = vec![
        ContentBlock::Text {
            text: "First!".to_string(),
        },
        ContentBlock::ToolUse {
            id: "tool_1".to_string(),
            name: "bash".to_string(),
            input: json!({"command": "echo test"}),
        },
        ContentBlock::Text {
            text: "Second!".to_string(),
        },
    ];

    let msg = Message::assistant(content.clone());
    let json_val = serde_json::to_value(&msg).unwrap();
    let deserialized: Message = serde_json::from_value(json_val).unwrap();

    assert_eq!(deserialized.content.len(), 3);
}

#[test]
fn test_message_tool_results() {
    let results = vec![
        ContentBlock::ToolResult {
            tool_use_id: "tool_1".to_string(),
            content: "output 1".to_string(),
        },
        ContentBlock::ToolResult {
            tool_use_id: "tool_2".to_string(),
            content: "output 2".to_string(),
        },
    ];

    let msg = Message::tool_results(results);
    assert_eq!(msg.role, "user");
    assert_eq!(msg.content.len(), 2);
}

#[test]
fn test_tool_use_with_different_tools() {
    let bash_tool = ContentBlock::ToolUse {
        id: "tool_1".to_string(),
        name: "bash".to_string(),
        input: json!({"command": "ls"}),
    };

    let read_tool = ContentBlock::ToolUse {
        id: "tool_2".to_string(),
        name: "read_file".to_string(),
        input: json!({"path": "test.txt"}),
    };

    let write_tool = ContentBlock::ToolUse {
        id: "tool_3".to_string(),
        name: "write_file".to_string(),
        input: json!({"path": "output.txt", "content": "hello"}),
    };

    let edit_tool = ContentBlock::ToolUse {
        id: "tool_4".to_string(),
        name: "edit_file".to_string(),
        input: json!({"path": "file.txt", "old_text": "old", "new_text": "new", "replace_all": false}),
    };

    let msg = Message::assistant(vec![bash_tool, read_tool, write_tool, edit_tool]);
    assert_eq!(msg.content.len(), 4);
}
