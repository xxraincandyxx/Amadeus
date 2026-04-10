// @amadeus-header
// summary: Runnable example for main usage.
// layer: example
// status: experimental
// feature_flags:
// - full
// provides:
// - module: example::server
// uses:
// - runtime: anyhow error handling
// - runtime: tokio task scheduling
// invariants:
// - Example code remains runnable against the current public API.
// side_effects:
// - Spawns asynchronous tasks.
// tests:
// - cmd: cargo run --example server --features full
// @end-amadeus-header

//! # Server Example - SDK-as-a-Service
//!
//! This example demonstrates how to run Amadeus as a standalone HTTP server.

use anyhow::Result;
use std::sync::Arc;

use amadeus::{
    agent::config::{Config, Provider},
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
            run_server(port, client, sdk_config.clone()).await?;
        }
        Provider::OpenAI => {
            let client = OpenAIClient::new(
                config.api_key.clone(),
                config.base_url.clone(),
                config.model.clone(),
            );
            run_server(port, client, sdk_config.clone()).await?;
        }
    }

    Ok(())
}
