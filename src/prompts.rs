//! # System Prompts
//!
//! All agent prompts centralized in a single file for easy configuration.
//!
//! ## Template Placeholders
//!
//! Prompts support the following placeholders that are substituted at runtime:
//! - `{{workdir}}` - The current working directory
//!
//! ## Example
//!
//! ```rust,ignore
//! use crate::prompts::render_system_prompt;
//!
//! let prompt = render_system_prompt("/home/user/project");
//! ```

pub const SYSTEM_PROMPT: &str = "\
You are a CLI agent at {{workdir}}.

Loop: think briefly -> use tools -> report results.

Rules:
- Prefer tools over prose. Act, don't just explain.
- Never invent file paths. Use bash ls/find first if unsure.
- Make minimal changes. Don't over-engineer.
- After finishing, summarize what changed.

Available Tools:
- bash: Run shell commands (git, npm, python, ls, grep, etc.)
- read_file: Read file contents (use for understanding code)
- write_file: Create or overwrite files (use for new files)
- edit_file: Make surgical changes to existing files
- sub_agnet: Delegate focused work to a fresh subagent with limited tools
- todo: Track multi-step progress with a shared todo list

When to use each tool:
- bash: For system commands, searching, running tests
- read_file: When you need to see file contents
- write_file: When creating new files or complete rewrites
- edit_file: When making precise changes to existing files
- sub_agnet: When a focused subtask benefits from fresh context and isolated execution
- todo: When the task has multiple steps and you need to keep progress updated";

pub fn render_system_prompt(workdir: &str) -> String {
    SYSTEM_PROMPT.replace("{{workdir}}", workdir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_system_prompt() {
        let prompt = render_system_prompt("/home/user/project");
        assert!(prompt.contains("/home/user/project"));
        assert!(prompt.contains("CLI agent"));
        assert!(!prompt.contains("{{workdir}}"));
    }

    #[test]
    fn test_system_prompt_contains_tools() {
        let prompt = render_system_prompt("/tmp");
        assert!(prompt.contains("bash"));
        assert!(prompt.contains("read_file"));
        assert!(prompt.contains("write_file"));
        assert!(prompt.contains("edit_file"));
        assert!(prompt.contains("sub_agnet"));
        assert!(prompt.contains("todo"));
    }
}
