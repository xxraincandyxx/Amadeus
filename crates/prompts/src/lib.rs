// @amadeus-header
// summary: Shared prompt templates and composable system prompt builder.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - const: crate::SYSTEM_PROMPT
// - fn: crate::render_system_prompt
// - type: crate::builder::SystemPromptBuilder
// - type: crate::builder::PromptSection
// uses: none
// invariants:
// - Prompt rendering stays deterministic and transport-agnostic.
// side_effects: none
// tests:
// - cmd: cargo test -p prompts
// @end-amadeus-header

//! Shared system prompt templates and composable builder.

pub mod builder;
pub mod sections;

pub use builder::{PromptSection, PromptSectionSummary, SystemPromptBuilder, DYNAMIC_BOUNDARY_MARKER};
pub use sections::default_sections;

const SUB_AGENT_AVAILABLE_TOOL: &str =
    "- sub_agent: Delegate focused work to a fresh subagent with isolated context\n";
const SUB_AGENT_USAGE: &str =
    "- sub_agent: When a focused subtask benefits from fresh context and isolated execution\n";

/// Legacy monolithic system prompt — kept for backward compatibility.
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
{{sub_agent_available_tool}}
- **todo:** Track progress on multi-step tasks.

## Code References

When referencing code, use `file_path:line_number` format for navigation.

## Output

Keep responses concise. Prefer one sentence explanations before tool calls.
After completing a task, report what changed briefly.";

/// Render the system prompt using the legacy monolithic template.
///
/// Prefer [`build_system_prompt`] for new code — it uses the composable builder.
pub fn render_system_prompt(workdir: &str, include_sub_agent_tool: bool) -> String {
    let mut prompt = SYSTEM_PROMPT.replace("{{workdir}}", workdir);

    if include_sub_agent_tool {
        prompt = prompt.replace("{{sub_agent_available_tool}}", SUB_AGENT_AVAILABLE_TOOL);
        prompt = prompt.replace("{{sub_agent_usage}}", SUB_AGENT_USAGE);
    } else {
        prompt = prompt.replace("{{sub_agent_available_tool}}", "");
        prompt = prompt.replace("{{sub_agent_usage}}", "");
    }

    prompt
}

/// Build a system prompt using the composable builder from default sections.
pub fn build_system_prompt(
    workdir: &str,
    include_sub_agent_tool: bool,
    extra_sections: &[PromptSection],
) -> String {
    let mut builder = SystemPromptBuilder::new();
    for section in default_sections(workdir, include_sub_agent_tool) {
        builder = builder.add_section(section);
    }
    for section in extra_sections {
        builder = builder.add_section(section.clone());
    }
    builder.build()
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

    #[test]
    fn builder_matches_legacy_with_sub_agent() {
        let legacy = render_system_prompt("/tmp", true);
        let built = build_system_prompt("/tmp", true, &[]);
        // The builder doesn't use the same "## Section" headings, but
        // it should contain all the same key terms from the content.
        for term in &["bash", "read_file", "write_file", "edit_file", "sub_agent", "Never commit secrets", "context waste"] {
            assert!(built.contains(term), "missing: {}", term);
        }
        // Both should be roughly the same length.
        let diff = (legacy.len() as i64 - built.len() as i64).abs();
        assert!(diff < 200, "legacy={} built={} diff={}", legacy.len(), built.len(), diff);
    }

    #[test]
    fn builder_with_extra_sections() {
        let extra = PromptSection::new("custom", "Custom", "## Custom\n\nCustom content.").with_priority(200);
        let built = build_system_prompt("/tmp", false, &[extra]);
        assert!(built.contains("Custom content."));
    }
}
