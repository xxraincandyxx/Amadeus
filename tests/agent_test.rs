// Agent loop tests
// Basic tests for agent creation and structure

mod mock_llm;

use claude_agent::agent::loop_agent::Agent;
use mock_llm::MockLLMClient;

#[tokio::test]
async fn test_agent_creation() {
    let mock_client = MockLLMClient::new();
    
    let _agent = Agent::new(
        mock_client,
        "/tmp".to_string(),
        30,
        false,
    );
    
    // Agent creation successful is the test assertion
}

#[tokio::test]
async fn test_agent_with_streaming_enabled() {
    let mock_client = MockLLMClient::new();
    
    let _agent = Agent::new(
        mock_client,
        "/tmp".to_string(),
        30,
        true, // streaming enabled
    );
    
    // Agent creation with streaming successful
}

#[tokio::test]
async fn test_agent_different_workdir() {
    let mock_client = MockLLMClient::new();
    
    let _agent = Agent::new(
        mock_client,
        "/home/user".to_string(),
        30,
        false,
    );
    
    // Agent with custom workdir successful
}

#[tokio::test]
async fn test_agent_custom_timeout() {
    let mock_client = MockLLMClient::new();
    
    let _agent = Agent::new(
        mock_client,
        "/tmp".to_string(),
        60, // custom timeout
        false,
    );
    
    // Agent with custom timeout successful
}
