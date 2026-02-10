use std::env;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::Result;

use claude_agent::{
    agent::config::{Config, Provider},
    client::anthropic::AnthropicClient,
    client::openai::OpenAIClient,
    agent::loop_agent::Agent,
    ui::colors::Palette,
    ui::repl::Repl,
};

/// Main entry point for the Claude AI agent.
///
/// This function is responsible for setting up the agent and running it.
/// If the first command line argument is provided, it runs in single-shot mode,
/// otherwise it runs in interactive mode.
///
/// # Single-shot mode
///
/// In single-shot mode, the agent is provided with a single command to execute.
/// It will execute the command and print the result to stdout.
///
/// # Interactive mode
///
/// In interactive mode, the agent runs an interactive REPL. The user is
/// presented with a prompt and can enter commands to execute.
///
/// The agent will execute the commands and print the results to stdout.
///
/// # Environment variables
///
/// The agent uses the following environment variables:
///
/// - `PROVIDER`: The AI provider to use. Can be either "anthropic" or "openai".
/// - `ANTHROPIC_API_KEY`: The API key for the Anthropic provider.
/// - `ANTHROPIC_BASE_URL`: The base URL for the Anthropic provider.
/// - `OPENAI_API_KEY`: The API key for the OpenAI provider.
/// - `OPENAI_BASE_URL`: The base URL for the OpenAI provider.
/// - `MODEL_ID`: The model to use for the AI provider.
/// - `USE_STREAMING`: Whether to use streaming responses.
/// - `TIMEOUT_SECONDS`: The timeout for commands in seconds.
#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        let config = Config::load()?;
        let history = Arc::new(RwLock::new(Vec::new()));
        let result = match config.provider {
            Provider::Anthropic => {
                let agent = Agent::new(
                    AnthropicClient::new(
                        config.api_key.clone(),
                        config.base_url.clone(),
                        config.model.clone(),
                    ),
                    config.workdir.to_string_lossy().to_string(),
                    config.timeout_seconds,
                    config.use_streaming,
                );
                agent.run(&args[1], Arc::clone(&history)).await?
            }
            Provider::OpenAI => {
                let agent = Agent::new(
                    OpenAIClient::new(
                        config.api_key.clone(),
                        config.base_url.clone(),
                        config.model.clone(),
                    ),
                    config.workdir.to_string_lossy().to_string(),
                    config.timeout_seconds,
                    config.use_streaming,
                );
                agent.run(&args[1], Arc::clone(&history)).await?
            }
        };
        println!("{}", result);
    } else {
        println!("{}", Palette::header());

        let config = Config::load()?;
        match config.provider {
            Provider::Anthropic => {
                let agent = Agent::new(
                    AnthropicClient::new(
                        config.api_key.clone(),
                        config.base_url.clone(),
                        config.model.clone(),
                    ),
                    config.workdir.to_string_lossy().to_string(),
                    config.timeout_seconds,
                    config.use_streaming,
                );
                Repl::new(agent).run().await?;
            }
            Provider::OpenAI => {
                let agent = Agent::new(
                    OpenAIClient::new(
                        config.api_key.clone(),
                        config.base_url.clone(),
                        config.model.clone(),
                    ),
                    config.workdir.to_string_lossy().to_string(),
                    config.timeout_seconds,
                    config.use_streaming,
                );
                Repl::new(agent).run().await?;
            }
        };
    }

    Ok(())
}
