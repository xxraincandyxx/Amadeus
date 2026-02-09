use std::env;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::Result;

use claude_agent::{
    agent::config::Config,
    client::anthropic::AnthropicClient,
    agent::loop_agent::Agent,
    ui::colors::Palette,
    ui::repl::Repl,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        // Subagent mode: execute task and print result
        let config = Config::load()?;
        let client = AnthropicClient::new(
            config.api_key.clone(),
            config.base_url.clone(),
            config.model.clone(),
        );

        let agent = Agent::new(
            client,
            config.workdir.to_string_lossy().to_string(),
            config.timeout_seconds,
        );

        let history = Arc::new(RwLock::new(Vec::new()));

        let result = agent.run(&args[1], Arc::clone(&history)).await?;
        println!("{}", result);
    } else {
        // Interactive REPL mode
        println!("{}", Palette::header());

        let config = Config::load()?;
        let client = AnthropicClient::new(
            config.api_key.clone(),
            config.base_url.clone(),
            config.model.clone(),
        );

        let agent = Agent::new(
            client,
            config.workdir.to_string_lossy().to_string(),
            config.timeout_seconds,
        );

        Repl::new(agent).run().await?;
    }

    Ok(())
}
