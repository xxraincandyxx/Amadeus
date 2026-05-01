// @amadeus-header
// summary: Project context loading and formatting shared across runtime surfaces.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - type: crate::ProjectContext
// - fn: crate::load_context_prompt
// uses:
// - artifact: filesystem paths and files
// invariants:
// - Context loading order stays deterministic across frontends.
// side_effects:
// - Reads filesystem state when context files exist.
// tests:
// - cmd: cargo test -p context
// @end-amadeus-header

//! Project context loading and memory providers.

pub mod memory;
pub mod memory_file;
pub mod memory_session;

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ProjectContext {
    pub content: String,
    pub source: PathBuf,
}

impl ProjectContext {
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

    pub fn to_prompt_section(&self) -> String {
        format!(
            "\n\n## Project Context\n\nThe following context has been provided by the project:\n\n{}",
            self.content
        )
    }
}

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
