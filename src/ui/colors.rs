//! # Color Palette
//!
//! Terminal color formatting using the `colored` crate.
//!
//! ## Theme (Dracula-inspired)
//!
//! - Purple: Prompts, commands, headers
//! - Magenta: Success indicators
//! - Red: Errors
//! - Cyan: Info messages

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

// The colored crate provides the Colorize trait
// * imports all extension methods (purple(), bold(), red(), etc.)
//
// This trait adds color methods to String and &str
// Example: "hello".red() returns a ColoredString
use colored::*;

/*
 * ============================================================================
 * PALETTE STRUCT
 * ============================================================================
 *
 * This is a "unit struct" - it has no fields.
 * It's used as a namespace for static methods.
 *
 * Why use a unit struct instead of just functions?
 * - Groups related functions together
 * - Can implement traits for it
 * - Can use in type parameters if needed
 */

/// Color palette for terminal output.
///
/// Provides static methods for formatting text with consistent colors.
pub struct Palette;

impl Palette {
    /// Get the header string (purple emoji).
    pub fn header() -> String {
        // "🎣" is a string literal
        // .purple() from Colorize trait - makes it purple
        // .bold() makes it bold
        // .to_string() converts to owned String
        //
        // Method chaining works because each method returns a colored string
        "🎣".purple().bold().to_string()
    }

    /// Get the prompt string.
    pub fn prompt() -> String {
        ">> ".purple().bold().to_string()
    }

    /// Format a command for display.
    pub fn command(cmd: &str) -> String {
        // format! creates a formatted String
        // Like println! but returns String instead of printing
        format!("$ {}", cmd).purple().to_string()
    }

    /// Get the tool result indicator.
    pub fn tool_result() -> String {
        // truecolor() sets a specific RGB color
        // 255, 0, 255 = bright magenta
        "✓".truecolor(255, 0, 255).to_string()
    }

    /// Format an error message.
    pub fn error(msg: &str) -> String {
        format!("✗ {}", msg).red().bold().to_string()
    }

    /// Format an info message.
    pub fn info(msg: &str) -> String {
        format!("ℹ {}", msg).cyan().to_string()
    }
}

/*
 * ============================================================================
 * HELPER FUNCTIONS
 * ============================================================================
 */

/// Print a command being executed.
///
/// Displays the command in purple with a `$` prefix.
pub fn print_command(cmd: &str) {
    // println! prints to stdout with a newline
    // Palette::command() returns the colored string
    println!("{}", Palette::command(cmd));
}

/// Print a tool execution result.
///
/// Handles large outputs by truncating to 50,000 characters.
/// Empty outputs show "(empty)".
pub fn print_tool_result(output: &str) {
    // Check for empty output
    if output.is_empty() {
        println!("(empty)");
        return;
    }

    // Truncate very large outputs to prevent terminal flooding
    let truncated = if output.len() > 50000 {
        // Slice the first 50,000 characters
        // &output[..50000] is string slicing
        // [..50000] means "from start to index 50000"
        &output[..50000]
    } else {
        // Output fits, use as-is
        // Note: this is a &str, not owned String
        output
    };

    println!("{}", truncated);
}
