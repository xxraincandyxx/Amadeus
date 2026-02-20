use claude_agent::agent::messages::{ContentBlock, Message};
use claude_agent::client::openai::OpenAIClient;
use claude_agent::client::LLMClient;
use claude_agent::error::AgentError;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn create_client_with_base_url(base_url: &str) -> OpenAIClient {
    OpenAIClient::new(
        "test-api-key".to_string(),
        Some(base_url.to_string()),
        "gpt-4".to_string(),
    )
}

#[test]
fn test_openai_client_new() {
    let client = OpenAIClient::new(
        "sk-test".to_string(),
        None,
        "gpt-4".to_string(),
    );
    drop(client);
}

#[test]
fn test_openai_client_with_custom_base_url() {
    let client = OpenAIClient::new(
        "test-key".to_string(),
        Some("https://custom.openai.com".to_string()),
        "gpt-4".to_string(),
    );
    drop(client);
}

#[tokio::test]
async fn test_openai_create_message_success() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer test-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "finish_reason": "stop",
                "message": {
                    "role": "assistant",
                    "content": "Hello, world!"
                }
            }]
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
async fn test_openai_create_message_with_tool_use() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_123",
                        "type": "function",
                        "function": {
                            "name": "bash",
                            "arguments": "{\"command\":\"ls\"}"
                        }
                    }]
                }
            }]
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
async fn test_openai_create_message_api_error() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
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
async fn test_openai_create_message_rate_limit() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
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
async fn test_openai_create_message_with_tools() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "bash",
                            "arguments": "{\"command\":\"echo test\"}"
                        }
                    }]
                }
            }]
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
async fn test_openai_stream_text_delta() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    let sse_response = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n\
data: [DONE]\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
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
async fn test_openai_stream_tool_call() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    let sse_response = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"id\":\"call_123\",\"function\":{\"name\":\"bash\"}}]}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"function\":{\"arguments\":\"{\\\"command\\\":\\\"ls\\\"}\"}}]}}]}\n\n\
data: [DONE]\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
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
async fn test_openai_stream_api_error() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
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
async fn test_openai_empty_content() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "finish_reason": "stop",
                "message": {
                    "role": "assistant",
                    "content": ""
                }
            }]
        })))
        .mount(&mock_server)
        .await;

    let messages = vec![Message::user("Hi")];
    let result = client
        .create_message("You are helpful", &messages, &[], 100)
        .await;

    assert!(result.is_ok());
    let (_, content) = result.unwrap();
    assert_eq!(content.len(), 1);
    match &content[0] {
        ContentBlock::Text { text } => assert_eq!(text, ""),
        _ => panic!("Expected Text content block"),
    }
}

#[test]
fn test_openai_transform_tools() {
    let tools = vec![json!({
        "name": "bash",
        "description": "Run shell commands",
        "input_schema": {
            "type": "object",
            "properties": {
                "command": {"type": "string"}
            }
        }
    })];

    let transformed = OpenAIClient::transform_tools(&tools);

    assert_eq!(transformed.len(), 1);
    assert_eq!(transformed[0]["type"], "function");
    assert_eq!(transformed[0]["function"]["name"], "bash");
    assert!(transformed[0]["function"]["parameters"].is_object());
}

#[test]
fn test_openai_transform_empty_tools() {
    let tools: Vec<serde_json::Value> = vec![];
    let transformed = OpenAIClient::transform_tools(&tools);
    assert!(transformed.is_empty());
}

#[test]
fn test_openai_prepare_messages() {
    let messages = vec![Message::user("Hello")];

    let prepared = OpenAIClient::prepare_messages("You are helpful", &messages);

    assert_eq!(prepared.len(), 2);
    assert_eq!(prepared[0]["role"], "system");
    assert_eq!(prepared[0]["content"], "You are helpful");
    assert_eq!(prepared[1]["role"], "user");
    assert_eq!(prepared[1]["content"], "Hello");
}

#[test]
fn test_openai_client_clone() {
    let client = OpenAIClient::new(
        "test-key".to_string(),
        None,
        "gpt-4".to_string(),
    );

    let _cloned = client.clone();
}

#[tokio::test]
async fn test_openai_finish_reason_length() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "finish_reason": "length",
                "message": {
                    "role": "assistant",
                    "content": "Truncated response"
                }
            }]
        })))
        .mount(&mock_server)
        .await;

    let messages = vec![Message::user("Write a long story")];
    let result = client
        .create_message("You are helpful", &messages, &[], 100)
        .await;

    assert!(result.is_ok());
    let (stop_reason, _) = result.unwrap();
    assert_eq!(stop_reason, "max_tokens");
}

#[tokio::test]
async fn test_openai_server_error() {
    let mock_server = MockServer::start().await;
    let client = create_client_with_base_url(&mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
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
