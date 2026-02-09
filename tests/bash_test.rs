use claude_agent::tools::bash::BashTool;
use claude_agent::agent::messages::ToolInput;
use claude_agent::error::AgentError;

#[tokio::test]
async fn test_bash_echo() {
    let tool = BashTool::new(30, "/tmp".to_string());
    let input = ToolInput {
        command: "echo 'hello world'".to_string(),
    };

    let result = tool.execute(&input).await.unwrap();
    assert!(result.contains("hello"));
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
    }
}
