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

// Arc for shared ownership
use std::sync::Arc;

// I/O types from standard library
// io: The I/O module
// Write: Trait for flushing output
use std::io::{self, Write};

// Async read-write lock
use tokio::sync::RwLock;

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

impl<C: LLMClient> Repl<C> {
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
        // INITIALIZE SHARED HISTORY
        // -----------------------------------------------------------------
        
        // Create shared history for the REPL session
        // History persists across multiple agent runs
        // 
        // Arc<RwLock<Vec<Message>>> means:
        // - Vec<Message>: The actual history data
        // - RwLock: Controls concurrent access
        // - Arc: Allows multiple owners (cheap to clone)
        let history = Arc::new(RwLock::new(Vec::new()));

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
            // print! doesn't add newline (unlike println!)
            print!("{}", Palette::prompt());
            
            // Flush stdout to ensure prompt appears immediately
            // Without flush, output might be buffered and not shown
            // 
            // io::stdout() returns a handle to standard output
            // .flush() forces buffered output to be written
            // ? propagates any I/O error
            io::stdout().flush()?;

            // -------------------------------------------------------------
            // READ USER INPUT
            // -------------------------------------------------------------
            
            // Create a mutable String to hold input
            let mut input = String::new();
            
            // Read a line from stdin
            // 
            // io::stdin() returns a handle to standard input
            // .read_line(&mut input) reads into the String
            // 
            // Returns Result<usize, io::Error>:
            // - Ok(n): Number of bytes read (0 means EOF)
            // - Err(e): I/O error
            match io::stdin().read_line(&mut input) {
                // Ok(0) means EOF (user pressed Ctrl+D)
                Ok(0) => break,
                
                // Ok(_) means we read something (ignore the count)
                Ok(_) => {}
                
                // Err means input failed (e.g., broken pipe)
                Err(e) => {
                    eprintln!("{}", Palette::error(&format!("Input error: {}", e)));
                    continue;  // Skip to next iteration
                }
            }

            // -------------------------------------------------------------
            // PROCESS INPUT
            // -------------------------------------------------------------
            
            // .trim() removes leading/trailing whitespace
            // Including the newline from read_line
            let input = input.trim();

            // Check for exit commands
            // Empty string or 'q' or 'exit' means quit
            if input.is_empty() || input == "q" || input == "exit" {
                break;
            }

            // -------------------------------------------------------------
            // RUN THE AGENT
            // -------------------------------------------------------------
            
            // Clone the Arc to pass to agent.run
            // Arc::clone increments the reference count (cheap)
            // We need to clone because we need history for future loops
            if let Err(e) = self.agent.run(input, Arc::clone(&history)).await {
                // Print error but don't exit
                eprintln!("{}", Palette::error(&format!("Agent error: {}", e)));
            }

            // Print blank line for readability
            println!();
        }

        // Print goodbye message
        println!("Goodbye!");
        
        Ok(())
    }
}
