//! # Claude Agent CLI Entry Point

use std::env;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use claude_agent::{
    agent::config::{Config, Provider},
    agent::loop_agent::Agent,
    api::http::run_server,
    client::anthropic::AnthropicClient,
    client::openai::OpenAIClient,
    ui::App,
};

fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

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
    let config = Arc::new(Config::load()?);
    let history = Arc::new(RwLock::new(Vec::new()));

    let result = match config.provider {
        Provider::Anthropic => {
            let client = AnthropicClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            let agent = Agent::new(client, Arc::clone(&config));
            agent.run(prompt, Arc::clone(&history)).await?
        }
        Provider::OpenAI => {
            let client = OpenAIClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            let agent = Agent::new(client, Arc::clone(&config));
            agent.run(prompt, Arc::clone(&history)).await?
        }
    };

    for tool_call in &result.tool_calls {
        if let Some(cmd) = tool_call.input.get("command").and_then(|v| v.as_str()) {
            println!("$ {}", cmd);
        }
        println!("{}", tool_call.output);
    }

    if !result.text.is_empty() {
        println!("{}", result.text);
    }

    Ok(())
}

async fn run_interactive() -> Result<()> {
    let config = Arc::new(Config::load()?);
    let workdir = config.workdir.clone();
    let model = config.model.clone();

    match config.provider {
        Provider::Anthropic => {
            let client = AnthropicClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            let agent = Agent::new(client, Arc::clone(&config));
            let mut app = App::new(agent, workdir, model);
            app.run().await?;
        }
        Provider::OpenAI => {
            let client = OpenAIClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            let agent = Agent::new(client, Arc::clone(&config));
            let mut app = App::new(agent, workdir, model);
            app.run().await?;
        }
    };

    Ok(())
}
