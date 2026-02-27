//! # Amadeus - AI Agent SDK
//!
//! Run with `cargo run` for TUI mode, or `cargo run -- --server` for HTTP mode.

use amadeus::agent::config::{Config, Provider};
use amadeus::agent::supervisor::{Supervisor, SupervisorConfig};
use amadeus::agent::worker::WorkerConfig;
use amadeus::client::anthropic::AnthropicClient;
use amadeus::client::openai::OpenAIClient;
use anyhow::Result;
use std::sync::Arc;

#[cfg(feature = "api")]
use amadeus::api::http::run_server;

#[cfg(feature = "tui")]
use amadeus::agent::loop_agent::Agent;
#[cfg(feature = "tui")]
use amadeus::ui::App;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initial Setup
    let config = Arc::new(Config::load()?);
    let sdk_config = Arc::clone(&config);
    let args: Vec<String> = std::env::args().collect();

    // 2. Initialize Core LLM Client
    let provider = match config.provider {
        Provider::Anthropic => {
            let client = AnthropicClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            ClientKind::Anthropic(client)
        }
        Provider::OpenAI => {
            let client = OpenAIClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            ClientKind::OpenAI(client)
        }
    };

    // 3. Mode Selection

    // --- SERVER MODE ---
    #[cfg(feature = "api")]
    if args.contains(&"--server".to_string()) {
        let port = args
            .iter()
            .position(|r| r == "--server")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(3000);

        match provider {
            ClientKind::Anthropic(c) => {
                let mut supervisor =
                    Supervisor::new(c.clone(), SupervisorConfig::default(), sdk_config);
                supervisor
                    .spawn(vec![WorkerConfig::new("Main Coder").capability("bash")])
                    .await?;
                let supervisor = Arc::new(supervisor);
                let s_clone = Arc::clone(&supervisor);
                tokio::spawn(async move {
                    let _ = s_clone.run().await;
                });
                run_server(port, supervisor).await?;
            }
            ClientKind::OpenAI(c) => {
                let mut supervisor =
                    Supervisor::new(c.clone(), SupervisorConfig::default(), sdk_config);
                supervisor
                    .spawn(vec![WorkerConfig::new("Main Coder").capability("bash")])
                    .await?;
                let supervisor = Arc::new(supervisor);
                let s_clone = Arc::clone(&supervisor);
                tokio::spawn(async move {
                    let _ = s_clone.run().await;
                });
                run_server(port, supervisor).await?;
            }
        }
        return Ok(());
    }

    // --- TUI MODE ---
    #[cfg(feature = "tui")]
    {
        let workdir = config.workdir.clone();
        let model = config.model.clone();

        match provider {
            ClientKind::Anthropic(c) => {
                let agent = Agent::new(c, sdk_config);
                let mut app = App::new(agent, workdir, model);
                app.run().await?;
            }
            ClientKind::OpenAI(c) => {
                let agent = Agent::new(c, sdk_config);
                let mut app = App::new(agent, workdir, model);
                app.run().await?;
            }
        }
        return Ok(());
    }

    // --- NO FEATURE ENABLED ---
    #[allow(unreachable_code)]
    {
        println!("Amadeus SDK - No features enabled.");
        println!("Enable 'tui' or 'api' feature to run.");
        Ok(())
    }
}

enum ClientKind {
    Anthropic(AnthropicClient),
    OpenAI(OpenAIClient),
}
