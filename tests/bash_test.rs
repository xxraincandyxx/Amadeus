use amadeus::error::AgentError;
use amadeus::tools::bash::BashTool;
use amadeus::tools::tool_trait::Tool;
use serde_json::json;
use std::fs;
use std::path::Path;

fn create_tool() -> BashTool {
    BashTool::new(30, "/tmp".to_string(), vec!["rm -rf /".to_string()], 50_000)
}

#[tokio::test]
async fn test_bash_echo() {
    let tool = create_tool();
    let input = json!({"command": "echo 'hello world'"});

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("hello"));
    assert!(result.contains("world"));
}

#[tokio::test]
async fn test_bash_ls() {
    let tool = create_tool();
    let input = json!({"command": "ls -la"});

    let result = tool.execute(input).await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
async fn test_bash_pwd() {
    let tool = create_tool();
    let input = json!({"command": "pwd"});

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("/tmp"));
}

#[tokio::test]
async fn test_bash_cat_nonexistent() {
    let tool = create_tool();
    let input = json!({"command": "cat /nonexistent/file.txt"});

    let result = tool.execute(input).await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("No such file") || output.contains("error"));
}

#[tokio::test]
async fn test_bash_timeout() {
    let tool = BashTool::new(1, "/tmp".to_string(), vec![], 50_000);
    let input = json!({"command": "sleep 10"});

    let result = tool.execute(input).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AgentError::Timeout(_)));
}

#[tokio::test]
async fn test_bash_write_file() {
    let tool = create_tool();
    let test_file = "/tmp/test_agent_write.txt";

    let input = json!({"command": format!("echo 'test content' > {}", test_file)});

    let result = tool.execute(input).await;
    assert!(result.is_ok());

    let read_input = json!({"command": format!("cat {}", test_file)});
    let read_result = tool.execute(read_input).await.unwrap();
    assert!(read_result.contains("test content"));

    let _ = fs::remove_file(test_file);
}

#[tokio::test]
async fn test_bash_read_file() {
    let test_file = "/tmp/test_agent_read.txt";
    fs::write(test_file, "file content\nline 2").unwrap();

    let tool = create_tool();
    let input = json!({"command": format!("cat {}", test_file)});

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("file content"));
    assert!(result.contains("line 2"));

    let _ = fs::remove_file(test_file);
}

#[tokio::test]
async fn test_bash_grep() {
    let test_file = "/tmp/test_agent_grep.txt";
    fs::write(test_file, "apple\nbanana\ncherry\napple pie").unwrap();

    let tool = create_tool();
    let input = json!({"command": format!("grep apple {}", test_file)});

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("apple"));

    let _ = fs::remove_file(test_file);
}

#[tokio::test]
async fn test_bash_exit_code() {
    let tool = create_tool();
    let input = json!({"command": "echo 'success' && echo 'also success'"});

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("success"));
}

#[tokio::test]
async fn test_bash_multiple_commands_semicolon() {
    let tool = create_tool();
    let input = json!({"command": "echo 'one'; echo 'two'; echo 'three'"});

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("one"));
    assert!(result.contains("two"));
    assert!(result.contains("three"));
}

#[tokio::test]
async fn test_bash_variables() {
    let tool = create_tool();
    let input = json!({"command": "VAR='hello'; echo $VAR"});

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("hello"));
}

#[tokio::test]
async fn test_bash_here_document() {
    let tool = create_tool();
    let input = json!({"command": "cat << 'EOF'\nmultiline\ntext\nEOF"});

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("multiline"));
    assert!(result.contains("text"));
}

#[tokio::test]
async fn test_bash_substitution() {
    let tool = create_tool();
    let input = json!({"command": "echo 'today is' $(date +%Y-%m-%d)"});

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("today is"));
    assert!(result.chars().any(|c| c.is_ascii_digit()));
}

#[tokio::test]
async fn test_bash_find() {
    let tool = create_tool();
    let input = json!({"command": "find /tmp -maxdepth 1 -name '*.txt' 2>/dev/null | head -5"});

    let result = tool.execute(input).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_bash_mkdir_rmdir() {
    let tool = create_tool();
    let dir = "/tmp/test_agent_dir_12345";

    let mkdir_input = json!({"command": format!("mkdir {}", dir)});
    tool.execute(mkdir_input).await.unwrap();

    let test_file = format!("{}/test.txt", dir);
    let write_input = json!({"command": format!("echo 'content' > {}", test_file)});
    tool.execute(write_input).await.unwrap();

    let rm_input = json!({"command": format!("rm -rf {}", dir)});
    let result = tool.execute(rm_input).await;
    assert!(result.is_ok());
    assert!(!Path::new(dir).exists());
}

#[tokio::test]
async fn test_bash_empty_command() {
    let tool = create_tool();
    let input = json!({"command": ""});

    let result = tool.execute(input).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_bash_working_directory() {
    let test_dir = "/tmp/test_agent_workdir";
    fs::create_dir_all(test_dir).unwrap();
    fs::write(format!("{}/marker.txt", test_dir), "test").unwrap();

    let tool = BashTool::new(30, test_dir.to_string(), vec![], 50_000);
    let input = json!({"command": "ls marker.txt"});

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("marker.txt"));

    let _ = fs::remove_dir_all(test_dir);
}

#[tokio::test]
async fn test_bash_stderr_capture() {
    let tool = create_tool();
    let input = json!({"command": "echo 'stdout' && echo 'stderr' >&2"});

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("stdout"));
    assert!(result.contains("stderr"));
}

#[tokio::test]
async fn test_bash_environment_variables() {
    let tool = create_tool();
    let input = json!({"command": "env | grep HOME"});

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("HOME"));
}

#[tokio::test]
async fn test_bash_which_command() {
    let tool = create_tool();
    let input = json!({"command": "which bash"});

    let result = tool.execute(input).await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
async fn test_bash_blocked_command() {
    let tool = create_tool();
    let input = json!({"command": "rm -rf /"});

    let result = tool.execute(input).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_bash_output_truncation() {
    let tool = BashTool::new(30, "/tmp".to_string(), vec![], 100);
    let input = json!({"command": "python3 -c \"print('x' * 200)\""});

    let result = tool.execute(input).await.unwrap();
    assert!(result.len() > 100);
    assert!(result.contains("truncated"));
}
