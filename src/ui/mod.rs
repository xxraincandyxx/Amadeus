//! # UI Module
//!
//! Terminal user interface components.
//!
//! ## Components
//!
//! - **`colors`**: Color palette and formatted output helpers
//! - **`repl`**: Interactive Read-Eval-Print Loop
//!
//! ## Theme
//!
//! Uses a Dracula-inspired color palette for consistent styling.

/*
 * ============================================================================
 * MODULE DECLARATIONS
 * ============================================================================
 */

// The colors module - contains Palette and print helpers
// Looks for src/ui/colors.rs
pub mod colors;

// The repl module - contains the interactive REPL
// Looks for src/ui/repl.rs
pub mod repl;
