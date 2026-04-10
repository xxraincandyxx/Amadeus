// @amadeus-header
// summary: Integration tests covering agent test behavior.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::agent_test
// uses:
// - module: amadeus::agent::config::Config
// - module: amadeus::agent::loop_agent::Agent
// - module: amadeus::client::anthropic::AnthropicClient
// invariants:
// - Assertions stay aligned with current user-visible behavior.
// side_effects: none
// tests:
// - cmd: cargo test agent_test --features full
// @end-amadeus-header

use std::sync::Arc;

use amadeus::agent::config::Config;
use amadeus::agent::loop_agent::Agent;
use amadeus::client::anthropic::AnthropicClient;

fn create_test_config() -> Config {
    Config {
        api_key: "test-key".to_string(),
        model: "test-model".to_string(),
        workdir: std::path::PathBuf::from("/tmp"),
        timeout_seconds: 30,
        ..Config::default()
    }
}

#[tokio::test]
async fn test_agent_creation() {
    let client = AnthropicClient::new("test-key".to_string(), None, "test-model".to_string());
    let config = Arc::new(create_test_config());

    let _agent = Agent::new(client, config);
}

#[tokio::test]
async fn test_agent_different_workdir() {
    let client = AnthropicClient::new("test-key".to_string(), None, "test-model".to_string());
    let mut config = create_test_config();
    config.workdir = std::path::PathBuf::from("/home/user");

    let _agent = Agent::new(client, Arc::new(config));
}

#[tokio::test]
async fn test_agent_custom_timeout() {
    let client = AnthropicClient::new("test-key".to_string(), None, "test-model".to_string());
    let mut config = create_test_config();
    config.timeout_seconds = 60;

    let _agent = Agent::new(client, Arc::new(config));
}

#[tokio::test]
async fn test_agent_custom_max_output() {
    let client = AnthropicClient::new("test-key".to_string(), None, "test-model".to_string());
    let mut config = create_test_config();
    config.max_output_bytes = 100_000;

    let _agent = Agent::new(client, Arc::new(config));
}

#[tokio::test]
async fn test_agent_custom_blocked_commands() {
    let client = AnthropicClient::new("test-key".to_string(), None, "test-model".to_string());
    let mut config = create_test_config();
    config.blocked_commands = vec!["rm -rf /".to_string(), "sudo".to_string()];

    let _agent = Agent::new(client, Arc::new(config));
}
