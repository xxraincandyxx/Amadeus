// @amadeus-header
// summary: Test utility code for fixtures.
// layer: infra
// status: active
// feature_flags:
// - test-utils
// provides:
// - module: crate::test_utils::fixtures
// - type: crate::test_utils::fixtures::FixtureLoader
// - type: crate::test_utils::fixtures::GoldenFileManager
// - type: crate::test_utils::fixtures::GoldenFile
// - type: crate::test_utils::fixtures::GoldenResponse
// uses:
// - protocol: serde serialization
// - artifact: filesystem paths and files
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Reads or writes filesystem state.
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

//! # Test Fixtures
//!
//! Utilities for loading and managing test fixtures.

use std::path::{Path, PathBuf};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Loader for test fixtures.
pub struct FixtureLoader {
    base_path: PathBuf,
}

impl FixtureLoader {
    /// Create a new fixture loader with the given base path.
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    /// Create a loader pointing to the default fixtures directory.
    pub fn default_loader() -> Self {
        Self::new(
            std::env::current_dir()
                .unwrap_or_default()
                .join("tests/fixtures"),
        )
    }

    /// Load a fixture as raw bytes.
    pub fn load_bytes(&self, name: &str) -> std::io::Result<Vec<u8>> {
        std::fs::read(self.base_path.join(name))
    }

    /// Load a fixture as a string.
    pub fn load_string(&self, name: &str) -> std::io::Result<String> {
        std::fs::read_to_string(self.base_path.join(name))
    }

    /// Load a JSON fixture.
    pub fn load_json<T: DeserializeOwned>(&self, name: &str) -> std::io::Result<T> {
        let content = self.load_string(name)?;
        serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Check if a fixture exists.
    pub fn exists(&self, name: &str) -> bool {
        self.base_path.join(name).exists()
    }

    /// Get the full path to a fixture.
    pub fn path(&self, name: &str) -> PathBuf {
        self.base_path.join(name)
    }

    /// List all fixtures in the directory.
    pub fn list(&self) -> std::io::Result<Vec<String>> {
        let mut fixtures = Vec::new();
        for entry in std::fs::read_dir(&self.base_path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                fixtures.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        Ok(fixtures)
    }
}

/// Golden file manager for API responses.
///
/// Supports record/replay pattern for API responses.
pub struct GoldenFileManager {
    base_path: PathBuf,
    record_mode: bool,
}

impl GoldenFileManager {
    /// Create a new golden file manager.
    pub fn new(base_path: impl AsRef<Path>, record_mode: bool) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
            record_mode,
        }
    }

    /// Create in replay mode (default).
    pub fn replay(base_path: impl AsRef<Path>) -> Self {
        Self::new(base_path, false)
    }

    /// Create in record mode.
    pub fn record(base_path: impl AsRef<Path>) -> Self {
        Self::new(base_path, true)
    }

    /// Load a golden file.
    pub fn load(&self, name: &str) -> std::io::Result<GoldenFile> {
        let path = self.base_path.join(name);
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let responses: Vec<GoldenResponse> = content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| serde_json::from_str(line).unwrap_or_default())
                .collect();
            Ok(GoldenFile { responses, path })
        } else {
            Ok(GoldenFile {
                responses: Vec::new(),
                path,
            })
        }
    }

    /// Save a golden file (only in record mode).
    pub fn save(&self, name: &str, responses: &[GoldenResponse]) -> std::io::Result<()> {
        if !self.record_mode {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Not in record mode",
            ));
        }

        std::fs::create_dir_all(&self.base_path)?;
        let path = self.base_path.join(name);
        let content = responses
            .iter()
            .map(|r| serde_json::to_string(r).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, content)
    }

    /// Check if we're in record mode.
    pub fn is_record_mode(&self) -> bool {
        self.record_mode
    }
}

/// A golden file containing recorded API responses.
#[derive(Debug, Clone)]
pub struct GoldenFile {
    pub responses: Vec<GoldenResponse>,
    pub path: PathBuf,
}

impl GoldenFile {
    /// Peek at the next response without consuming it.
    pub fn peek(&self) -> Option<&GoldenResponse> {
        self.responses.first()
    }

    /// Check if there are more responses.
    pub fn has_more(&self) -> bool {
        !self.responses.is_empty()
    }
}

impl Iterator for GoldenFile {
    type Item = GoldenResponse;

    fn next(&mut self) -> Option<Self::Item> {
        if self.responses.is_empty() {
            None
        } else {
            Some(self.responses.remove(0))
        }
    }
}

/// A single recorded API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenResponse {
    /// The method/endpoint that was called.
    pub method: String,
    /// The recorded response.
    pub response: serde_json::Value,
}

impl Default for GoldenResponse {
    fn default() -> Self {
        Self {
            method: String::new(),
            response: serde_json::Value::Null,
        }
    }
}
