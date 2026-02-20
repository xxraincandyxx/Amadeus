//! # Amadeus - AI Agent SDK
//!
//! Run with `cargo run` for TUI mode, or `cargo run -- --server` for HTTP mode.

#[cfg(feature = "tui")]
use std::sync::Arc;

use anyhow::Result;

#[cfg(feature = "api")]
use amadeus::api::http::run_server;

#[cfg(feature = "tui")]
use amadeus::{
    agent::config::{Config, Provider},
    agent::loop_agent::Agent,
    client::anthropic::AnthropicClient,
    client::openai::OpenAIClient,
    ui::App,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    #[cfg(feature = "api")]
    if args.len() > 1 && args[1] == "--server" {
        let port = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3000);
        run_server(port).await?;
        return Ok(());
    }

    #[cfg(feature = "tui")]
    {
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
        return Ok(());
    }

    #[cfg(not(any(feature = "tui", feature = "api")))]
    {
        println!("Amadeus SDK - No features enabled.");
        println!("Enable 'tui' or 'api' feature to run.");
        println!("\nUsage:");
        println!("  cargo run --features tui          # TUI mode");
        println!("  cargo run --features api -- --server  # HTTP server mode");
        Ok(())
    }
}
