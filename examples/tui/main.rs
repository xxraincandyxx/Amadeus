//! # TUI Example - SDK Test Harness
//!
//! This is a test harness for the Amadeus SDK.
//! It demonstrates SDK usage and tests performance.

use std::sync::Arc;
use anyhow::Result;

use amadeus::{
    agent::config::{Config, Provider},
    agent::loop_agent::Agent,
    client::anthropic::AnthropicClient,
    client::openai::OpenAIClient,
    ui::App,
};

#[tokio::main]
async fn main() -> Result<()> {
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
