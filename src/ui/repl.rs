//! # Interactive REPL
//!
//! Read-Eval-Print Loop for interactive agent usage.
//!
//! ## Features
//!
//! - Continuous prompt loop until exit
//! - Graceful error handling (continues on errors)
//! - Exit via 'q', 'exit', or Ctrl+D (EOF)

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

// I/O types from standard library
use std::io::{self, Write};

// Our agent type
use crate::agent::loop_agent::Agent;

// The LLM client trait (for generic bound)
use crate::client::LLMClient;

// Color palette for output
use crate::ui::colors::Palette;

/*
 * ============================================================================
 * REPL STRUCT
 * ============================================================================
 */

/// Interactive REPL for the agent.
///
/// Wraps an `Agent` and provides a continuous prompt loop
/// for interactive usage.
///
/// # Type Parameter
///
/// * `C` - The LLM client type (must implement `LLMClient`)
pub struct Repl<C: LLMClient> {
    /// The agent to run prompts through
    agent: Agent<C>,
}

impl<C: LLMClient + Clone + 'static> Repl<C> {
    /// Create a new REPL instance.
    pub fn new(agent: Agent<C>) -> Self {
        Self { agent }
    }

    /// Run the interactive REPL loop.
    ///
    /// Continuously prompts for input and runs the agent.
    /// Exits when user enters 'q', 'exit', or triggers EOF (Ctrl+D).
    pub async fn run(&self) -> Result<(), anyhow::Error> {
        // -----------------------------------------------------------------
        // PRINT WELCOME
        // -----------------------------------------------------------------

        // Print the header (fishing pole emoji)
        println!("{}", Palette::header());

        // Print usage instructions
        println!("Type 'q', 'exit', or press Ctrl+D to quit.\n");

        // -----------------------------------------------------------------
        // MAIN LOOP
        // -----------------------------------------------------------------

        loop {
            // Print the prompt (>> in purple)
            print!("{}", Palette::prompt());

            // Flush stdout to ensure prompt appears immediately
            io::stdout().flush()?;

            // -------------------------------------------------------------
            // READ USER INPUT
            // -------------------------------------------------------------

            let mut input = String::new();

            match io::stdin().read_line(&mut input) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) => {
                    eprintln!("{}", Palette::error(&format!("Input error: {}", e)));
                    continue;
                }
            }

            // -------------------------------------------------------------
            // PROCESS INPUT
            // -------------------------------------------------------------

            let input = input.trim();

            if input.is_empty() || input == "q" || input == "exit" {
                break;
            }

            // -------------------------------------------------------------
            // RUN THE AGENT
            // -------------------------------------------------------------

            if let Err(e) = self.agent.run(input).await {
                eprintln!("{}", Palette::error(&format!("Agent error: {}", e)));
            }

            println!();
        }

        println!("Goodbye!");

        Ok(())
    }
}
