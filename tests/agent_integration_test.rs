//! # Agent Integration Tests
//!
//! Integration tests for the agent using patterns inspired by gemini-cli.
//!
//! ## Test Structure
//!
//! Tests follow the arrange-act-assert pattern with mock LLM responses.

use std::collections::HashMap;

use amadeus::test_utils::{FileSystemStructure, FixtureLoader, TmpDirBuilder, TmpDirWithStructure};

/// Test that the file system structure utilities work correctly.
#[tokio::test]
async fn test_file_system_structure_creation() {
    let structure = FileSystemStructure::dir(
        "project",
        vec![
            FileSystemStructure::file("README.md", "# Test Project"),
            FileSystemStructure::dir(
                "src",
                vec![FileSystemStructure::file("main.rs", "fn main() {}")],
            ),
        ],
    );

    // Verify structure counts
    assert_eq!(structure.file_count(), 2);
    assert_eq!(structure.dir_count(), 2); // project + src

    // Create the structure in a temp directory
    let tmp_dir: TmpDirWithStructure = TmpDirBuilder::new("test_fs")
        .with_structure(structure)
        .build()
        .await
        .expect("Failed to create temp dir");

    // Verify files exist
    assert!(tmp_dir.file_exists("project/README.md").await);
    assert!(tmp_dir.file_exists("project/src/main.rs").await);

    // Verify content
    let content = tmp_dir
        .read_file("project/README.md")
        .await
        .expect("Failed to read");
    assert!(content.contains("# Test Project"));
}

/// Test the fixture loader with sample files.
#[tokio::test]
async fn test_fixture_loader() {
    let loader = FixtureLoader::new("tests/fixtures/files");

    // Load a simple text fixture
    if loader.exists("simple.rs") {
        let content = loader
            .load_string("simple.rs")
            .expect("Failed to load fixture");
        assert!(content.contains("fn main"));
    }

    // Load a JSON fixture
    if loader.exists("sample.json") {
        let json: serde_json::Value = loader
            .load_json("sample.json")
            .expect("Failed to load JSON");
        assert!(json.is_object());
    }
}

/// Test that the temp directory builder works correctly.
#[tokio::test]
async fn test_tmp_dir_builder() {
    let tmp_dir: TmpDirWithStructure = TmpDirBuilder::new("test_prefix")
        .build()
        .await
        .expect("Failed to create temp dir");

    // Write a file
    tmp_dir
        .write_file("test.txt", "Hello, World!")
        .await
        .expect("Failed to write");

    // Read it back
    let content = tmp_dir.read_file("test.txt").await.expect("Failed to read");
    assert_eq!(content, "Hello, World!");

    // Check existence
    assert!(tmp_dir.file_exists("test.txt").await);
    assert!(!tmp_dir.file_exists("nonexistent.txt").await);
}

/// Test create_tmp_dir helper function.
#[tokio::test]
async fn test_create_tmp_dir_helper() {
    use amadeus::test_utils::create_tmp_dir;

    let mut files = HashMap::new();
    files.insert("file1.txt".to_string(), "content1".to_string());
    files.insert("file2.txt".to_string(), "content2".to_string());

    let tmp_dir: TmpDirWithStructure = create_tmp_dir(&files)
        .await
        .expect("Failed to create tmp dir");

    // Files are created under "root" directory
    assert!(tmp_dir.file_exists("root/file1.txt").await);
    assert!(tmp_dir.file_exists("root/file2.txt").await);
}

/// Test file comparison utilities.
#[tokio::test]
async fn test_file_comparison() {
    use amadeus::test_utils::{file_content_equals, files_equal};

    let tmp_dir: TmpDirWithStructure = TmpDirBuilder::new("test_compare")
        .build()
        .await
        .expect("Failed to create temp dir");

    // Create two identical files
    tmp_dir
        .write_file("file1.txt", "same content")
        .await
        .expect("Failed to write");
    tmp_dir
        .write_file("file2.txt", "same content")
        .await
        .expect("Failed to write");
    tmp_dir
        .write_file("file3.txt", "different content")
        .await
        .expect("Failed to write");

    let path1 = tmp_dir.file_path("file1.txt");
    let path2 = tmp_dir.file_path("file2.txt");
    let path3 = tmp_dir.file_path("file3.txt");

    // Test files_equal
    let res1: std::io::Result<bool> = files_equal(&path1, &path2).await;
    let eq1 = res1.expect("Failed to compare");
    assert!(eq1);
    let eq2: bool = files_equal(&path1, &path3)
        .await
        .expect("Failed to compare");
    assert!(!eq2);

    // Test file_content_equals
    let res3: std::io::Result<bool> = file_content_equals(&path1, "same content").await;
    let eq3 = res3.expect("Failed to compare");
    assert!(eq3);
}

/// Test sample project structure.
#[tokio::test]
async fn test_sample_rust_project() {
    use amadeus::test_utils::sample_rust_project;

    let project = sample_rust_project();

    // Verify it has correct structure
    assert_eq!(project.file_count(), 2); // Cargo.toml + main.rs
    assert_eq!(project.dir_count(), 2); // project + src

    // Create it
    let tmp_dir: TmpDirWithStructure = TmpDirBuilder::new("rust_project")
        .with_structure(project)
        .build()
        .await
        .expect("Failed to create temp dir");

    // Verify Cargo.toml exists and contains expected content
    assert!(tmp_dir.file_exists("project/Cargo.toml").await);
    let cargo = tmp_dir
        .read_file("project/Cargo.toml")
        .await
        .expect("Failed to read");
    assert!(cargo.contains("[package]"));
    assert!(cargo.contains("name = \"sample\""));
}
