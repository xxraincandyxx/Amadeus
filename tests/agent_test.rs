// Agent loop tests
// Basic tests for agent creation and structure

use claude_agent::agent::loop_agent::Agent;
use claude_agent::client::anthropic::AnthropicClient;
use claude_agent::client::LLMClient;

#[tokio::test]
async fn test_agent_creation() {
    let client = AnthropicClient::new(
        "test-key".to_string(),
        None,
        "test-model".to_string()
    );
    
    let _agent = Agent::new(
        client,
        "/tmp".to_string(),
        30,
        false,
    );
    
    // Agent creation successful is the test assertion
}

#[tokio::test]
async fn test_agent_with_streaming_enabled() {
    let client = AnthropicClient::new(
        "test-key".to_string(),
        None,
        "test-model".to_string()
    );
    
    let _agent = Agent::new(
        client,
        "/tmp".to_string(),
        30,
        true, // streaming enabled
    );
    
    // Agent creation with streaming successful
}

#[tokio::test]
async fn test_agent_different_workdir() {
    let client = AnthropicClient::new(
        "test-key".to_string(),
        None,
        "test-model".to_string()
    );
    
    let _agent = Agent::new(
        client,
        "/home/user".to_string(),
        30,
        false,
    );
    
    // Agent with custom workdir successful
}

#[tokio::test]
async fn test_agent_custom_timeout() {
    let client = AnthropicClient::new(
        "test-key".to_string(),
        None,
        "test-model".to_string()
    );
    
    let _agent = Agent::new(
        client,
        "/tmp".to_string(),
        60, // custom timeout
        false,
    );
    
    // Agent with custom timeout successful
}
