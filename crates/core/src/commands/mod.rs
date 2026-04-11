// @amadeus-header
// summary: Core command compatibility module combining shared slash commands and runtime-backed reports.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::commands
// - module: crate::commands::composer
// - module: crate::commands::context
// - fn: crate::commands::composer::apply_citation_candidate
// - fn: crate::commands::composer::filter_citation_candidates
// - fn: crate::commands::composer::find_active_citation_query
// - fn: crate::commands::composer::format_citation_markdown
// - fn: crate::commands::composer::normalize_pasted_path
// - fn: crate::commands::composer::parse_render_spans
// - fn: crate::commands::composer::scan_workspace_citation_candidates
// - type: crate::commands::composer::ActiveCitationQuery
// - type: crate::commands::composer::CitationApplyResult
// - type: crate::commands::composer::CitationCandidate
// - type: crate::commands::composer::CitationRenderSpan
// - fn: crate::commands::context::build_context_report
// - type: crate::commands::context::ContextEntry
// - type: crate::commands::context::ContextReport
// - type: crate::commands::context::ContextSection
// - type: crate::commands::context::ContextSectionGroup
// - type: crate::commands::SlashCommand
// - type: crate::commands::SlashCommandSpec
// - const: crate::commands::SLASH_COMMAND_SPECS
// uses:
// - module: amadeus_commands
// invariants:
// - Runtime-backed command helpers stay separate from transport-agnostic slash command parsing.
// side_effects: none
// tests:
// - cmd: cargo test -p core slash_command --features full
// @end-amadeus-header

pub mod composer;
pub mod context;

pub use amadeus_commands::{SlashCommand, SlashCommandSpec, SLASH_COMMAND_SPECS};
pub use composer::{
    apply_citation_candidate, filter_citation_candidates, find_active_citation_query,
    format_citation_markdown, normalize_pasted_path, parse_render_spans,
    scan_workspace_citation_candidates, ActiveCitationQuery, CitationApplyResult,
    CitationCandidate, CitationRenderSpan,
};
pub use context::{
    build_context_report, ContextEntry, ContextReport, ContextSection, ContextSectionGroup,
};

#[cfg(test)]
mod tests {
    use super::{SlashCommand, SLASH_COMMAND_SPECS};

    #[test]
    fn slash_command_specs_include_hooks_and_rewind() {
        assert!(SLASH_COMMAND_SPECS.iter().any(|spec| spec.name == "btw"));
        assert!(SLASH_COMMAND_SPECS.iter().any(|spec| spec.name == "hooks"));
        assert!(SLASH_COMMAND_SPECS.iter().any(|spec| spec.name == "rewind"));
    }

    #[test]
    fn parse_known_commands_and_aliases() {
        assert_eq!(SlashCommand::parse("/btw"), Some(SlashCommand::Btw));
        assert_eq!(SlashCommand::parse("/help"), Some(SlashCommand::Help));
        assert_eq!(SlashCommand::parse("/compact"), Some(SlashCommand::Compact));
        assert_eq!(
            SlashCommand::parse("/compress"),
            Some(SlashCommand::Compact)
        );
        assert_eq!(SlashCommand::parse("/hooks"), Some(SlashCommand::Hooks));
        assert_eq!(
            SlashCommand::parse("/rewind 2"),
            Some(SlashCommand::Rewind { steps: Some(2) })
        );
        assert_eq!(SlashCommand::parse("/exit"), Some(SlashCommand::Exit));
    }

    #[test]
    fn parse_unknown_command() {
        assert_eq!(
            SlashCommand::parse("/unknown-command"),
            Some(SlashCommand::Unknown("unknown-command".to_string()))
        );
        assert_eq!(SlashCommand::parse("hello"), None);
    }
}
