use claude_agent::tools::bash::BashTool;
use claude_agent::agent::messages::ToolInput;
use claude_agent::error::AgentError;
use std::fs;
use std::path::Path;

#[tokio::test]
async fn test_bash_echo() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "echo 'hello world'".to_string(),
    };

    let result = tool.execute(&input).await.unwrap();
    assert!(result.contains("hello"));
    assert!(result.contains("world"));
}

#[tokio::test]
async fn test_bash_ls() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "ls -la".to_string(),
    };

    let result = tool.execute(&input).await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
async fn test_bash_pwd() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "pwd".to_string(),
    };

    let result = tool.execute(&input).await.unwrap();
    assert!(result.contains("/tmp"));
}

#[tokio::test]
async fn test_bash_cat_nonexistent() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "cat /nonexistent/file.txt".to_string(),
    };

    let result = tool.execute(&input).await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("No such file") || output.contains("error"));
}

#[tokio::test]
async fn test_bash_timeout() {
    let tool = BashTool::new(1, "/tmp".to_string());
    let input = ToolInput {
        command: "sleep 10".to_string(),
    };

    let result = tool.execute(&input).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AgentError::Timeout(_)));
}

#[tokio::test]
async fn test_bash_write_file() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let test_file = "/tmp/test_agent_write.txt";
    
    let input = ToolInput {
        command: format!("echo 'test content' > {}", test_file),
    };

    let result = tool.execute(&input).await;
    assert!(result.is_ok());

    let read_input = ToolInput {
        command: format!("cat {}", test_file),
    };
    let read_result = tool.execute(&read_input).await.unwrap();
    assert!(read_result.contains("test content"));

    let _ = fs::remove_file(test_file);
}

#[tokio::test]
async fn test_bash_read_file() {
    let test_file = "/tmp/test_agent_read.txt";
    fs::write(test_file, "file content\nline 2").unwrap();

    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: format!("cat {}", test_file),
    };

    let result = tool.execute(&input).await.unwrap();
    assert!(result.contains("file content"));
    assert!(result.contains("line 2"));

    let _ = fs::remove_file(test_file);
}

#[tokio::test]
async fn test_bash_grep() {
    let test_file = "/tmp/test_agent_grep.txt";
    fs::write(test_file, "apple\nbanana\ncherry\napple pie").unwrap();

    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: format!("grep apple {}", test_file),
    };

    let result = tool.execute(&input).await.unwrap();
    assert!(result.contains("apple"));

    let _ = fs::remove_file(test_file);
}

#[tokio::test]
async fn test_bash_exit_code() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "echo 'success' && echo 'also success'".to_string(),
    };

    let result = tool.execute(&input).await.unwrap();
    assert!(result.contains("success"));
}

#[tokio::test]
async fn test_bash_multiple_commands_semicolon() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "echo 'one'; echo 'two'; echo 'three'".to_string(),
    };

    let result = tool.execute(&input).await.unwrap();
    assert!(result.contains("one"));
    assert!(result.contains("two"));
    assert!(result.contains("three"));
}

#[tokio::test]
async fn test_bash_variables() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "VAR='hello'; echo $VAR".to_string(),
    };

    let result = tool.execute(&input).await.unwrap();
    assert!(result.contains("hello"));
}

#[tokio::test]
async fn test_bash_here_document() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "cat << 'EOF'\nmultiline\ntext\nEOF".to_string(),
    };

    let result = tool.execute(&input).await.unwrap();
    assert!(result.contains("multiline"));
    assert!(result.contains("text"));
}

#[tokio::test]
async fn test_bash_substitution() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "echo 'today is' $(date +%Y-%m-%d)".to_string(),
    };

    let result = tool.execute(&input).await.unwrap();
    assert!(result.contains("today is"));
    assert!(result.chars().any(|c| c.is_digit(10))); // Should contain date
}

#[tokio::test]
async fn test_bash_find() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "find /tmp -maxdepth 1 -name '*.txt' 2>/dev/null | head -5".to_string(),
    };

    let result = tool.execute(&input).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_bash_mkdir_rmdir() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let dir = "/tmp/test_agent_dir_12345";

    let mkdir_input = ToolInput {
        command: format!("mkdir {}", dir),
    };
    tool.execute(&mkdir_input).await.unwrap();

    let test_file = format!("{}/test.txt", dir);
    let write_input = ToolInput {
        command: format!("echo 'content' > {}", test_file),
    };
    tool.execute(&write_input).await.unwrap();

    let rm_input = ToolInput {
        command: format!("rm -rf {}", dir),
    };
    let result = tool.execute(&rm_input).await;
    assert!(result.is_ok());
    assert!(!Path::new(dir).exists());
}

#[tokio::test]
async fn test_bash_concurrent() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let inputs = vec![
        ToolInput {
            command: "echo 'a'".to_string(),
        },
        ToolInput {
            command: "echo 'b'".to_string(),
        },
        ToolInput {
            command: "echo 'c'".to_string(),
        },
    ];

    let results = tool.execute_all(inputs).await;
    assert_eq!(results.len(), 3);
    
    for result in results {
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("a") || output.contains("b") || output.contains("c"));
    }
}

#[tokio::test]
async fn test_bash_concurrent_with_timeout() {
    let tool = BashTool::new(1, "/tmp".to_string());
    let inputs = vec![
        ToolInput {
            command: "echo 'fast'".to_string(),
        },
        ToolInput {
            command: "sleep 10".to_string(),
        },
        ToolInput {
            command: "echo 'also fast'".to_string(),
        },
    ];

    let results = tool.execute_all(inputs).await;
    assert_eq!(results.len(), 3);
    
    let successes = results.iter().filter(|r| r.is_ok()).count();
    let timeouts = results.iter().filter(|r| {
        r.as_ref().err().map_or(false, |e| matches!(e, AgentError::Timeout(_)))
    }).count();
    
    assert!(successes >= 2); // Fast commands should succeed
    assert!(timeouts >= 1); // Slow command should timeout
}

#[tokio::test]
async fn test_bash_empty_command() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "".to_string(),
    };

    let result = tool.execute(&input).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_bash_working_directory() {
    let test_dir = "/tmp/test_agent_workdir";
    fs::create_dir_all(test_dir).unwrap();
    fs::write(format!("{}/marker.txt", test_dir), "test").unwrap();

    let tool = BashTool::new(30, test_dir.to_string());
    let input = ToolInput {
        command: "ls marker.txt".to_string(),
    };

    let result = tool.execute(&input).await.unwrap();
    assert!(result.contains("marker.txt"));

    let _ = fs::remove_dir_all(test_dir);
}

#[tokio::test]
async fn test_bash_stderr_capture() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "echo 'stdout' && echo 'stderr' >&2".to_string(),
    };

    let result = tool.execute(&input).await.unwrap();
    assert!(result.contains("stdout"));
    assert!(result.contains("stderr"));
}

#[tokio::test]
async fn test_bash_complex_pipeline() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "echo '  hello  world  ' | tr -s ' ' | trim".to_string(),
    };

    let result = tool.execute(&input).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_bash_environment_variables() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "env | grep HOME".to_string(),
    };

    let result = tool.execute(&input).await.unwrap();
    assert!(result.contains("HOME"));
}

#[tokio::test]
async fn test_bash_which_command() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "which bash".to_string(),
    };

    let result = tool.execute(&input).await.unwrap();
    assert!(!result.is_empty());
}
