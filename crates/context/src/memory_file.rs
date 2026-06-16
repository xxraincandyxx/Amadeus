// @amadeus-header
// summary: File-based memory provider that loads project context from .amadeus/context.md etc.
// layer: core
// status: active
// feature_flags: none
// provides:
// - type: crate::memory_file::FileMemoryProvider
// uses:
// - module: crate::ProjectContext
// - module: crate::memory::{MemoryProvider, MemoryEntry}
// invariants:
// - Load order is deterministic: .amadeus/context.md > .amadeus/CONTEXT.md > CONTEXT.md > context.md
// side_effects:
// - Reads filesystem state.
// tests:
// - cmd: cargo test -p context
// @end-amadeus-header

//! File-based memory provider.

use std::path::PathBuf;

use crate::memory::{MemoryEntry, MemoryProvider};
use crate::ProjectContext;

/// Loads project context from well-known files in the working directory.
///
/// Search order (first non-empty file wins):
/// 1. `.amadeus/context.md`
/// 2. `.amadeus/CONTEXT.md`
/// 3. `CONTEXT.md`
/// 4. `context.md`
#[derive(Debug, Clone)]
pub struct FileMemoryProvider {
    workdir: PathBuf,
}

impl FileMemoryProvider {
    pub fn new(workdir: PathBuf) -> Self {
        Self { workdir }
    }
}

impl MemoryProvider for FileMemoryProvider {
    fn name(&self) -> &'static str {
        "file"
    }

    fn load(&self) -> Vec<MemoryEntry> {
        ProjectContext::load(&self.workdir)
            .map(|ctx| MemoryEntry {
                key: "project_context".into(),
                content: ctx.content,
                source: "file".into(),
            })
            .into_iter()
            .collect()
    }

    fn writable(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_loads_from_amadeus_dir() {
        let temp = TempDir::new().unwrap();
        let amadeus_dir = temp.path().join(".amadeus");
        fs::create_dir_all(&amadeus_dir).unwrap();
        fs::write(amadeus_dir.join("context.md"), "test context").unwrap();

        let provider = FileMemoryProvider::new(temp.path().to_path_buf());
        let entries = provider.load();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "test context");
        assert_eq!(entries[0].source, "file");
    }

    #[test]
    fn test_empty_when_no_context_files() {
        let temp = TempDir::new().unwrap();
        let provider = FileMemoryProvider::new(temp.path().to_path_buf());
        assert!(provider.load().is_empty());
    }

    #[test]
    fn test_read_only() {
        let provider = FileMemoryProvider::new(PathBuf::from("."));
        assert!(!provider.writable());
    }
}
