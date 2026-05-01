// @amadeus-header
// summary: Default system prompt sections decomposed from the legacy monolithic template.
// layer: core
// status: active
// feature_flags: none
// provides:
// - fn: crate::sections::default_sections
// uses:
// - type: crate::builder::PromptSection
// invariants:
// - Default sections must produce identical output to the old render_system_prompt()
//   when joined in the same order and passed the same placeholder values.
// side_effects: none
// tests:
// - cmd: cargo test -p prompts
// @end-amadeus-header

//! Default system prompt sections.
//!
//! These decompose the old monolithic `SYSTEM_PROMPT` into individually
//! replaceable sections. Each section has a stable id and priority.

use crate::builder::PromptSection;

pub fn core_loop(workdir: &str) -> PromptSection {
    PromptSection::new(
        "core_loop",
        "Core Loop",
        format!("You are a CLI agent at {}.\n\nThink briefly, then act. Use tools to accomplish tasks, not to explain.", workdir),
    )
    .with_priority(0)
}

pub fn security() -> PromptSection {
    PromptSection::new(
        "security",
        "Security",
        "- **Never commit secrets.** Protect API keys, credentials, and `.env` files.\n- **Verify before executing** destructive commands (rm, chmod, sudo).\n- **Explain before acting** on file-modifying bash commands.",
    )
    .with_priority(10)
}

pub fn context_efficiency() -> PromptSection {
    PromptSection::new(
        "context_efficiency",
        "Context Efficiency",
        "The full conversation history is sent each turn. Minimize context waste:\n- Combine independent searches: use grep/glob with conservative limits\n- Read files once; avoid re-reading unchanged files\n- Prefer targeted edits over file rewrites when possible\n- Parallel tool calls when tools are independent",
    )
    .with_priority(20)
}

pub fn engineering_standards() -> PromptSection {
    PromptSection::new(
        "engineering_standards",
        "Engineering Standards",
        "- **Follow existing conventions.** Match code style, naming, and patterns.\n- **Never assume libraries.** Verify dependencies exist before using them.\n- **Make surgical changes.** Don't refactor unrelated code.\n- **Verify your work.** Run tests/lint after changes.",
    )
    .with_priority(30)
}

pub fn task_management() -> PromptSection {
    PromptSection::new(
        "task_management",
        "Task Management",
        "Use todo to track multi-step tasks. Mark items complete immediately.",
    )
    .with_priority(40)
}

pub fn tool_usage(include_sub_agent_tool: bool) -> PromptSection {
    let sub_agent_line = if include_sub_agent_tool {
        "- **sub_agent:** Delegate focused work to a fresh subagent with isolated context\n"
    } else {
        ""
    };

    PromptSection::new(
        "tool_usage",
        "Tool Usage",
        format!(
            "- **bash:** Shell commands (git, npm, cargo, grep, ls). Use for system ops, not file ops.\n- **read_file:** Read file contents. Use for understanding code.\n- **write_file:** Create files or complete rewrites.\n- **edit_file:** Targeted changes to existing files.\n{}- **todo:** Track progress on multi-step tasks.",
            sub_agent_line
        ),
    )
    .with_priority(50)
}

pub fn code_references() -> PromptSection {
    PromptSection::new(
        "code_references",
        "Code References",
        "When referencing code, use `file_path:line_number` format for navigation.",
    )
    .with_priority(60)
}

pub fn output_style() -> PromptSection {
    PromptSection::new(
        "output",
        "Output",
        "Keep responses concise. Prefer one sentence explanations before tool calls.\nAfter completing a task, report what changed briefly.",
    )
    .with_priority(70)
}

/// Project-specific context injected from files like `.amadeus/context.md`.
pub fn project_context(content: &str) -> PromptSection {
    PromptSection::new(
        "project_context",
        "Project Context",
        format!(
            "## Project Context\n\nThe following context has been provided by the project:\n\n{}",
            content
        ),
    )
    .with_priority(100)
    .with_dynamic(true)
}

/// All default sections in priority order.
pub fn default_sections(workdir: &str, include_sub_agent_tool: bool) -> Vec<PromptSection> {
    vec![
        core_loop(workdir),
        security(),
        context_efficiency(),
        engineering_standards(),
        task_management(),
        tool_usage(include_sub_agent_tool),
        code_references(),
        output_style(),
    ]
}
