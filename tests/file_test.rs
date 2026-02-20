use claude_agent::error::AgentError;
use claude_agent::tools::file::{EditFileTool, FileTools, ReadFileTool, WriteFileTool};
use claude_agent::tools::tool_trait::Tool;
use serde_json::json;
use tempfile::TempDir;

struct TestTools {
    read: ReadFileTool,
    write: WriteFileTool,
    edit: EditFileTool,
    _temp_dir: Option<TempDir>,
}

fn create_tools() -> TestTools {
    let temp_dir = TempDir::new().unwrap();
    let workdir = temp_dir.path().to_path_buf();
    let tools = FileTools::new(workdir, 50_000);
    TestTools {
        read: ReadFileTool::new(tools.clone()),
        write: WriteFileTool::new(tools.clone()),
        edit: EditFileTool::new(tools),
        _temp_dir: Some(temp_dir),
    }
}

fn create_tools_with_workdir(workdir: &std::path::Path) -> TestTools {
    let tools = FileTools::new(workdir.to_path_buf(), 50_000);
    TestTools {
        read: ReadFileTool::new(tools.clone()),
        write: WriteFileTool::new(tools.clone()),
        edit: EditFileTool::new(tools),
        _temp_dir: None,
    }
}

fn create_tools_with_limit(max_bytes: usize) -> TestTools {
    let temp_dir = TempDir::new().unwrap();
    let workdir = temp_dir.path().to_path_buf();
    let tools = FileTools::new(workdir, max_bytes);
    TestTools {
        read: ReadFileTool::new(tools.clone()),
        write: WriteFileTool::new(tools.clone()),
        edit: EditFileTool::new(tools),
        _temp_dir: Some(temp_dir),
    }
}

#[tokio::test]
async fn test_file_write_and_read() {
    let tools = create_tools();

    let write_input = json!({
        "path": "test.txt",
        "content": "hello world"
    });
    let result = tools.write.execute(write_input).await.unwrap();
    assert!(result.contains("Wrote"));
    assert!(result.contains("test.txt"));

    let read_input = json!({"path": "test.txt"});
    let result = tools.read.execute(read_input).await.unwrap();
    assert_eq!(result, "hello world");
}

#[tokio::test]
async fn test_file_write_creates_parent_dirs() {
    let tools = create_tools();

    let write_input = json!({
        "path": "subdir/nested/deep/file.txt",
        "content": "nested content"
    });
    let result = tools.write.execute(write_input).await.unwrap();
    assert!(result.contains("Wrote"));

    let read_input = json!({"path": "subdir/nested/deep/file.txt"});
    let result = tools.read.execute(read_input).await.unwrap();
    assert_eq!(result, "nested content");
}

#[tokio::test]
async fn test_file_read_with_limit() {
    let tools = create_tools();

    let content = "line1\nline2\nline3\nline4\nline5";
    let write_input = json!({
        "path": "multiline.txt",
        "content": content
    });
    tools.write.execute(write_input).await.unwrap();

    let read_input = json!({
        "path": "multiline.txt",
        "limit": 2
    });
    let result = tools.read.execute(read_input).await.unwrap();
    assert_eq!(result, "line1\nline2");
}

#[tokio::test]
async fn test_file_read_limit_greater_than_lines() {
    let tools = create_tools();

    let content = "line1\nline2";
    let write_input = json!({
        "path": "short.txt",
        "content": content
    });
    tools.write.execute(write_input).await.unwrap();

    let read_input = json!({
        "path": "short.txt",
        "limit": 10
    });
    let result = tools.read.execute(read_input).await.unwrap();
    assert_eq!(result, "line1\nline2");
}

#[tokio::test]
async fn test_file_read_nonexistent() {
    let tools = create_tools();

    let read_input = json!({"path": "nonexistent.txt"});
    let result = tools.read.execute(read_input).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_file_edit_replace_first() {
    let tools = create_tools();

    let write_input = json!({
        "path": "edit.txt",
        "content": "hello world hello"
    });
    tools.write.execute(write_input).await.unwrap();

    let edit_input = json!({
        "path": "edit.txt",
        "old_text": "hello",
        "new_text": "goodbye"
    });
    let result = tools.edit.execute(edit_input).await.unwrap();
    assert!(result.contains("Edited"));

    let read_input = json!({"path": "edit.txt"});
    let result = tools.read.execute(read_input).await.unwrap();
    assert_eq!(result, "goodbye world hello");
}

#[tokio::test]
async fn test_file_edit_replace_all() {
    let tools = create_tools();

    let write_input = json!({
        "path": "replace_all.txt",
        "content": "foo bar foo bar foo"
    });
    tools.write.execute(write_input).await.unwrap();

    let edit_input = json!({
        "path": "replace_all.txt",
        "old_text": "foo",
        "new_text": "baz",
        "replace_all": true
    });
    let result = tools.edit.execute(edit_input).await.unwrap();
    assert!(result.contains("Edited"));

    let read_input = json!({"path": "replace_all.txt"});
    let result = tools.read.execute(read_input).await.unwrap();
    assert_eq!(result, "baz bar baz bar baz");
}

#[tokio::test]
async fn test_file_edit_text_not_found() {
    let tools = create_tools();

    let write_input = json!({
        "path": "noedit.txt",
        "content": "unchanged content"
    });
    tools.write.execute(write_input).await.unwrap();

    let edit_input = json!({
        "path": "noedit.txt",
        "old_text": "nonexistent",
        "new_text": "replacement"
    });
    let result = tools.edit.execute(edit_input).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AgentError::TextNotFound { .. }));
}

