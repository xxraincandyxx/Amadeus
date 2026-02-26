# Amadeus SDK Test Flow

## Test Structure

```
tests/
├── bash_test.rs       # Bash tool tests
├── config_test.rs     # Configuration tests
├── messages_test.rs   # Message type tests
├── agent_test.rs      # Agent loop tests
└── mock_llm.rs        # Mock LLM for testing
```

## Running Tests

```bash
# Run all tests
cargo test

# Run specific test file
cargo test --test bash_test

# Run with verbose output
cargo test -- --nocapture

# Run with specific feature
cargo test --features full
```

## Test Categories

### 1. Unit Tests

#### Tool Tests (bash_test.rs)

```rust
#[test]
fn test_bash_simple_command() {
    let tool = BashTool::new(Config::default());
    let input = json!({"command": "echo hello"});
    let result = tool.execute(input).await.unwrap();
    assert!(result["output"].as_str().unwrap().contains("hello"));
}

#[test]
fn test_bash_timeout() {
    let config = Config { timeout_secs: 1, ..Default::default() };
    let tool = BashTool::new(config);
    let input = json!({"command": "sleep 10"});
    let result = tool.execute(input).await;
    assert!(result.is_err());
}

#[test]
fn test_bash_working_directory() {
    let tool = BashTool::new(Config { workdir: "/tmp".into(), ..Default::default() });
    let input = json!({"command": "pwd"});
    let result = tool.execute(input).await.unwrap();
    assert!(result["output"].as_str().unwrap().contains("/tmp"));
}
```

#### Message Tests (messages_test.rs)

```rust
#[test]
fn test_message_creation() {
    let msg = Message::user("Hello");
    assert_eq!(msg.role, "user");
    assert!(matches!(msg.content[0], ContentBlock::Text { .. }));
}

#[test]
fn test_message_serialization() {
    let msg = Message::user("Hello");
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"role\":\"user\""));
}
```

#### Config Tests (config_test.rs)

```rust
#[test]
fn test_config_from_env() {
    std::env::set_var("ANTHROPIC_API_KEY", "test-key");
    let config = Config::load().unwrap();
    assert_eq!(config.api_key, "test-key");
}
```

### 2. Integration Tests

#### Agent Loop Tests (agent_test.rs)

```rust
#[tokio::test]
async fn test_agent_run_with_mock() {
    let mock_client = MockLLMClient::new();
    mock_client.set_response("Hello, I'm an AI assistant.");
    
    let agent = Agent::new(mock_client, Arc::new(Config::default()));
    let history = Arc::new(RwLock::new(Vec::new()));
    
    let result = agent.run("Hi", history).await.unwrap();
    assert!(result.text.contains("Hello"));
}

#[tokio::test]
async fn test_agent_tool_execution() {
    let mock_client = MockLLMClient::new();
    mock_client.set_tool_call("bash", json!({"command": "echo test"}));
    
    let agent = Agent::new(mock_client, Arc::new(Config::default()));
    let history = Arc::new(RwLock::new(Vec::new()));
    
    let result = agent.run("Run echo test", history).await.unwrap();
    assert!(!result.tool_calls.is_empty());
}
```

### 3. TUI Test Harness

```bash
# Run TUI for manual testing
cargo run --example tui --features tui

# Test scenarios:
# 1. Basic conversation
# 2. Tool execution (bash, file operations)
# 3. Long-running operations
# 4. Error handling
# 5. Streaming performance
```

### 4. HTTP Server Test Harness

```bash
# Start server
cargo run --example server --features api

# Test endpoints
curl -X POST http://localhost:3000/chat \
  -H "Content-Type: application/json" \
  -d '{"message": "Hello"}'

curl -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d '{"command": "ls -la"}'
```

## Mock LLM Testing

```rust
// tests/mock_llm.rs

use amadeus::client::LLMClient;
use amadeus::agent::messages::{Message, ContentBlock};

pub struct MockLLMClient {
    response: String,
    tool_calls: Vec<(String, serde_json::Value)>,
}

impl MockLLMClient {
    pub fn new() -> Self {
        Self {
            response: String::new(),
            tool_calls: Vec::new(),
        }
    }
    
    pub fn set_response(&mut self, text: &str) {
        self.response = text.to_string();
    }
    
    pub fn set_tool_call(&mut self, name: &str, input: serde_json::Value) {
        self.tool_calls.push((name.to_string(), input));
    }
}

#[async_trait]
impl LLMClient for MockLLMClient {
    async fn create_message(
        &self,
        _system: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>), amadeus::AgentError> {
        let mut content = Vec::new();
        
        if !self.response.is_empty() {
            content.push(ContentBlock::Text { text: self.response.clone() });
        }
        
        for (id, (name, input)) in self.tool_calls.iter().enumerate() {
            content.push(ContentBlock::ToolUse {
                id: format!("call_{}", id),
                name: name.clone(),
                input: input.clone(),
            });
        }
        
        Ok(("end_turn".to_string(), content))
    }
}
```

## CI/CD Test Flow

```yaml
# .github/workflows/test.yml

name: Test

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          
      - name: Run tests
        run: cargo test --all-features
        
      - name: Run clippy
        run: cargo clippy --all-features -- -D warnings
        
      - name: Check formatting
        run: cargo fmt -- --check
```

## Test Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Run coverage
cargo tarpaulin --all-features --out Html

# Open report
open tarpaulin-report.html
```

## Performance Testing

```bash
# Benchmark agent loop
cargo bench

# Profile with flamegraph
cargo install flamegraph
cargo flamegraph --example tui --features tui
```

## Pre-commit Checklist

```bash
#!/bin/sh
# Run before committing

# Format
cargo fmt

# Lint
cargo clippy --all-features -- -D warnings

# Test
cargo test --all-features

# Doc
cargo doc --no-deps
```

---

*Last updated: 2026-02-20*
