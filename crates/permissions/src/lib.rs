// @amadeus-header
// summary: Transport-agnostic permission policy types shared across runtime surfaces.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - type: crate::PermissionMode
// - type: crate::PermissionDecision
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
}

impl PermissionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
            Self::Prompt => "prompt",
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

#[cfg(test)]
mod tests {
    use super::PermissionMode;

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
}
