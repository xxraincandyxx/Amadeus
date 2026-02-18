//! # Claude Agent CLI Entry Point

use std::env;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;

use claude_agent::{
    agent::config::{Config, Provider},
    agent::loop_agent::Agent,
    api::http::run_server,
    client::anthropic::AnthropicClient,
    client::openai::OpenAIClient,
    ui::colors::Palette,
    ui::repl::Repl,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "--server" => {
                let port = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3000);
                run_server(port).await?;
                Ok(())
            }
            _ => run_single_shot(&args[1]).await,
        }
    } else {
        run_interactive().await
    }
}

async fn run_single_shot(prompt: &str) -> Result<()> {
    let config = Config::load()?;
    let history = Arc::new(RwLock::new(Vec::new()));

    let result = match config.provider {
        Provider::Anthropic => {
            let client = AnthropicClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            let agent = Agent::new(client, &config);
            agent.run(prompt, Arc::clone(&history)).await?
        }
        Provider::OpenAI => {
            let client = OpenAIClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            let agent = Agent::new(client, &config);
            agent.run(prompt, Arc::clone(&history)).await?
        }
    };

    println!("{}", result);
    Ok(())
}

async fn run_interactive() -> Result<()> {
    println!("{}", Palette::header());

    let config = Config::load()?;

    match config.provider {
        Provider::Anthropic => {
            let client = AnthropicClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            let agent = Agent::new(client, &config);
            Repl::new(agent).run().await?;
        }
        Provider::OpenAI => {
            let client = OpenAIClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            let agent = Agent::new(client, &config);
            Repl::new(agent).run().await?;
        }
    };

    Ok(())
}
