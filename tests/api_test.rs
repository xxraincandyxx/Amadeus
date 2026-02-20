use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    routing::{get, post},
    Router,
};
use serde_json::json;
use tower::util::ServiceExt;

use claude_agent::api::handlers::{chat, execute, health};
use claude_agent::api::types::{ChatRequest, ExecuteRequest, HealthResponse};

fn create_test_router() -> Router<()> {
    Router::new()
        .route("/health", get(health::health))
        .route("/execute", post(execute::execute))
        .route("/chat", post(chat::chat))
}

#[tokio::test]
async fn test_health_endpoint() {
    let app = create_test_router();

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let health: HealthResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(health.status, "ok");
    assert!(!health.version.is_empty());
}

#[tokio::test]
async fn test_execute_endpoint_echo() {
    let app = create_test_router();

    let request = json!({
        "command": "echo hello"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/execute")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(result["exit_code"], 0);
    assert!(result["output"].as_str().unwrap().contains("hello"));
    assert_eq!(result["timed_out"], false);
}

#[tokio::test]
async fn test_execute_endpoint_with_timeout() {
    let app = create_test_router();

    let request = json!({
        "command": "echo test",
        "timeout_secs": 10
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/execute")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_execute_endpoint_blocked_command() {
    let app = create_test_router();

    let request = json!({
        "command": "rm -rf /"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/execute")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(result["exit_code"], -1);
    assert!(result["output"].as_str().unwrap().contains("blocked"));
}

#[tokio::test]
async fn test_execute_endpoint_missing_command() {
    let app = create_test_router();

    let request = json!({});

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/execute")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[test]
fn test_chat_request_deserialization() {
    let json = r#"{"message": "Hello, agent!"}"#;
    let request: ChatRequest = serde_json::from_str(json).unwrap();

    assert_eq!(request.message, "Hello, agent!");
    assert!(request.timeout_secs.is_none());
    assert!(request.stream.is_none());
}

#[test]
fn test_chat_request_with_options() {
    let json = r#"{"message": "Test", "timeout_secs": 60, "stream": true}"#;
    let request: ChatRequest = serde_json::from_str(json).unwrap();

    assert_eq!(request.message, "Test");
    assert_eq!(request.timeout_secs, Some(60));
    assert_eq!(request.stream, Some(true));
}

#[test]
fn test_execute_request_deserialization() {
    let json = r#"{"command": "ls -la", "timeout_secs": 30}"#;
    let request: ExecuteRequest = serde_json::from_str(json).unwrap();

    assert_eq!(request.command, "ls -la");
    assert_eq!(request.timeout_secs, Some(30));
}

#[test]
fn test_execute_request_defaults() {
    let json = r#"{"command": "pwd"}"#;
    let request: ExecuteRequest = serde_json::from_str(json).unwrap();

    assert_eq!(request.command, "pwd");
    assert!(request.timeout_secs.is_none());
}

#[tokio::test]
async fn test_execute_endpoint_pipeline() {
    let app = create_test_router();

    let request = json!({
        "command": "echo 'one two three' | grep two"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/execute")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(result["output"].as_str().unwrap().contains("two"));
}

#[tokio::test]
async fn test_execute_endpoint_stderr_capture() {
    let app = create_test_router();

    let request = json!({
        "command": "echo 'stdout' && echo 'stderr' >&2"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/execute")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let output = result["output"].as_str().unwrap();
    assert!(output.contains("stdout"));
    assert!(output.contains("stderr"));
}
