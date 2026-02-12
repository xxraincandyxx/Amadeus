//! # Bash Tool
//!
//! Execute shell commands with timeout support.
//!
//! ## Features
//!
//! - Async execution using `tokio::process::Command`
//! - Configurable timeout (returns `AgentError::Timeout`)
//! - Working directory support
//! - Combined stdout + stderr capture
//! - Concurrent execution of multiple commands
//!
//! ## Security Considerations
//!
//! The bash tool executes arbitrary shell commands. In production:
//! - Consider sandboxing (containers, namespaces)
//! - Validate/whitelist commands
//! - Limit resource usage (ulimit, cgroups)
//!
//! ## Example
//!
//! ```rust,ignore
//! use crate::tools::bash::BashTool;
//! use crate::agent::messages::ToolInput;
//!
//! let tool = BashTool::new(30, "/home/user/project".to_string());
//! let input = ToolInput { command: "ls -la".to_string() };
//!
//! let output = tool.execute(&input).await?;
//! println!("{}", output);
//! ```

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

// ToolInput struct - defines what the bash tool accepts as input
use crate::agent::messages::ToolInput;

// Our error types
use crate::error::{AgentError, Result};

// `join_all` - runs multiple futures concurrently and waits for all to complete
// This is like Promise.all() in JavaScript
use futures::future::join_all;

// Tokio's async version of std::process::Command
// Why use tokio's version?
// - std::process::Command is synchronous (blocks the thread)
// - tokio::process::Command is async (yields to other tasks while waiting)
use tokio::process::Command;

// Async timeout functionality
use tokio::time::{timeout, Duration};

/*
 * ============================================================================
 * BASH TOOL STRUCT
 * ============================================================================
 * 
 * A struct that holds the configuration for executing bash commands.
 * It's lightweight - just stores timeout and working directory.
 */

/// Tool for executing bash commands.
///
/// Commands are executed via `sh -c` in a subprocess, allowing
/// full shell syntax (pipes, redirects, etc.).
pub struct BashTool {
    /// Timeout in seconds for command execution
    /// 
    /// u64 = unsigned 64-bit integer
    /// Commands running longer than this are killed
    timeout_secs: u64,
    
    /// Working directory for commands
    /// 
    /// All commands execute in this directory
    /// Passed as String (not PathBuf) for simplicity
    workdir: String,
}

/*
 * ============================================================================
 * BASH TOOL IMPLEMENTATION
 * ============================================================================
 */

impl BashTool {
    // -------------------------------------------------------------------------
    // CONSTRUCTOR
    // -------------------------------------------------------------------------
    
    /// Create a new BashTool instance.
    ///
    /// # Arguments
    ///
    /// * `timeout_secs` - Maximum seconds before timing out
    /// * `workdir` - Directory to execute commands in
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let tool = BashTool::new(60, "/tmp".to_string());
    /// ```
    
    // This is a "associated function" (like a static method in other languages)
    // Called as: BashTool::new(30, "/tmp".to_string())
    // NOT: tool.new(...) - there's no instance yet!
    pub fn new(timeout_secs: u64, workdir: String) -> Self {
        // Create and return a new BashTool instance
        // 
        // "Field init shorthand" - when variable name matches field name,
        // you can write just the name instead of `timeout_secs: timeout_secs`
        Self {
            timeout_secs,  // Same as: timeout_secs: timeout_secs
            workdir,       // Same as: workdir: workdir
        }
    }

    // -------------------------------------------------------------------------
    // EXECUTE SINGLE COMMAND
    // -------------------------------------------------------------------------
    
    /// Execute a single command.
    ///
    /// # Arguments
    ///
    /// * `input` - The tool input containing the command string
    ///
    /// # Returns
    ///
    /// Combined stdout and stderr output on success, or an error.
    ///
    /// # Errors
    ///
    /// - `AgentError::Timeout`: Command exceeded timeout
    /// - `AgentError::Io`: Process execution failed
    pub async fn execute(&self, input: &ToolInput) -> Result<String> {
        // Delegate to the private method that handles the actual execution
        // 
        // Why have both execute() and execute_with_timeout()?
        // - execute() is the public API
        // - execute_with_timeout() is the implementation detail
        // - This separation allows for future flexibility
        self.execute_with_timeout(&input.command).await
    }

    // -------------------------------------------------------------------------
    // EXECUTE WITH TIMEOUT (INTERNAL)
    // -------------------------------------------------------------------------
    
    /// Execute a command with timeout enforcement.
    ///
    /// Uses `tokio::time::timeout` to enforce the time limit.
    /// If exceeded, returns `AgentError::Timeout`.
    ///
    /// # Arguments
    ///
    /// * `cmd` - The shell command to execute
    ///
    /// # Returns
    ///
    /// Combined stdout + stderr output.
    
