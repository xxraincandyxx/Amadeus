// @amadeus-header
// summary: Transport-agnostic slash-command definitions and parsing shared across frontends.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - type: crate::SlashCommand
// - type: crate::SlashCommandSpec
// - const: crate::SLASH_COMMAND_SPECS
// uses: none
// invariants:
// - Slash command parsing and metadata stay transport-agnostic.
// side_effects: none
// tests:
// - cmd: cargo test -p commands
// @end-amadeus-header

//! Transport-agnostic slash command parsing and metadata.

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
        self.name == candidate || self.aliases.contains(&candidate)
    }
}

pub const SLASH_COMMAND_SPECS: &[SlashCommandSpec] = &[
    SlashCommandSpec::new(
        "btw",
        &[],
        "Ask a side question without adding to conversation history",
        Some("<question>"),
    ),
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
    Btw { question: Option<String> },
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

        let mut parts = trimmed.trim_start_matches('/').splitn(2, char::is_whitespace);
        let command = parts.next()?.to_ascii_lowercase();
        let remainder = parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        Some(match command.as_str() {
            "btw" => Self::Btw {
                question: remainder.map(String::from),
            },
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
            Self::Btw { .. } => "btw",
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
        assert!(SLASH_COMMAND_SPECS.iter().any(|spec| spec.name == "btw"));
        assert!(SLASH_COMMAND_SPECS.iter().any(|spec| spec.name == "hooks"));
        assert!(SLASH_COMMAND_SPECS.iter().any(|spec| spec.name == "rewind"));
    }

    #[test]
    fn parse_known_commands_and_aliases() {
        assert_eq!(
            SlashCommand::parse("/btw"),
            Some(SlashCommand::Btw { question: None })
        );
        assert_eq!(
            SlashCommand::parse("/btw hello there"),
            Some(SlashCommand::Btw {
                question: Some("hello there".to_string())
            })
        );
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
