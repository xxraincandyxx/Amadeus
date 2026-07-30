// @amadeus-header
// summary: Shared prompt templates and composable system prompt builder.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - fn: crate::build_system_prompt
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

pub use builder::{
    PromptSection, PromptSectionSummary, SystemPromptBuilder, DYNAMIC_BOUNDARY_MARKER,
};
pub use sections::default_sections;

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
    fn builder_includes_default_sections_with_sub_agent() {
        let built = build_system_prompt("/tmp", true, &[]);
        for term in &[
            "bash",
            "read_file",
            "write_file",
            "edit_file",
            "sub_agent",
            "Never commit secrets",
            "context waste",
        ] {
            assert!(built.contains(term), "missing: {}", term);
        }
        assert!(built.contains("/tmp"));
    }

    #[test]
    fn builder_can_omit_sub_agent_tool() {
        let built = build_system_prompt("/tmp", false, &[]);
        assert!(!built.contains("sub_agent"));
    }

    #[test]
    fn builder_with_extra_sections() {
        let extra = PromptSection::new("custom", "Custom", "## Custom\n\nCustom content.")
            .with_priority(200);
        let built = build_system_prompt("/tmp", false, &[extra]);
        assert!(built.contains("Custom content."));
    }
}
