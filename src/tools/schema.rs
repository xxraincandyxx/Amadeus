//! # Tool Schemas
//!
//! JSON schemas for tool definitions sent to LLMs.

use serde_json::Value;
use std::sync::OnceLock;

static BASH_TOOL_SCHEMA: OnceLock<Value> = OnceLock::new();
static READ_FILE_SCHEMA: OnceLock<Value> = OnceLock::new();
static WRITE_FILE_SCHEMA: OnceLock<Value> = OnceLock::new();
static EDIT_FILE_SCHEMA: OnceLock<Value> = OnceLock::new();

pub fn bash_tool() -> &'static Value {
    BASH_TOOL_SCHEMA.get_or_init(|| {
        serde_json::json!({
            "name": "bash",
            "description": "Run a shell command. Use for: ls, find, grep, git, npm, python, etc.",
            "parameters": {
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
    })
}

pub fn read_file_tool() -> &'static Value {
    READ_FILE_SCHEMA.get_or_init(|| {
        serde_json::json!({
            "name": "read_file",
            "description": "Read file contents. Returns UTF-8 text. Use for understanding existing code.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max lines to read (optional, default: all)"
                    }
                },
                "required": ["path"]
            }
        })
    })
}

pub fn write_file_tool() -> &'static Value {
    WRITE_FILE_SCHEMA.get_or_init(|| {
        serde_json::json!({
            "name": "write_file",
            "description": "Write content to a file. Creates parent directories if needed. Use for new files or complete rewrites.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path for the file"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write"
                    }
                },
                "required": ["path", "content"]
            }
        })
    })
}

pub fn edit_file_tool() -> &'static Value {
    EDIT_FILE_SCHEMA.get_or_init(|| {
        serde_json::json!({
            "name": "edit_file",
            "description": "Replace exact text in a file. Use for surgical edits to existing files.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "Exact text to find (must match precisely)"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "Replacement text"
                    },
                    "replace_all": {
                        "type": "boolean",
                        "description": "Replace all occurrences (default: false, only first)"
                    }
                },
                "required": ["path", "old_text", "new_text"]
            }
        })
    })
}

pub fn all_tools() -> Vec<&'static Value> {
    vec![
        bash_tool(),
        read_file_tool(),
        write_file_tool(),
        edit_file_tool(),
    ]
}
