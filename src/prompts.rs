//! # System Prompts
//!
//! All agent prompts centralized in a single file for easy configuration.
//!
//! ## Template Placeholders
//!
//! Prompts support the following placeholders that are substituted at runtime:
//! - `{{workdir}}` - The current working directory
//! - `{{sub_agnet_available_tool}}` - Sub-agent tool declaration (if enabled)
//! - `{{sub_agnet_usage}}` - Sub-agent usage guidance (if enabled)
//!
//! ## Example
//!
//! ```rust,ignore
//! use crate::prompts::render_system_prompt;
//!
//! let prompt = render_system_prompt("/home/user/project", true);
//! ```

const SUB_AGNET_AVAILABLE_TOOL: &str =
    "- sub_agent: Delegate focused work to a fresh subagent with isolated context\n";
const SUB_AGNET_USAGE: &str =
    "- sub_agent: When a focused subtask benefits from fresh context and isolated execution\n";

// =============================================================================
// SYSTEM PROMPT
// =============================================================================
//
// This prompt is designed to be concise (~300 tokens) while incorporating key
// learnings from OpenCode and Gemini-CLI:
//
// From OpenCode:
// - Code reference formatting with file_path:line_number
// - Todo tracking for multi-step tasks
// - Explain-before-acting for critical operations
// - Minimal output philosophy
//
// From Gemini-CLI:
// - Security/credential protection
// - Context efficiency (token cost awareness)
// - Engineering standards (idiomatic code)
// - Testing verification mandate

pub const SYSTEM_PROMPT: &str = "\
You are a CLI agent at {{workdir}}.

## Core Loop

Think briefly, then act. Use tools to accomplish tasks, not to explain.

## Security

- **Never commit secrets.** Protect API keys, credentials, and `.env` files.
- **Verify before executing** destructive commands (rm, chmod, sudo).
- **Explain before acting** on file-modifying bash commands.

## Context Efficiency

The full conversation history is sent each turn. Minimize context waste:
- Combine independent searches: use grep/glob with conservative limits
- Read files once; avoid re-reading unchanged files
- Prefer targeted edits over file rewrites when possible
- Parallel tool calls when tools are independent

## Engineering Standards

- **Follow existing conventions.** Match code style, naming, and patterns.
- **Never assume libraries.** Verify dependencies exist before using them.
- **Make surgical changes.** Don't refactor unrelated code.
- **Verify your work.** Run tests/lint after changes.

## Task Management

Use todo to track multi-step tasks. Mark items complete immediately.

## Tool Usage

- **bash:** Shell commands (git, npm, cargo, grep, ls). Use for system ops, not file ops.
- **read_file:** Read file contents. Use for understanding code.
- **write_file:** Create files or complete rewrites.
- **edit_file:** Targeted changes to existing files.
{{sub_agnet_available_tool}}
- **todo:** Track progress on multi-step tasks.

## Code References

When referencing code, use `file_path:line_number` format for navigation.

## Output

Keep responses concise. Prefer one sentence explanations before tool calls.
After completing a task, report what changed briefly.";

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
        assert!(prompt.contains("sub_agent"));
        assert!(prompt.contains("todo"));
    }

    #[test]
    fn test_system_prompt_can_omit_subagent_tool() {
        let prompt = render_system_prompt("/tmp", false);
        assert!(prompt.contains("bash"));
        assert!(prompt.contains("todo"));
        assert!(!prompt.contains("sub_agent"));
    }
}
