// @amadeus-header
// summary: Core slash-command definitions and parsing shared across frontends.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::commands
// - module: crate::commands::context
// - fn: crate::commands::context::build_context_report
// - type: crate::commands::SlashCommand
// - type: crate::commands::SlashCommandSpec
// - type: crate::commands::context::ContextEntry
// - type: crate::commands::context::ContextReport
// - type: crate::commands::context::ContextSection
// - type: crate::commands::context::ContextSectionGroup
// - const: crate::commands::SLASH_COMMAND_SPECS
// uses: none
// invariants:
// - Slash command parsing and metadata stay transport-agnostic.
// side_effects: none
// tests:
// - cmd: cargo test -p core slash_command --features full
// @end-amadeus-header

pub mod context;

pub use context::{
    build_context_report, ContextEntry, ContextReport, ContextSection, ContextSectionGroup,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashCommandSpec {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub summary: &'static str,
    pub argument_hint: Option<&'static str>,
}

impl SlashCommandSpec {
    pub const fn new(
        name: &'static str,
        aliases: &'static [&'static str],
        summary: &'static str,
        argument_hint: Option<&'static str>,
    ) -> Self {
        Self {
            name,
            aliases,
            summary,
            argument_hint,
        }
    }

    pub fn matches(&self, candidate: &str) -> bool {
        self.name == candidate || self.aliases.iter().any(|alias| *alias == candidate)
    }
}

pub const SLASH_COMMAND_SPECS: &[SlashCommandSpec] = &[
    SlashCommandSpec::new("help", &[], "Show available commands", None),
    SlashCommandSpec::new("compact", &["compress"], "Trigger context compaction", None),
    SlashCommandSpec::new("context", &[], "Show current context usage", None),
    SlashCommandSpec::new("hooks", &[], "Inspect configured hook phases", None),
    SlashCommandSpec::new("new-agent", &[], "Create a new agent session", None),
    SlashCommandSpec::new(
        "rewind",
        &[],
        "Restore the session to an earlier checkpoint",
        Some("[steps]"),
    ),
    SlashCommandSpec::new("exit", &[], "Exit the current TUI session", None),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommand {
    Help,
    Compact,
    Context,
    Hooks,
    NewAgent,
    Rewind { steps: Option<usize> },
    Exit,
    Unknown(String),
}

impl SlashCommand {
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        if !trimmed.starts_with('/') {
            return None;
        }

        let mut parts = trimmed.trim_start_matches('/').split_whitespace();
        let command = parts.next()?.to_ascii_lowercase();
        let remainder = parts.next();

        Some(match command.as_str() {
            "help" => Self::Help,
            "compact" | "compress" => Self::Compact,
            "context" => Self::Context,
            "hooks" => Self::Hooks,
            "new-agent" => Self::NewAgent,
            "rewind" => Self::Rewind {
                steps: remainder.and_then(|value| value.parse::<usize>().ok()),
            },
            "exit" => Self::Exit,
            other => Self::Unknown(other.to_string()),
        })
    }

    pub fn primary_name(&self) -> &'static str {
        match self {
            Self::Help => "help",
            Self::Compact => "compact",
            Self::Context => "context",
            Self::Hooks => "hooks",
            Self::NewAgent => "new-agent",
            Self::Rewind { .. } => "rewind",
            Self::Exit => "exit",
            Self::Unknown(_) => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SlashCommand, SLASH_COMMAND_SPECS};

    #[test]
    fn slash_command_specs_include_hooks_and_rewind() {
        assert!(SLASH_COMMAND_SPECS.iter().any(|spec| spec.name == "hooks"));
        assert!(SLASH_COMMAND_SPECS.iter().any(|spec| spec.name == "rewind"));
    }

    #[test]
    fn parse_known_commands_and_aliases() {
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
            SlashCommand::parse("/btw"),
            Some(SlashCommand::Unknown("btw".to_string()))
        );
        assert_eq!(SlashCommand::parse("hello"), None);
    }
}
