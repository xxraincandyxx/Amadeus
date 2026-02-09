use serde_json::Value;

pub fn bash_tool() -> Value {
    serde_json::json!({
        "name": "bash",
        "description": "Execute shell command. Common patterns:\n\
                        - Read: cat/head/tail, grep/find/rg/ls, wc -l\n\
                        - Write: echo 'content' > file, sed -i 's/old/new/g' file\n\
                        - Subagent: For complex subtasks, spawn a subagent to keep context clean:\n\
                          cargo run -- 'task description' (spawns isolated agent, returns summary)",
        "input_schema": {
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                }
            },
            "required": ["command"]
        }
    })
}