    // `async fn` - This function is asynchronous
    // Returns a Future that must be .awaited
    async fn execute_with_timeout(&self, cmd: &str) -> Result<String> {
        // ---------------------------------------------------------------------
        // CREATE DURATION FOR TIMEOUT
        // ---------------------------------------------------------------------
        
        // Convert seconds to Duration type
        // Duration::from_secs() creates a Duration from seconds
        // 
        // Duration is used for time-based operations in Tokio
        let duration = Duration::from_secs(self.timeout_secs);

        // ---------------------------------------------------------------------
        // DEFINE THE ASYNC OPERATION
        // ---------------------------------------------------------------------
        
        // Create an async block (like a closure, but async)
        // This block contains the actual command execution
        // 
        // async { ... } creates a Future
        // The `output` variable holds this Future
        let output = async {
            // -----------------------------------------------------------------
            // SPAWN THE SHELL PROCESS
            // -----------------------------------------------------------------
            
            // Create a new Command to run the shell
            // 
            // Command::new("sh") - Use /bin/sh as the program
            // .arg("-c") - Pass the -c flag (read command from next argument)
            // .arg(cmd) - The actual command string to execute
            // 
            // This is equivalent to running: sh -c "your command"
            // Using sh -c allows for:
            // - Pipes: "cat file | grep pattern"
            // - Redirects: "echo hello > file.txt"
            // - Variables: "VAR=value; echo $VAR"
            // - Multiple commands: "cmd1 && cmd2 || cmd3"
            let result = Command::new("sh")
                .arg("-c")
                .arg(cmd)
                // Set the working directory for this command
                .current_dir(&self.workdir)
                // Execute and wait for completion
                // .output() captures both stdout and stderr
                // Returns Result<std::process::Output, io::Error>
                .output()
                // .await - Wait for the async operation to complete
                // This suspends this function until the process finishes
                // While waiting, other async tasks can run
                .await?;

            // -----------------------------------------------------------------
            // PROCESS THE OUTPUT
            // -----------------------------------------------------------------
            
            // result.stdout is Vec<u8> (raw bytes)
            // String::from_utf8_lossy() converts bytes to String
            // 
            // "Lossy" means: if bytes aren't valid UTF-8, they're replaced
            // with the Unicode replacement character (�) instead of crashing
            // 
            // This is important because:
            // - Shell output might not always be valid UTF-8
            // - Binary files might be printed to stdout
            // - Some programs output weird characters
            let stdout = String::from_utf8_lossy(&result.stdout).to_string();
            
            // Same for stderr
            let stderr = String::from_utf8_lossy(&result.stderr).to_string();

            // -----------------------------------------------------------------
            // COMBINE STDOUT AND STDERR
            // -----------------------------------------------------------------
            
            // Combine both streams into one string
            // stdout comes first, then stderr
            // 
            // format!("{}{}", a, b) concatenates two strings
            // Could also use: stdout + &stderr
            Ok(format!("{}{}", stdout, stderr))
        };

        // ---------------------------------------------------------------------
        // APPLY TIMEOUT
        // ---------------------------------------------------------------------
        
        // `timeout(duration, future)` wraps a future with a timeout
        // 
        // Returns: Result<F, Elapsed>
        // - Ok(result) if the future completed within the duration
        // - Err(Elapsed) if the timeout expired
        // 
        // This is how we enforce the time limit on commands
        match timeout(duration, output).await {
            // Future completed within timeout
            // result is the inner Result<String, AgentError> from output
            Ok(result) => result,
            
            // Timeout expired
            // _ ignores the Elapsed error (we don't need its details)
            Err(_) => Err(AgentError::Timeout(self.timeout_secs)),
        }
    }

    // -------------------------------------------------------------------------
    // CONCURRENT EXECUTION
    // -------------------------------------------------------------------------
    
    /// Execute multiple commands concurrently.
    ///
    /// Commands are executed in parallel using `futures::future::join_all`.
    /// Each command has its own timeout enforced independently.
    ///
    /// # Arguments
    ///
    /// * `inputs` - Vector of tool inputs to execute
    ///
    /// # Returns
    ///
    /// Vector of results (preserves input order).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let inputs = vec![
    ///     ToolInput { command: "echo a".to_string() },
    ///     ToolInput { command: "echo b".to_string() },
    /// ];
    /// let results = tool.execute_all(inputs).await;
    /// assert_eq!(results.len(), 2);
    /// ```
    
    // Takes `inputs: Vec<ToolInput>` by value (takes ownership)
    pub async fn execute_all(&self, inputs: Vec<ToolInput>) -> Vec<Result<String>> {
        // ---------------------------------------------------------------------
        // CREATE FUTURES FOR EACH INPUT
        // ---------------------------------------------------------------------
        
        // Transform each ToolInput into a Future
        // 
        // .into_iter() - consumes the Vec and gives an iterator that owns items
        // .map() - transforms each item
        // .collect::<Vec<_>>() - collects back into a Vec
        // 
        // The <Vec<_>> is a type annotation with inference
        // _ means "infer the element type"
        let futures = inputs
            .into_iter()
            // For each input, create a Future that executes it
            .map(|input| {
                // Clone the command string (needed because we move `input`)
                let cmd = input.command.clone();
                
                // Create a NEW BashTool instance for this execution
                // Why create a new one?
                // - To capture `self.timeout_secs` and `self.workdir`
                // - Because async blocks can't borrow from `self` easily
                //   (self might be dropped before the async completes)
                // 
                // This is cheap - BashTool is just two small fields
                let tool = BashTool::new(self.timeout_secs, self.workdir.clone());
                
                // Create an async block (the Future)
                // `move` keyword: move captured variables into the async block
                // Without `move`, variables would be borrowed (reference)
                // With `move`, variables are moved (ownership transferred)
                // 
                // We need `move` because:
                // - `cmd` and `tool` must live as long as the Future
                // - The Future might outlive this function call
                async move {
                    // Execute the command
                    tool.execute_with_timeout(&cmd).await
                }
            })
            // Collect the Futures into a Vec
            // At this point, we have Vec<impl Future<Output = Result<String>>>
            .collect::<Vec<_>>();

        // ---------------------------------------------------------------------
        // EXECUTE ALL FUTURES CONCURRENTLY
        // ---------------------------------------------------------------------
        
        // `join_all(futures)` runs all futures concurrently
        // 
        // Unlike sequential execution:
        //   for f in futures { f.await }  // One at a time
        // 
        // join_all starts all futures immediately and waits for all:
        //   join_all(futures).await  // All at once
        // 
        // Returns: Vec<F::Output> where F::Output is Result<String>
        // 
        // The order of results matches the order of input futures
        join_all(futures).await
    }
}
