//! # File System Test Helpers
//!
//! Utilities for creating and managing test file systems.

use std::collections::HashMap;
use std::path::Path;

use tempfile::TempDir;
use tokio::fs;

/// Declarative file system structure for tests.
#[derive(Debug, Clone)]
pub enum FileSystemStructure {
    /// A file with content
    File { name: String, content: String },
    /// A directory with children
    Dir {
        name: String,
        children: Vec<FileSystemStructure>,
    },
}

impl FileSystemStructure {
    /// Create a file entry.
    pub fn file(name: &str, content: &str) -> Self {
        Self::File {
            name: name.to_string(),
            content: content.to_string(),
        }
    }

    /// Create a directory entry.
    pub fn dir(name: &str, children: Vec<Self>) -> Self {
        Self::Dir {
            name: name.to_string(),
            children,
        }
    }

    /// Create the file system structure at the given path.
    pub fn create_at<'a>(
        &'a self,
        base_path: &'a Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + 'a>> {
        Box::pin(async move {
            match self {
                Self::File { name, content } => {
                    let file_path = base_path.join(name);
                    fs::write(&file_path, content).await?;
                }
                Self::Dir { name, children } => {
                    let dir_path = base_path.join(name);
                    fs::create_dir_all(&dir_path).await?;
                    for child in children {
                        child.create_at(&dir_path).await?;
                    }
                }
            }
            Ok(())
        })
    }

    /// Get the name of this entry.
    pub fn name(&self) -> &str {
        match self {
            Self::File { name, .. } => name,
            Self::Dir { name, .. } => name,
        }
    }

    /// Count total files in this structure.
    pub fn file_count(&self) -> usize {
        match self {
            Self::File { .. } => 1,
            Self::Dir { children, .. } => children.iter().map(|c| c.file_count()).sum(),
        }
    }

    /// Count total directories in this structure.
    pub fn dir_count(&self) -> usize {
        match self {
            Self::File { .. } => 0,
            Self::Dir { children, .. } => 1 + children.iter().map(|c| c.dir_count()).sum::<usize>(),
        }
    }
}

/// Helper for creating temporary test directories with structure.
pub struct TmpDirBuilder {
    prefix: String,
    structure: Option<FileSystemStructure>,
}

impl TmpDirBuilder {
    /// Create a new builder with a prefix.
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
            structure: None,
        }
    }

    /// Set the file system structure.
    pub fn with_structure(mut self, structure: FileSystemStructure) -> Self {
        self.structure = Some(structure);
        self
    }

    /// Build the temporary directory.
    pub async fn build(self) -> std::io::Result<TmpDirWithStructure> {
        let temp_dir = tempfile::Builder::new().prefix(&self.prefix).tempdir()?;

        if let Some(structure) = &self.structure {
            structure.create_at(temp_dir.path()).await?;
        }

        Ok(TmpDirWithStructure {
            temp_dir,
            _structure: self.structure,
        })
    }
}

/// Temporary directory with known structure.
pub struct TmpDirWithStructure {
    temp_dir: TempDir,
    _structure: Option<FileSystemStructure>,
}

impl TmpDirWithStructure {
    /// Get the path to the temporary directory.
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Get a file path within the directory.
    pub fn file_path(&self, relative: &str) -> std::path::PathBuf {
        self.temp_dir.path().join(relative)
    }

    /// Read a file's contents.
    pub async fn read_file(&self, relative: &str) -> std::io::Result<String> {
        fs::read_to_string(self.file_path(relative)).await
    }

    /// Write to a file.
    pub async fn write_file(&self, relative: &str, content: &str) -> std::io::Result<()> {
        let path = self.file_path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&path, content).await
    }

    /// Check if a file exists.
    pub async fn file_exists(&self, relative: &str) -> bool {
        fs::try_exists(self.file_path(relative))
            .await
            .unwrap_or(false)
    }

    /// List all files in the directory (non-recursive, just top level).
    pub async fn list_top_level_files(&self) -> std::io::Result<Vec<String>> {
        let mut files = Vec::new();
        let mut entries = fs::read_dir(self.temp_dir.path()).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type().await?.is_file() {
                files.push(name);
            }
        }
        Ok(files)
    }

    /// Cleanup and consume the directory.
    pub fn cleanup(self) {
        drop(self);
    }
}

impl Drop for TmpDirWithStructure {
    fn drop(&mut self) {
        // TempDir automatically cleans up
    }
}

/// Create a simple temporary directory with files.
pub async fn create_tmp_dir(
    files: &HashMap<String, String>,
) -> std::io::Result<TmpDirWithStructure> {
    let mut builder = TmpDirBuilder::new("test");

    // Convert flat file map to directory structure
    let mut root_children: Vec<FileSystemStructure> = Vec::new();

    for (path, content) in files {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() == 1 {
            root_children.push(FileSystemStructure::file(parts[0], content));
        } else {
            // For nested paths, just create file with full relative path
            root_children.push(FileSystemStructure::file(path, content));
        }
    }

    builder = builder.with_structure(FileSystemStructure::dir("root", root_children));
    builder.build().await
}

/// Compare two files for equality.
pub async fn files_equal(path1: &Path, path2: &Path) -> std::io::Result<bool> {
    let content1 = fs::read(path1).await?;
    let content2 = fs::read(path2).await?;
    Ok(content1 == content2)
}

/// Compare file contents with a string.
pub async fn file_content_equals(path: &Path, expected: &str) -> std::io::Result<bool> {
    let content = fs::read_to_string(path).await?;
    Ok(content == expected)
}

/// Assert that a file contains specific text.
pub async fn assert_file_contains(path: &Path, text: &str) -> std::io::Result<()> {
    let content = fs::read_to_string(path).await?;
    assert!(
        content.contains(text),
        "File {:?} does not contain {:?}\nActual content:\n{}",
        path,
        text,
        content
    );
    Ok(())
}

/// Create a sample project structure for testing.
pub fn sample_rust_project() -> FileSystemStructure {
    FileSystemStructure::dir(
        "project",
        vec![
            FileSystemStructure::file(
                "Cargo.toml",
                r#"
[package]
name = "sample"
version = "0.0.0"
edition = "2021"

[dependencies]
"#,
            ),
            FileSystemStructure::dir(
                "src",
                vec![FileSystemStructure::file(
                    "main.rs",
                    r#"
fn main() {
    println!("Hello, world!");
}
"#,
                )],
            ),
        ],
    )
}
