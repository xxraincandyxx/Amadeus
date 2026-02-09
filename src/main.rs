use std::env;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::Result;

use claude_agent::{
    agent::config::{Config, Provider},
    client::LLMClient,
    client::anthropic::AnthropicClient,
    client::openai::OpenAIClient,
    agent::loop_agent::Agent,
    ui::colors::Palette,
    ui::repl::Repl,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        let config = Config::load()?;

        let agent = match config.provider {
            Provider::Anthropic => Agent::new(
                AnthropicClient::new(
                    config.api_key.clone(),
                    config.base_url.clone(),
                    config.model.clone(),
                ),
                config.workdir.to_string_lossy().to_string(),
                config.timeout_seconds,
                config.use_streaming,
            ),
            Provider::OpenAI => Agent::new(
                OpenAIClient::new(
                    config.api_key.clone(),
                    config.base_url.clone(),
                    config.model.clone(),
                ),
                config.workdir.to_string_lossy().to_string(),
                config.timeout_seconds,
                config.use_streaming,
            ),
        };

        let history = Arc::new(RwLock::new(Vec::new()));
        let result = agent.run(&args[1], Arc::clone(&history)).await?;
        println!("{}", result);
    } else {
        println!("{}", Palette::header());

        let config = Config::load()?;

        let agent = match config.provider {
            Provider::Anthropic => Agent::new(
                AnthropicClient::new(
                    config.api_key.clone(),
                    config.base_url.clone(),
                    config.model.clone(),
                ),
                config.workdir.to_string_lossy().to_string(),
                config.timeout_seconds,
                config.use_streaming,
            ),
            Provider::OpenAI => Agent::new(
                OpenAIClient::new(
                    config.api_key.clone(),
                    config.base_url.clone(),
                    config.model.clone(),
                ),
                config.workdir.to_string_lossy().to_string(),
                config.timeout_seconds,
                config.use_streaming,
            ),
        };

        Repl::new(agent).run().await?;
    }

    Ok(())
}
