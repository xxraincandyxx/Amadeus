//! # Claude Agent CLI Entry Point
//!
//! Binary entry point for the Claude AI coding agent.
//! Supports three modes: HTTP server, single-shot, and interactive REPL.
//!
//! # Modes
//!
//! | Mode | Command | Description |
//! |------|---------|-------------|
//! | Server | `--server [port]` | Start HTTP REST API |
//! | Single-shot | `"prompt"` | Run once with prompt |
//! | Interactive | (no args) | Start interactive REPL |

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

// Standard library imports
// env: Access to environment variables and command-line arguments
use std::env;

// Arc: Atomic Reference Counting for shared ownership
// Used to share history between the main function and agent
use std::sync::Arc;

// RwLock: Reader-Writer Lock for controlled access to shared data
// The tokio version is async-safe (doesn't block the thread)
use tokio::sync::RwLock;

// anyhow::Result: A convenient Result type that can hold any error
// We use this in main() because it's more flexible than our custom Result
use anyhow::Result;

// Imports from our crate (claude_agent)
// These come from src/lib.rs exports
use claude_agent::{
    // Config: Loads configuration from environment
    // Provider: Enum for Anthropic vs OpenAI
    agent::config::{Config, Provider},
    // The generic agent type
    agent::loop_agent::Agent,
    // HTTP server for REST API
    api::http::run_server,
    // Anthropic-specific client implementation
    client::anthropic::AnthropicClient,
    // OpenAI-specific client implementation
    client::openai::OpenAIClient,
    // Color palette for terminal output
    ui::colors::Palette,
    // Interactive REPL
    ui::repl::Repl,
};

/*
 * ============================================================================
 * MAIN ENTRY POINT
 * ============================================================================
 */

/// Main entry point for the Claude AI agent.
///
/// This function is responsible for setting up the agent and running it.
/// The mode is determined by command-line arguments:
///
/// - `--server [port]`: Start HTTP server mode
/// - `"prompt"`: Single-shot mode with the prompt
/// - No args: Interactive REPL mode
///
/// # HTTP Server Mode
///
/// Starts an HTTP REST API server:
///
/// ```bash
/// cargo run -- --server
/// cargo run -- --server 8080
/// ```
///
/// # Single-shot Mode
///
/// In single-shot mode, the agent is provided with a single command to execute.
/// It will execute the command and print the result to stdout.
///
/// ```bash
/// cargo run -- "list all rust files in src/"
/// ```
///
/// # Interactive Mode
///
/// In interactive mode, the agent runs an interactive REPL. The user is
/// presented with a prompt and can enter commands to execute.
///
/// ```bash
/// cargo run
/// ```
///
/// # Environment Variables
///
/// The agent uses the following environment variables:
///
/// - `PROVIDER`: The AI provider to use. Can be either "anthropic" or "openai".
/// - `ANTHROPIC_API_KEY`: The API key for the Anthropic provider.
/// - `ANTHROPIC_BASE_URL`: The base URL for the Anthropic provider (optional).
/// - `OPENAI_API_KEY`: The API key for the OpenAI provider.
/// - `OPENAI_BASE_URL`: The base URL for the OpenAI provider (optional).
/// - `MODEL_ID`: The model to use for the AI provider.
/// - `USE_STREAMING`: Whether to use streaming responses.
//
// #[tokio::main] is an attribute macro from the tokio crate
// It transforms async fn main() into a synchronous main() that:
// 1. Creates a Tokio runtime
// 2. Runs the async function inside that runtime
//
// Without this macro, async fn main() wouldn't compile because
// the entry point must be synchronous
#[tokio::main]
async fn main() -> Result<()> {
    // -----------------------------------------------------------------
    // PARSE COMMAND LINE ARGUMENTS
    // -----------------------------------------------------------------

    // env::args() returns an iterator over command-line arguments
    // .collect() gathers them into a Vec<String>
    //
    // args will contain:
    // - args[0]: The program name (e.g., "claude-agent" or "target/debug/claude-agent")
    // - args[1..]: User-provided arguments
    //
    // Example: cargo run -- "hello world"
    //   args = ["target/debug/claude-agent", "hello world"]
    let args: Vec<String> = env::args().collect();

    // -----------------------------------------------------------------
    // DETERMINE MODE FROM ARGUMENTS
    // -----------------------------------------------------------------

    // Check if user provided an argument
    if args.len() > 1 {
        match args[1].as_str() {
            // ---------------------------------------------------------
            // HTTP SERVER MODE
            // ---------------------------------------------------------
            "--server" => {
                // Parse optional port number
                // args.get(2) returns Option<&String>
                // .and_then() chains Option operations
                // .parse().ok() converts string to number, returns None on error
                let port = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3000);

                // Run the HTTP server
                // This runs forever until Ctrl+C
                run_server(port).await?;

                Ok(())
            }

            // ---------------------------------------------------------
            // SINGLE-SHOT MODE
            // ---------------------------------------------------------
            _ => {
                // Treat first argument as a prompt
                // The trailing .await: means we wait for the async operation to complete
                run_single_shot(&args[1]).await
            }
        }
    } else {
        // -------------------------------------------------------------
        // INTERACTIVE MODE
        // -------------------------------------------------------------
        // No arguments: start the REPL
        run_interactive().await
    }
}

