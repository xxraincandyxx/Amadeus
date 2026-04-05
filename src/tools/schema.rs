//! # Tool Schemas
//!
//! JSON schemas for tool definitions sent to LLMs.

use serde_json::Value;
use std::sync::OnceLock;

static BASH_TOOL_SCHEMA: OnceLock<Value> = OnceLock::new();
static READ_FILE_SCHEMA: OnceLock<Value> = OnceLock::new();
static WRITE_FILE_SCHEMA: OnceLock<Value> = OnceLock::new();
static EDIT_FILE_SCHEMA: OnceLock<Value> = OnceLock::new();
static GLOB_TOOL_SCHEMA: OnceLock<Value> = OnceLock::new();
static GREP_TOOL_SCHEMA: OnceLock<Value> = OnceLock::new();
static TODO_TOOL_SCHEMA: OnceLock<Value> = OnceLock::new();
static WEB_FETCH_TOOL_SCHEMA: OnceLock<Value> = OnceLock::new();
static SUB_AGENT_TOOL_SCHEMA: OnceLock<Value> = OnceLock::new();

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

pub fn glob_tool() -> &'static Value {
    GLOB_TOOL_SCHEMA.get_or_init(|| {
        serde_json::json!({
            "name": "glob",
            "description": "Fast file pattern matching tool that works with any codebase size. Supports glob patterns like '**/*.js' or 'src/**/*.ts'. Returns matching file paths sorted by modification time. Use this tool to find files by name patterns.",
            "parameters": {
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The glob pattern to match files against (e.g., '**/*.js', 'src/**/*.ts')"
                    },
                    "path": {
                        "type": "string",
                        "description": "The directory to search in. If not specified, the current working directory will be used."
                    }
                },
                "required": ["pattern"]
            }
        })
    })
}

pub fn grep_tool() -> &'static Value {
    GREP_TOOL_SCHEMA.get_or_init(|| {
        serde_json::json!({
            "name": "grep",
            "description": "A powerful search tool built on ripgrep. Supports full regex syntax. Use this tool to search for patterns within file contents. Prefer this over glob when searching for content within files.",
            "parameters": {
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The regular expression pattern to search for in file contents"
                    },
                    "path": {
                        "type": "string",
                        "description": "The directory or file to search in. Defaults to the current working directory if not specified."
                    },
                    "glob": {
                        "type": "string",
                        "description": "Glob pattern to filter files (e.g., '*.js', '*.rs')"
                    },
                    "case_sensitive": {
                        "type": "boolean",
                        "description": "Whether the search should be case sensitive. Default is false (case insensitive)."
                    },
                    "output_mode": {
                        "type": "string",
                        "enum": ["content", "files_with_matches"],
                        "description": "Output mode. 'content' shows matching lines with context, 'files_with_matches' only shows file paths. Default is 'content'."
                    },
                    "head_limit": {
                        "type": "integer",
                        "description": "Limit the number of results returned. Default is 100."
                    }
                },
                "required": ["pattern"]
            }
        })
    })
}

pub fn web_fetch_tool() -> &'static Value {
    WEB_FETCH_TOOL_SCHEMA.get_or_init(|| {
        serde_json::json!({
            "name": "web_fetch",
            "description": "Fetch and convert URL content to LLM-friendly input. Use this tool to retrieve content from web URLs. Only supports HTTP/HTTPS protocols and text-based content.",
            "parameters": {
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch content from (must be HTTP or HTTPS)"
                    },
                    "format": {
                        "type": "string",
                        "description": "Desired format for the response (e.g., 'text', 'markdown'). Default is raw text."
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Request timeout in seconds. Default is 20."
                    },
                    "max_bytes": {
                        "type": "integer",
                        "description": "Maximum bytes to read from response. Default is 50000."
                    }
                },
                "required": ["url"]
            }
        })
    })
}

pub fn todo_tool() -> &'static Value {
    TODO_TOOL_SCHEMA.get_or_init(|| {
        serde_json::json!({
            "name": "todo",
            "description": "Update the shared todo list for the current task. Use it to track progress on multi-step work.",
            "parameters": {
                "type": "object",
                "properties": {
                    "items": {
                        "type": "array",
                        "description": "The full todo list to store",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {
                                    "type": "string",
                                    "description": "Stable todo identifier"
                                },
                                "text": {
                                    "type": "string",
                                    "description": "Todo description"
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed"],
                                    "description": "Current todo status"
                                }
                            },
                            "required": ["id", "text", "status"]
                        }
                    }
                },
                "required": ["items"]
            }
        })
    })
}

pub fn sub_agent_tool() -> &'static Value {
    SUB_AGENT_TOOL_SCHEMA.get_or_init(|| {
        serde_json::json!({
            "name": "sub_agent",
            "description": "Spawn a focused subagent with fresh context. It shares the filesystem but not conversation history, and returns only the child's final summary.",
            "parameters": {
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "The task prompt for the subagent"
                    },
                    "description": {
                        "type": "string",
                        "description": "Optional short description of the delegated task"
                    }
                },
                "required": ["prompt"]
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
        glob_tool(),
        grep_tool(),
        todo_tool(),
        web_fetch_tool(),
        sub_agent_tool(),
    ]
}
