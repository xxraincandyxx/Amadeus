// @amadeus-header
// summary: Pluggable memory provider trait and registry for dynamic context loading.
// layer: core
// status: active
// feature_flags: none
// provides:
// - trait: crate::memory::MemoryProvider
// - type: crate::memory::MemoryEntry
// - type: crate::memory::MemoryRegistry
// uses:
// - module: amadeus_prompts::builder::PromptSection
// invariants:
// - Memory providers must be Send + Sync + Debug for use in multi-threaded runtime.
// side_effects:
// - Providers may read from filesystem, session state, or external services.
// tests:
// - cmd: cargo test -p context
// @end-amadeus-header

//! Pluggable memory providers for dynamic context injection.
//!
//! Memory providers supply entries that are injected into the system prompt
//! as dynamic sections. Built-in providers handle file-based project context
//! and session-based compaction summaries.

use std::fmt;
use std::sync::Arc;

/// A single piece of memory.
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    /// Unique key for deduplication (e.g. "project_context", "compaction_0").
    pub key: String,
    /// The memory content.
    pub content: String,
    /// Source identifier — "file", "session", "compaction", "user", "dynamic".
    pub source: String,
}

impl MemoryEntry {
    pub fn new(
        key: impl Into<String>,
        content: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            content: content.into(),
            source: source.into(),
        }
    }
}

/// Errors that can occur during memory operations.
#[derive(Debug, Clone)]
pub enum MemoryError {
    NotFound(String),
    WriteFailed(String),
    ClearFailed(String),
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(msg) => write!(f, "memory not found: {}", msg),
            Self::WriteFailed(msg) => write!(f, "memory write failed: {}", msg),
            Self::ClearFailed(msg) => write!(f, "memory clear failed: {}", msg),
        }
    }
}

/// A pluggable memory provider.
///
/// Implementations supply context that is injected into the system prompt
/// during agent construction or at runtime.
pub trait MemoryProvider: Send + Sync + fmt::Debug {
    /// Unique name of this provider (e.g. "file", "session").
    fn name(&self) -> &'static str;

    /// Load all memory entries from this provider.
    fn load(&self) -> Vec<MemoryEntry>;

    /// Store a new memory entry. Returns an error if the provider is read-only.
    fn store(&self, _entry: MemoryEntry) -> Result<(), MemoryError> {
        Err(MemoryError::WriteFailed(format!(
            "provider '{}' is read-only",
            self.name()
        )))
    }

    /// Whether this provider supports writes.
    fn writable(&self) -> bool {
        false
    }
}

/// A registry that manages multiple `MemoryProvider` instances and builds
/// a combined prompt section from all loaded entries.
#[derive(Debug, Clone, Default)]
pub struct MemoryRegistry {
    providers: Vec<Arc<dyn MemoryProvider>>,
}

impl MemoryRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a memory provider.
    pub fn register(&mut self, provider: Arc<dyn MemoryProvider>) {
        self.providers.push(provider);
    }

    /// List all registered providers.
    pub fn list_providers(&self) -> &[Arc<dyn MemoryProvider>] {
        &self.providers
    }

    /// Load all entries from all providers.
    pub fn load_all(&self) -> Vec<MemoryEntry> {
        let mut all = Vec::new();
        for provider in &self.providers {
            all.extend(provider.load());
        }
        all
    }

    /// Load entries from a specific provider by name.
    pub fn load_from(&self, provider_name: &str) -> Vec<MemoryEntry> {
        self.providers
            .iter()
            .find(|p| p.name() == provider_name)
            .map(|p| p.load())
            .unwrap_or_default()
    }

    /// Build the memory section content as a string.
    ///
    /// Returns `None` if no entries are loaded. The caller can wrap this in
    /// a [`PromptSection`] (from `amadeus_prompts`) for injection into the
    /// system prompt.
    pub fn build_memory_content(&self) -> Option<String> {
        let entries = self.load_all();
        if entries.is_empty() {
            return None;
        }

        Some(
            entries
                .iter()
                .map(|e| format!("## {}\n\n{}", e.key, e.content))
                .collect::<Vec<_>>()
                .join("\n\n"),
        )
    }

    /// Number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct TestProvider {
        name: &'static str,
        entries: Mutex<Vec<MemoryEntry>>,
        writable: bool,
    }

    impl MemoryProvider for TestProvider {
        fn name(&self) -> &'static str {
            self.name
        }

        fn load(&self) -> Vec<MemoryEntry> {
            self.entries.lock().unwrap().clone()
        }

        fn store(&self, entry: MemoryEntry) -> Result<(), MemoryError> {
            if !self.writable {
                return Err(MemoryError::WriteFailed("read-only".into()));
            }
            self.entries.lock().unwrap().push(entry);
            Ok(())
        }

        fn writable(&self) -> bool {
            self.writable
        }
    }

    #[test]
    fn test_empty_registry_returns_no_content() {
        let reg = MemoryRegistry::new();
        assert!(reg.build_memory_content().is_none());
    }

    #[test]
    fn test_registry_aggregates_entries() {
        let mut reg = MemoryRegistry::new();
        reg.register(Arc::new(TestProvider {
            name: "a",
            entries: Mutex::new(vec![MemoryEntry::new("k1", "v1", "test")]),
            writable: false,
        }));
        let content = reg.build_memory_content().unwrap();
        assert!(content.contains("v1"));
    }

    #[test]
    fn test_load_from_specific_provider() {
        let mut reg = MemoryRegistry::new();
        reg.register(Arc::new(TestProvider {
            name: "x",
            entries: Mutex::new(vec![MemoryEntry::new("kx", "vx", "test")]),
            writable: false,
        }));
        let entries = reg.load_from("x");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "kx");
        assert!(reg.load_from("nonexistent").is_empty());
    }
}
