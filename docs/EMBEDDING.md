# Embedding Amadeus in Rust

The provider-independent workflow kernel, workflow-backed agent registry, and schema-v2 architecture compiler are available from the root `amadeus` facade. HTTP sessions can execute ReAct, Plan-and-Execute, Reflection, Supervisor-Team, or user-edited architecture manifests while reusing the production model, tool, delegation, and event infrastructure. See [Agent Architectures](AGENT_ARCHITECTURES.md) and the [architecture guide](ARCHITECTURE.md#workflow-kernel) for the current boundary.

Add to your `Cargo.toml`:

```toml
[dependencies]
amadeus = { git = "https://github.com/xxraincandyxx/Amadeus", features = ["full"] }
tokio = { version = "1", features = ["full"] }
```

## Creating an agent

```rust
use amadeus::{Agent, Config, AnthropicClient};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Arc::new(Config::load()?);

    let client = AnthropicClient::new(
        config.api_key.clone(),
        config.base_url.clone(),
        config.model.clone(),
    );

    let agent = Agent::builder(client, config)
        .with_default_tools()
        .build();

    let result = agent.run("Create a hello world program in Rust").await?;
    println!("{}", result.text);

    Ok(())
}
```

## Custom tools

```rust
use amadeus::{Agent, Config, OpenAIClient, Tool};
use amadeus::error::AgentError;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

struct WeatherTool;

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &'static str {
        "get_weather"
    }

    fn schema(&self) -> &'static Value {
        &serde_json::json!({
            "name": "get_weather",
            "description": "Get the current weather for a location",
            "input_schema": {
                "type": "object",
                "properties": {
                    "location": { "type": "string" }
                },
                "required": ["location"]
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<String, AgentError> {
        let location = input["location"].as_str().unwrap_or("unknown");
        Ok(format!("Sunny, 72F in {location}"))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Arc::new(Config::load()?);
    let client = OpenAIClient::new(
        config.api_key.clone(),
        config.base_url.clone(),
        config.model.clone(),
    );

    let agent = Agent::builder(client, config)
        .with_default_tools()
        .register_tool(Box::new(WeatherTool))
        .build();

    let result = agent.run("What's the weather in Tokyo?").await?;
    println!("{}", result.text);

    Ok(())
}
```

## Event streaming

```rust
use amadeus::events::AgentEvent;

let mut stream = agent.run_stream();

while let Some(event) = stream.next().await {
    match event? {
        AgentEvent::TextDelta { delta } => print!("{}", delta),
        AgentEvent::ToolStart { id, name } => {
            println!("\n[Tool: {}]", name);
        }
        AgentEvent::ToolComplete { name, output, .. } => {
            println!("Output: {}", output);
        }
        AgentEvent::TokenUsage { total_tokens, .. } => {
            println!("\nTokens: {}", total_tokens);
        }
        AgentEvent::Done { result } => {
            println!("\nComplete!");
        }
        _ => {}
    }
}
```

## Policy and safety

```rust
use amadeus::policy::{Policy, ApprovalMode};
use std::sync::Arc;

// Auto: all tools execute without approval
let mut policy = Policy::new();
policy.set_mode(ApprovalMode::Auto);

// Ask: only dangerous operations require approval (opt-in — not the default path)
let mut policy = Policy::new();
policy.set_mode(ApprovalMode::Ask);

// Strict: all tools require approval except auto-approved ones
let mut policy = Policy::new();
policy.set_mode(ApprovalMode::Strict);

// Note: Policy is a secondary layer, only consulted when you explicitly attach
// it via `.with_policy(policy)`. The always-on gate is PermissionMode, set with
// `--permission-mode` (read-only | workspace-write | danger-full-access | prompt).
let agent = Agent::builder(client, config)
    .with_default_tools()
    .with_policy(Arc::new(policy))
    .build();
```

When attached, the policy system blocks dangerous patterns including `sudo`, `chmod 777`, `rm -rf /`, writing to `.env`/`.pem`/`.key` files, and shell pipes to `bash`/`sh`. Without an explicit `with_policy`, the `PermissionMode` gate still blocks dangerous commands via the `PermissionEnforcer`.