#[tokio::test]
async fn test_file_path_escape_parent_dir() {
    let temp_dir = TempDir::new().unwrap();
    let workdir = temp_dir.path().to_path_buf();
    let tools = create_tools_with_workdir(&workdir);

    let write_input = json!({
        "path": "../outside.txt",
        "content": "escape attempt"
    });
    let result = tools.write.execute(write_input).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AgentError::PathEscape(_)));

    let read_input = json!({"path": "../outside.txt"});
    let result = tools.read.execute(read_input).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AgentError::PathEscape(_)));

    let edit_input = json!({
        "path": "../outside.txt",
        "old_text": "x",
        "new_text": "y"
    });
    let result = tools.edit.execute(edit_input).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AgentError::PathEscape(_)));
}

#[tokio::test]
async fn test_file_path_escape_absolute_path() {
    let temp_dir = TempDir::new().unwrap();
    let workdir = temp_dir.path().to_path_buf();
    let tools = create_tools_with_workdir(&workdir);

    let write_input = json!({
        "path": "/etc/passwd",
        "content": "escape attempt"
    });
    let result = tools.write.execute(write_input).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AgentError::PathEscape(_)));

    let read_input = json!({"path": "/etc/passwd"});
    let result = tools.read.execute(read_input).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AgentError::PathEscape(_)));
}

#[tokio::test]
async fn test_file_path_escape_nested_parent() {
    let temp_dir = TempDir::new().unwrap();
    let workdir = temp_dir.path().to_path_buf();
    let tools = create_tools_with_workdir(&workdir);

    let write_input = json!({
        "path": "subdir/../../outside.txt",
        "content": "escape attempt"
    });
    let result = tools.write.execute(write_input).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AgentError::PathEscape(_)));
}

#[tokio::test]
async fn test_file_valid_subdir_access() {
    let temp_dir = TempDir::new().unwrap();
    let workdir = temp_dir.path().to_path_buf();
    let tools = create_tools_with_workdir(&workdir);

    let write_input = json!({
        "path": "subdir/file.txt",
        "content": "valid nested"
    });
    let result = tools.write.execute(write_input).await.unwrap();
    assert!(result.contains("Wrote"));

    let read_input = json!({"path": "subdir/file.txt"});
    let result = tools.read.execute(read_input).await.unwrap();
    assert_eq!(result, "valid nested");
}

#[tokio::test]
async fn test_file_output_truncation() {
    let tools = create_tools_with_limit(100);

    let long_content = "x".repeat(200);
    let write_input = json!({
        "path": "long.txt",
        "content": long_content
    });
    tools.write.execute(write_input).await.unwrap();

    let read_input = json!({"path": "long.txt"});
    let result = tools.read.execute(read_input).await.unwrap();
    assert!(result.len() > 100);
    assert!(result.contains("truncated"));
}

#[tokio::test]
async fn test_file_overwrite_existing() {
    let tools = create_tools();

    let write_input = json!({
        "path": "overwrite.txt",
        "content": "original content"
    });
    tools.write.execute(write_input).await.unwrap();

    let write_input = json!({
        "path": "overwrite.txt",
        "content": "new content"
    });
    tools.write.execute(write_input).await.unwrap();

    let read_input = json!({"path": "overwrite.txt"});
    let result = tools.read.execute(read_input).await.unwrap();
    assert_eq!(result, "new content");
}

#[tokio::test]
async fn test_file_empty_content() {
    let tools = create_tools();

    let write_input = json!({
        "path": "empty.txt",
        "content": ""
    });
    let result = tools.write.execute(write_input).await.unwrap();
    assert!(result.contains("Wrote 0 bytes"));

    let read_input = json!({"path": "empty.txt"});
    let result = tools.read.execute(read_input).await.unwrap();
    assert_eq!(result, "");
}

#[tokio::test]
async fn test_file_special_characters_in_content() {
    let tools = create_tools();

    let content = "hello\nworld\ttab\r\n\"quotes\" 'apostrophe' 🎉";
    let write_input = json!({
        "path": "special.txt",
        "content": content
    });
    tools.write.execute(write_input).await.unwrap();

    let read_input = json!({"path": "special.txt"});
    let result = tools.read.execute(read_input).await.unwrap();
    assert!(result.contains("hello"));
    assert!(result.contains("world"));
    assert!(result.contains("🎉"));
}

#[tokio::test]
async fn test_file_tool_names() {
    let tools = create_tools();

    assert_eq!(tools.read.name(), "read_file");
    assert_eq!(tools.write.name(), "write_file");
    assert_eq!(tools.edit.name(), "edit_file");
}

#[tokio::test]
async fn test_file_tool_schemas() {
    let tools = create_tools();

    let read_schema = tools.read.schema();
    assert!(read_schema.is_object());
    assert!(read_schema.get("name").is_some());

    let write_schema = tools.write.schema();
    assert!(write_schema.is_object());
    assert!(write_schema.get("name").is_some());

    let edit_schema = tools.edit.schema();
    assert!(edit_schema.is_object());
    assert!(edit_schema.get("name").is_some());
}

#[tokio::test]
async fn test_file_invalid_input() {
    let tools = create_tools();

    let invalid_input = json!({"invalid": "field"});
    let result = tools.write.execute(invalid_input).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AgentError::ToolInput { .. }));
}