/*
 * ============================================================================
 * SINGLE-SHOT MODE
 * ============================================================================
 */

/// Run the agent in single-shot mode.
///
/// Takes a prompt string, creates the appropriate client based on config,
/// runs the agent once, and prints the result.
///
/// # Arguments
///
/// * `prompt` - The user's prompt to send to the agent
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if configuration loading,
/// agent creation, or execution fails.
async fn run_single_shot(prompt: &str) -> Result<()> {
    // -----------------------------------------------------------------
    // LOAD CONFIGURATION
    // -----------------------------------------------------------------

    // Config::load() reads environment variables and builds a Config
    // The ? operator propagates errors up to the caller
    //
    // This loads from .env file (if present) and environment variables
    let config = Config::load()?;

    // -----------------------------------------------------------------
    // CREATE SHARED HISTORY
    // -----------------------------------------------------------------

    // Create empty history for this run
    // In single-shot mode, there's no persistent history
    // But we still need Arc<RwLock> because agent.run() expects it
    //
    // Arc::new() creates a new Arc-wrapped value
    // RwLock::new() creates a new RwLock-wrapped value
    // Vec::new() creates an empty vector
    let history = Arc::new(RwLock::new(Vec::new()));

    // -----------------------------------------------------------------
    // CREATE CLIENT AND RUN AGENT
    // -----------------------------------------------------------------

    // Match on the provider to create the appropriate client
    // config.provider is the Provider enum (Anthropic or OpenAI)
    let result = match config.provider {
        // -------------------------------------------------------------
        // ANTHROPIC PROVIDER
        // -------------------------------------------------------------
        Provider::Anthropic => {
            // Create an Anthropic client
            let agent = Agent::new(
                // Create the client
                AnthropicClient::new(
                    // Clone values because we need to use config multiple times
                    // (though in this branch we only use it once, the pattern
                    // is consistent with the OpenAI branch)
                    config.api_key.clone(),
                    config.base_url.clone(),
                    config.model.clone(),
                ),
                // Working directory as string
                // to_string_lossy() converts PathBuf to String
                // It replaces invalid UTF-8 with replacement characters
                config.workdir.to_string_lossy().to_string(),
                // Timeout from config
                config.timeout_seconds,
                // Streaming flag from config
                config.use_streaming,
            );

            // Run the agent with the prompt
            // Arc::clone() increments reference count (cheap)
            // .await waits for the async operation
            // ? propagates errors
            agent.run(prompt, Arc::clone(&history)).await?
        }

        // -------------------------------------------------------------
        // OPENAI PROVIDER
        // -------------------------------------------------------------
        Provider::OpenAI => {
            // Same pattern as Anthropic, but with OpenAIClient
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
            agent.run(prompt, Arc::clone(&history)).await?
        }
    };

    // -----------------------------------------------------------------
    // PRINT RESULT
    // -----------------------------------------------------------------

    // Print the agent's response to stdout
    println!("{}", result);

    Ok(())
}

/*
 * ============================================================================
 * INTERACTIVE MODE
 * ============================================================================
 */

/// Run the agent in interactive REPL mode.
///
/// Displays a header, loads configuration, and starts the interactive
/// read-eval-print loop where users can enter multiple prompts.
///
/// # Returns
///
/// Returns `Ok(())` when the user exits gracefully, or an error if
/// configuration loading or REPL initialization fails.
async fn run_interactive() -> Result<()> {
    // -----------------------------------------------------------------
    // PRINT HEADER
    // -----------------------------------------------------------------

    // Print the fancy header (fishing pole emoji in purple)
    // In Rust, the exclamation mark is used as part of a macro invocation, such
    // as println!, vec!, or format!
    println!("{}", Palette::header());

    // -----------------------------------------------------------------
    // LOAD CONFIGURATION
    // -----------------------------------------------------------------

    // Load config same as single-shot mode
    let config = Config::load()?;

    // -----------------------------------------------------------------
    // CREATE CLIENT AND START REPL
    // -----------------------------------------------------------------

    // Match on provider to create appropriate client and REPL
    match config.provider {
        // -------------------------------------------------------------
        // ANTHROPIC PROVIDER
        // -------------------------------------------------------------
        Provider::Anthropic => {
            // Create the agent with Anthropic client
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

            // Create and run the REPL
            // Repl::new() wraps the agent
            // .run() starts the interactive loop
            // .await waits for the REPL to finish
            // ? propagates errors
            Repl::new(agent).run().await?;
        }

        // -------------------------------------------------------------
        // OPENAI PROVIDER
        // -------------------------------------------------------------
        Provider::OpenAI => {
            // Same pattern with OpenAI client
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

    Ok(())
}
