use claude_agent::agent::messages::Message;
use claude_agent::client::openai::OpenAIClient;

#[test]
fn test_tool_transformation() {
    let anthropic_tool = serde_json::json!({
        "name": "bash",
        "description": "Execute shell command",
        "input_schema": {
            "type": "object",
            "properties": {"command": {"type": "string"}},
            "required": ["command"]
        }
    });

    let tools = vec![anthropic_tool];
    let openai_tools = OpenAIClient::transform_tools(&tools);

    assert_eq!(openai_tools.len(), 1);
    assert_eq!(openai_tools[0]["type"], "function");
    assert_eq!(openai_tools[0]["function"]["name"], "bash");
}

#[test]
fn test_message_transformation() {
    let system = "You are a helpful assistant";
    let messages = vec![
        Message::user("Hello"),
        Message::assistant(vec![claude_agent::agent::messages::ContentBlock::Text {
            text: "Hi there!".to_string(),
        }]),
    ];

    let openai_messages = OpenAIClient::prepare_messages(system, &messages);

    assert_eq!(openai_messages.len(), 3);
    assert_eq!(openai_messages[0]["role"], "system");
}
