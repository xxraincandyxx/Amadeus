//! # Project Context
//!
//! Load project-specific context files that provide additional information
//! to the LLM about the codebase, conventions, and guidelines.
//!
//! ## Context Files
//!
//! The SDK searches for context files in the following order:
//! 1. `.amadeus/context.md` - Project-specific context
//! 2. `.amadeus/CONTEXT.md` - Alternative name
//! 3. `CONTEXT.md` - Root level context
//!
//! ## Usage
//!
//! ```rust,ignore
//! use amadeus::context::ProjectContext;
//!
//! if let Some(ctx) = ProjectContext::load(&workdir) {
//!     println!("Loaded context from: {:?}", ctx.source);
//!     println!("Content:\n{}", ctx.content);
//! }
//! ```

use std::path::{Path, PathBuf};

/// Represents a loaded project context.
#[derive(Debug, Clone)]
pub struct ProjectContext {
    /// The content of the context file.
    pub content: String,
    /// The path from which the context was loaded.
    pub source: PathBuf,
}

impl ProjectContext {
    /// Load project context from a working directory.
    ///
    /// Searches for context files in order:
    /// 1. `.amadeus/context.md`
    /// 2. `.amadeus/CONTEXT.md`
    /// 3. `CONTEXT.md`
    ///
    /// Returns `Some(ProjectContext)` if a context file is found and readable,
    /// or `None` if no context file exists.
    pub fn load(workdir: &Path) -> Option<Self> {
        let candidates = [
            workdir.join(".amadeus/context.md"),
            workdir.join(".amadeus/CONTEXT.md"),
            workdir.join("CONTEXT.md"),
            workdir.join("context.md"),
        ];

        for path in candidates {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if !content.trim().is_empty() {
                        tracing::info!(
                            path = %path.display(),
                            "Loaded project context"
                        );
                        return Some(Self {
                            content,
                            source: path,
                        });
                    }
                }
            }
        }

        None
    }

    /// Format the context for inclusion in a system prompt.
    ///
    /// Returns a formatted string that can be appended to the system prompt.
    pub fn to_prompt_section(&self) -> String {
        format!(
            "\n\n## Project Context\n\nThe following context has been provided by the project:\n\n{}",
            self.content
        )
    }
}

/// Load and format context for the system prompt.
///
/// This is a convenience function that loads context and formats it for
/// inclusion in a system prompt. Returns an empty string if no context exists.
pub fn load_context_prompt(workdir: &Path) -> String {
    ProjectContext::load(workdir)
        .map(|ctx| ctx.to_prompt_section())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_load_context_from_amadeus_dir() {
        let temp = TempDir::new().unwrap();
        let amadeus_dir = temp.path().join(".amadeus");
        fs::create_dir_all(&amadeus_dir).unwrap();

        let context_path = amadeus_dir.join("context.md");
        fs::write(&context_path, "This is project context.").unwrap();

        let ctx = ProjectContext::load(temp.path());
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.content, "This is project context.");
        assert_eq!(ctx.source, context_path);
    }

    #[test]
    fn test_load_context_fallback_to_root() {
        let temp = TempDir::new().unwrap();
        let context_path = temp.path().join("CONTEXT.md");
        fs::write(&context_path, "Root context.").unwrap();

        let ctx = ProjectContext::load(temp.path());
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.content, "Root context.");
    }

    #[test]
    fn test_no_context_returns_none() {
        let temp = TempDir::new().unwrap();
        let ctx = ProjectContext::load(temp.path());
        assert!(ctx.is_none());
    }

    #[test]
    fn test_empty_context_returns_none() {
        let temp = TempDir::new().unwrap();
        let amadeus_dir = temp.path().join(".amadeus");
        fs::create_dir_all(&amadeus_dir).unwrap();

        let context_path = amadeus_dir.join("context.md");
        fs::write(&context_path, "   \n  \n  ").unwrap();

        let ctx = ProjectContext::load(temp.path());
        assert!(ctx.is_none());
    }
}
