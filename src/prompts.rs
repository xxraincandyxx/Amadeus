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
//! let prompt = render_system_prompt("/home/user/project", true);
//! ```

const SUB_AGNET_AVAILABLE_TOOL: &str =
    "- sub_agnet: Delegate focused work to a fresh subagent with limited tools\n";
const SUB_AGNET_USAGE: &str =
    "- sub_agnet: When a focused subtask benefits from fresh context and isolated execution\n";

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
{{sub_agnet_available_tool}}- todo: Track multi-step progress with a shared todo list

When to use each tool:
- bash: For system commands, searching, running tests
- read_file: When you need to see file contents
- write_file: When creating new files or complete rewrites
- edit_file: When making precise changes to existing files
{{sub_agnet_usage}}- todo: When the task has multiple steps and you need to keep progress updated";

pub fn render_system_prompt(workdir: &str, include_sub_agnet_tool: bool) -> String {
    let mut prompt = SYSTEM_PROMPT.replace("{{workdir}}", workdir);

    if include_sub_agnet_tool {
        prompt = prompt.replace("{{sub_agnet_available_tool}}", SUB_AGNET_AVAILABLE_TOOL);
        prompt = prompt.replace("{{sub_agnet_usage}}", SUB_AGNET_USAGE);
    } else {
        prompt = prompt.replace("{{sub_agnet_available_tool}}", "");
        prompt = prompt.replace("{{sub_agnet_usage}}", "");
    }

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_system_prompt() {
        let prompt = render_system_prompt("/home/user/project", true);
        assert!(prompt.contains("/home/user/project"));
        assert!(prompt.contains("CLI agent"));
        assert!(!prompt.contains("{{workdir}}"));
    }

    #[test]
    fn test_system_prompt_contains_tools() {
        let prompt = render_system_prompt("/tmp", true);
        assert!(prompt.contains("bash"));
        assert!(prompt.contains("read_file"));
        assert!(prompt.contains("write_file"));
        assert!(prompt.contains("edit_file"));
        assert!(prompt.contains("sub_agnet"));
        assert!(prompt.contains("todo"));
    }

    #[test]
    fn test_system_prompt_can_omit_subagent_tool() {
        let prompt = render_system_prompt("/tmp", false);
        assert!(prompt.contains("bash"));
        assert!(prompt.contains("todo"));
        assert!(!prompt.contains("sub_agnet"));
    }
}
