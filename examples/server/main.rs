//! # Server Example - SDK-as-a-Service
//!
//! This example demonstrates how to run Amadeus as a standalone HTTP server.

use anyhow::Result;
use std::sync::Arc;

use amadeus::{
    agent::config::{Config, Provider},
    agent::supervisor::{Supervisor, SupervisorConfig},
    agent::worker::WorkerConfig,
    api::http::run_server,
    client::anthropic::AnthropicClient,
    client::openai::OpenAIClient,
};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Arc::new(Config::load()?);
    let sdk_config = Arc::clone(&config);
    let port = 3000;

    match config.provider {
        Provider::Anthropic => {
            let client = AnthropicClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            let mut supervisor =
                Supervisor::new(client, SupervisorConfig::default(), sdk_config.clone());
            supervisor
                .spawn(vec![WorkerConfig::new("Main Coder").capability("bash")])
                .await?;
            let supervisor = Arc::new(supervisor);
            let s_clone = Arc::clone(&supervisor);
            tokio::spawn(async move {
                let _ = s_clone.run().await;
            });
            run_server(port, supervisor, sdk_config.clone()).await?;
        }
        Provider::OpenAI => {
            let client = OpenAIClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            let mut supervisor =
                Supervisor::new(client, SupervisorConfig::default(), sdk_config.clone());
            supervisor
                .spawn(vec![WorkerConfig::new("Main Coder").capability("bash")])
                .await?;
            let supervisor = Arc::new(supervisor);
            let s_clone = Arc::clone(&supervisor);
            tokio::spawn(async move {
                let _ = s_clone.run().await;
            });
            run_server(port, supervisor, sdk_config.clone()).await?;
        }
    }

    Ok(())
}
