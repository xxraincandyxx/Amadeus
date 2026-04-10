// @amadeus-header
// summary: Transport-agnostic permission policy types shared across runtime surfaces.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - type: crate::PermissionMode
// - type: crate::PermissionDecision
// - type: crate::PermissionRule
// - type: crate::PermissionRuleAction
// - type: crate::PermissionRuleTarget
// uses:
// - protocol: serde serialization
// invariants:
// - Permission type semantics stay independent from any specific tool implementation.
// side_effects: none
// tests:
// - cmd: cargo test -p permissions
// @end-amadeus-header

//! Transport-agnostic permission policy types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionMode {
    ReadOnly,
    #[default]
    WorkspaceWrite,
    DangerFullAccess,
    Prompt,
    Allow,
}

impl PermissionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
            Self::Prompt => "prompt",
            Self::Allow => "allow",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        match input.trim().to_ascii_lowercase().as_str() {
            "read-only" | "readonly" => Some(Self::ReadOnly),
            "workspace-write" | "workspace_write" | "accept-edits" | "acceptedits" => {
                Some(Self::WorkspaceWrite)
            }
            "danger-full-access" | "danger_full_access" | "danger" => Some(Self::DangerFullAccess),
            "prompt" => Some(Self::Prompt),
            "allow" => Some(Self::Allow),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow,
    Ask {
        required: PermissionMode,
        reason: String,
    },
    Deny {
        required: PermissionMode,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionRuleAction {
    Allow,
    Ask,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionRuleTarget {
    Bash(String),
    Tool(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRule {
    pub action: PermissionRuleAction,
    pub target: PermissionRuleTarget,
}

impl PermissionRule {
    pub fn parse(input: &str) -> Option<Self> {
        let (action, remainder) = input.split_once(':')?;
        let action = match action.trim().to_ascii_lowercase().as_str() {
            "allow" => PermissionRuleAction::Allow,
            "ask" => PermissionRuleAction::Ask,
            "deny" => PermissionRuleAction::Deny,
            _ => return None,
        };

        let remainder = remainder.trim();
        if let Some(pattern) = remainder
            .strip_prefix("bash(")
            .and_then(|value| value.strip_suffix(')'))
        {
            return Some(Self {
                action,
                target: PermissionRuleTarget::Bash(pattern.trim().to_string()),
            });
        }

        remainder
            .strip_prefix("tool(")
            .and_then(|value| value.strip_suffix(')'))
            .map(|tool| Self {
                action,
                target: PermissionRuleTarget::Tool(tool.trim().to_string()),
            })
    }

    pub fn matches(&self, tool_name: &str, command: Option<&str>) -> bool {
        match &self.target {
            PermissionRuleTarget::Tool(expected) => wildcard_match(expected, tool_name),
            PermissionRuleTarget::Bash(pattern) => {
                tool_name == "bash"
                    && command.is_some_and(|command| wildcard_match(pattern, command))
            }
        }
    }
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    let parts = pattern.split('*').collect::<Vec<_>>();
    if parts.len() == 1 {
        return pattern == value;
    }

    let anchored_start = !pattern.starts_with('*');
    let anchored_end = !pattern.ends_with('*');
    let mut remainder = value;

    for (index, part) in parts.iter().filter(|part| !part.is_empty()).enumerate() {
        if index == 0 && anchored_start {
            let Some(stripped) = remainder.strip_prefix(part) else {
                return false;
            };
            remainder = stripped;
            continue;
        }

        let Some(position) = remainder.find(part) else {
            return false;
        };
        remainder = &remainder[position + part.len()..];
    }

    if anchored_end {
        if let Some(last) = parts.iter().rev().find(|part| !part.is_empty()) {
            value.ends_with(last)
        } else {
            true
        }
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{PermissionMode, PermissionRule, PermissionRuleAction, PermissionRuleTarget};

    #[test]
    fn parses_known_permission_modes() {
        assert_eq!(
            PermissionMode::parse("read-only"),
            Some(PermissionMode::ReadOnly)
        );
        assert_eq!(
            PermissionMode::parse("workspace-write"),
            Some(PermissionMode::WorkspaceWrite)
        );
        assert_eq!(
            PermissionMode::parse("danger-full-access"),
            Some(PermissionMode::DangerFullAccess)
        );
        assert_eq!(
            PermissionMode::parse("prompt"),
            Some(PermissionMode::Prompt)
        );
        assert_eq!(PermissionMode::parse("allow"), Some(PermissionMode::Allow));
    }

    #[test]
    fn normalizes_legacy_aliases() {
        assert_eq!(
            PermissionMode::parse("accept-edits"),
            Some(PermissionMode::WorkspaceWrite)
        );
        assert_eq!(
            PermissionMode::parse("danger_full_access"),
            Some(PermissionMode::DangerFullAccess)
        );
        assert_eq!(
            PermissionMode::parse("readonly"),
            Some(PermissionMode::ReadOnly)
        );
    }

    #[test]
    fn parses_permission_rules() {
        let rule = PermissionRule::parse("allow:bash(git:*)").unwrap();
        assert_eq!(rule.action, PermissionRuleAction::Allow);
        assert_eq!(rule.target, PermissionRuleTarget::Bash("git:*".to_string()));

        let rule = PermissionRule::parse("deny:tool(write_file)").unwrap();
        assert_eq!(rule.action, PermissionRuleAction::Deny);
        assert_eq!(
            rule.target,
            PermissionRuleTarget::Tool("write_file".to_string())
        );
    }

    #[test]
    fn matches_permission_rules_with_wildcards() {
        let bash_rule = PermissionRule::parse("allow:bash(git:*)").unwrap();
        assert!(bash_rule.matches("bash", Some("git:status")));
        assert!(!bash_rule.matches("bash", Some("cargo test")));

        let tool_rule = PermissionRule::parse("ask:tool(write_*)").unwrap();
        assert!(tool_rule.matches("write_file", None));
        assert!(!tool_rule.matches("read_file", None));
    }
}
