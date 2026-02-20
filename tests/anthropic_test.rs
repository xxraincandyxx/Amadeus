use claude_agent::agent::messages::{ContentBlock, Message};
use claude_agent::client::anthropic::AnthropicClient;
use claude_agent::client::LLMClient;
use claude_agent::error::AgentError;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn create_client_with_base_url(base_url: &str) -> AnthropicClient {
    AnthropicClient::new(
        "test-api-key".to_string(),
        Some(base_url.to_string()),
        "claude-sonnet-4-5-20250929".to_string(),
    )
}

#[test]
fn test_anthropic_client_new() {
    let client = AnthropicClient::new(
        "sk-ant-test".to_string(),
        None,
        "claude-sonnet-4-5-20250929".to_string(),
    );
    drop(client);
}

#[test]
fn test_anthropic_client_with_custom_base_url() {
    let client = AnthropicClient::new(
        "test-key".to_string(),
        Some("https://custom.api.com".to_string()),
        "claude-sonnet-4-5-20250929".to_string(),
    );
    drop(client);
}

#[tokio::test]
async fn test_anthropic_create_message_success() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-api-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "stop_reason": "end_turn",
            "content": [
                {"type": "text", "text": "Hello, world!"}
            ]
        })))
        .mount(&mock_server)
        .await;

    let messages = vec![Message::user("Hi")];
    let result = client
        .create_message("You are helpful", &messages, &[], 100)
        .await;

    assert!(result.is_ok());
    let (stop_reason, content) = result.unwrap();
    assert_eq!(stop_reason, "end_turn");
    assert_eq!(content.len(), 1);
}

#[tokio::test]
async fn test_anthropic_create_message_with_tool_use() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "stop_reason": "tool_use",
            "content": [
                {
                    "type": "tool_use",
                    "id": "tool_123",
                    "name": "bash",
                    "input": {"command": "ls"}
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    let messages = vec![Message::user("List files")];
    let result = client
        .create_message("You are helpful", &messages, &[], 100)
        .await;

    assert!(result.is_ok());
    let (stop_reason, content) = result.unwrap();
    assert_eq!(stop_reason, "tool_use");
    assert_eq!(content.len(), 1);

    match &content[0] {
        ContentBlock::ToolUse { name, .. } => assert_eq!(name, "bash"),
        _ => panic!("Expected ToolUse content block"),
    }
}

#[tokio::test]
async fn test_anthropic_create_message_api_error() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": {"message": "Invalid API key"}
        })))
        .mount(&mock_server)
        .await;

    let messages = vec![Message::user("Hi")];
    let result = client
        .create_message("You are helpful", &messages, &[], 100)
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, AgentError::InvalidResponse(_)));
}

#[tokio::test]
async fn test_anthropic_create_message_rate_limit() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({
            "error": {"message": "Rate limit exceeded"}
        })))
        .mount(&mock_server)
        .await;

    let messages = vec![Message::user("Hi")];
    let result = client
        .create_message("You are helpful", &messages, &[], 100)
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_anthropic_create_message_with_tools() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "stop_reason": "tool_use",
            "content": [
                {
                    "type": "tool_use",
                    "id": "tool_1",
                    "name": "bash",
                    "input": {"command": "echo test"}
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    let tools = vec![json!({
        "name": "bash",
        "description": "Run shell commands",
        "input_schema": {
            "type": "object",
            "properties": {
                "command": {"type": "string"}
            },
            "required": ["command"]
        }
    })];

    let messages = vec![Message::user("Run a command")];
    let result = client
        .create_message("You are helpful", &messages, &tools, 100)
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_anthropic_stream_text_delta() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    let sse_response = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\"}}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" world\"}}\n\n\
event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_response),
        )
        .mount(&mock_server)
        .await;

    let messages = vec![Message::user("Hi")];
    let stream = client
        .create_message_stream("You are helpful", &messages, &[], 100)
        .await;

    assert!(stream.is_ok());
}

#[tokio::test]
async fn test_anthropic_stream_tool_call() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    let sse_response = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\"}}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tool_123\",\"name\":\"bash\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"command\\\":\\\"ls\\\"}\"}}\n\n\
event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_response),
        )
        .mount(&mock_server)
        .await;

    let messages = vec![Message::user("List files")];
    let stream = client
        .create_message_stream("You are helpful", &messages, &[], 100)
        .await;

    assert!(stream.is_ok());
}

#[tokio::test]
async fn test_anthropic_stream_api_error() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "error": {"message": "Internal server error"}
        })))
        .mount(&mock_server)
        .await;

    let messages = vec![Message::user("Hi")];
    let result = client
        .create_message_stream("You are helpful", &messages, &[], 100)
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_anthropic_multiple_content_blocks() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "stop_reason": "end_turn",
            "content": [
                {"type": "text", "text": "Let me help you."},
                {
                    "type": "tool_use",
                    "id": "tool_1",
                    "name": "bash",
                    "input": {"command": "ls"}
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    let messages = vec![Message::user("List files")];
    let result = client
        .create_message("You are helpful", &messages, &[], 100)
        .await;

    assert!(result.is_ok());
    let (_, content) = result.unwrap();
    assert_eq!(content.len(), 2);
}

#[tokio::test]
async fn test_anthropic_empty_content() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "stop_reason": "end_turn",
            "content": []
        })))
        .mount(&mock_server)
        .await;

    let messages = vec![Message::user("Hi")];
    let result = client
        .create_message("You are helpful", &messages, &[], 100)
        .await;

    assert!(result.is_ok());
    let (_, content) = result.unwrap();
    assert!(content.is_empty());
}

#[test]
fn test_anthropic_client_clone() {
    let client = AnthropicClient::new(
        "test-key".to_string(),
        None,
        "claude-sonnet-4-5-20250929".to_string(),
    );

    let _cloned = client.clone();
}

#[tokio::test]
async fn test_anthropic_request_includes_all_fields() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-api-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "stop_reason": "end_turn",
            "content": [{"type": "text", "text": "ok"}]
        })))
        .mount(&mock_server)
        .await;

    let messages = vec![Message::user("Test")];
    let tools = vec![json!({"name": "test_tool"})];

    let result = client
        .create_message("System prompt", &messages, &tools, 500)
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_anthropic_server_error() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(503).set_body_json(json!({
            "error": {"message": "Service unavailable"}
        })))
        .mount(&mock_server)
        .await;

    let messages = vec![Message::user("Hi")];
    let result = client
        .create_message("You are helpful", &messages, &[], 100)
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_anthropic_overloaded_error() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(529).set_body_json(json!({
            "error": {"type": "overloaded_error", "message": "Overloaded"}
        })))
        .mount(&mock_server)
        .await;

    let messages = vec![Message::user("Hi")];
    let result = client
        .create_message("You are helpful", &messages, &[], 100)
        .await;

    assert!(result.is_err());
}
