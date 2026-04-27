// @amadeus-header
// summary: Permission enforcement for tool execution built on shared permission policy types.
// layer: policy
// status: active
// feature_flags: none
// provides:
// - module: crate::permissions
// - type: crate::permissions::PermissionEnforcer
// uses:
// - module: amadeus_permissions
// - module: crate::tools::bash
// - format: JSON values
// invariants:
// - Permission mode checks remain stricter than approval mode checks.
// side_effects: none
// tests:
// - cmd: cargo test -p core permissions --features full
// @end-amadeus-header

use std::path::PathBuf;

use serde_json::Value;

pub use amadeus_permissions::{PermissionDecision, PermissionMode};
use amadeus_permissions::{PermissionRule, PermissionRuleAction};

use crate::agent::config::Config;
use crate::security::PathPolicy;
use crate::tools::bash::{classify_command, is_read_only_command, BashCommandKind};
use crate::tools::ToolSpec;

#[derive(Debug, Clone)]
pub struct PermissionEnforcer {
    active_mode: PermissionMode,
    workspace_roots: Vec<PathBuf>,
    rules: Vec<PermissionRule>,
    path_policy: PathPolicy,
}

impl PermissionEnforcer {
    pub fn from_config(config: &Config) -> Self {
        let workspace_roots = {
            let mut roots = vec![config.workdir.clone()];
            roots.extend(config.permissions.additional_directories.iter().cloned());
            roots
        };
        Self {
            active_mode: config.permission_mode,
            workspace_roots: workspace_roots.clone(),
            rules: build_rules(config),
            path_policy: PathPolicy::new(
                config.workdir.clone(),
                config.permissions.additional_directories.clone(),
            ),
        }
    }

    pub fn active_mode(&self) -> PermissionMode {
        self.active_mode
    }

    pub fn workspace_roots(&self) -> &[PathBuf] {
        &self.workspace_roots
    }

    pub fn check(&self, tool_name: &str, input: &Value) -> PermissionDecision {
        let required = self.required_mode_for(tool_name, input);
        self.evaluate(tool_name, input, required)
    }

    pub fn check_with_spec(&self, spec: &ToolSpec, input: &Value) -> PermissionDecision {
        let required = self.required_mode_for_spec(spec, input);
        self.evaluate(&spec.name, input, required)
    }

    fn evaluate(
        &self,
        tool_name: &str,
        input: &Value,
        required: PermissionMode,
    ) -> PermissionDecision {
        if let Some(rule_decision) = self.rule_decision(tool_name, input, required) {
            return rule_decision;
        }

        if self.active_mode == PermissionMode::Allow {
            return PermissionDecision::Allow;
        }

        if self.active_mode == PermissionMode::Prompt {
            return PermissionDecision::Ask {
                required,
                reason: self.reason(tool_name, required, input),
            };
        }

        if self.active_mode == PermissionMode::WorkspaceWrite
            && required == PermissionMode::DangerFullAccess
        {
            return PermissionDecision::Ask {
                required,
                reason: self.reason(tool_name, required, input),
            };
        }

        if self.active_mode >= required {
            PermissionDecision::Allow
        } else {
            PermissionDecision::Deny {
                required,
                reason: self.reason(tool_name, required, input),
            }
        }
    }

    fn required_mode_for(&self, tool_name: &str, input: &Value) -> PermissionMode {
        match tool_name {
            "read_file" | "glob" | "grep" | "web_fetch" | "todo" | "sub_agent" | "call_peer" => {
                PermissionMode::ReadOnly
            }
            "write_file" | "edit_file" => {
                if self.path_within_workspace(input) {
                    PermissionMode::WorkspaceWrite
                } else {
                    PermissionMode::DangerFullAccess
                }
            }
            "bash" => match input
                .get("command")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
            {
                command if is_read_only_command(command) => PermissionMode::ReadOnly,
                command => match classify_command(command) {
                    BashCommandKind::ReadOnly => PermissionMode::WorkspaceWrite,
                    BashCommandKind::WorkspaceWrite => PermissionMode::WorkspaceWrite,
                    BashCommandKind::Destructive => PermissionMode::DangerFullAccess,
                },
            },
            _ => PermissionMode::DangerFullAccess,
        }
    }

    fn required_mode_for_spec(&self, spec: &ToolSpec, input: &Value) -> PermissionMode {
        match spec.name.as_str() {
            "write_file" | "edit_file" => {
                if self.path_within_workspace(input) {
                    spec.required_permission
                } else {
                    PermissionMode::DangerFullAccess
                }
            }
            "bash" => match input
                .get("command")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
            {
                command if is_read_only_command(command) => PermissionMode::ReadOnly,
                command => match classify_command(command) {
                    BashCommandKind::ReadOnly => PermissionMode::ReadOnly,
                    BashCommandKind::WorkspaceWrite => PermissionMode::WorkspaceWrite,
                    BashCommandKind::Destructive => PermissionMode::DangerFullAccess,
                },
            },
            _ => spec.required_permission,
        }
    }

    fn rule_decision(
        &self,
        tool_name: &str,
        input: &Value,
        required: PermissionMode,
    ) -> Option<PermissionDecision> {
        let command = input.get("command").and_then(|value| value.as_str());
        let matched_rule = self
            .rules
            .iter()
            .find(|rule| rule.matches(tool_name, command))?;

        let reason = match matched_rule.action {
            PermissionRuleAction::Allow => format!("permission rule allowed {tool_name}"),
            PermissionRuleAction::Ask => {
                format!("permission rule requires approval for {tool_name}")
            }
            PermissionRuleAction::Deny => format!("permission rule denied {tool_name}"),
        };

        Some(match matched_rule.action {
            PermissionRuleAction::Allow => PermissionDecision::Allow,
            PermissionRuleAction::Ask => PermissionDecision::Ask { required, reason },
            PermissionRuleAction::Deny => PermissionDecision::Deny { required, reason },
        })
    }

    fn path_within_workspace(&self, input: &Value) -> bool {
        let Some(path) = input.get("path").and_then(|value| value.as_str()) else {
            return true;
        };

        self.path_policy.resolve_write(path).is_ok()
    }

    fn reason(&self, tool_name: &str, required: PermissionMode, input: &Value) -> String {
        match tool_name {
            "write_file" | "edit_file" => {
                let path = input
                    .get("path")
                    .and_then(|value| value.as_str())
                    .unwrap_or("<unknown>");
                format!(
                    "tool '{tool_name}' needs {} for path '{}'",
                    required.as_str(),
                    path
                )
            }
            "bash" => {
                let command = input
                    .get("command")
                    .and_then(|value| value.as_str())
                    .unwrap_or("<unknown>");
                format!("bash command requires {}: {}", required.as_str(), command)
            }
            _ => format!("tool '{tool_name}' needs {}", required.as_str()),
        }
    }
}

fn build_rules(config: &Config) -> Vec<PermissionRule> {
    let mut rules = Vec::new();
    rules.extend(
        config
            .permissions
            .deny
            .iter()
            .filter_map(|rule| PermissionRule::parse(&format!("deny:{rule}"))),
    );
    rules.extend(
        config
            .permissions
            .ask
            .iter()
            .filter_map(|rule| PermissionRule::parse(&format!("ask:{rule}"))),
    );
    rules.extend(
        config
            .permissions
            .allow
            .iter()
            .filter_map(|rule| PermissionRule::parse(&format!("allow:{rule}"))),
    );
    rules.extend(
        config
            .permissions
            .rules
            .iter()
            .filter_map(|rule| PermissionRule::parse(rule)),
    );
    rules
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{ToolLevel, ToolSource, ToolSpec};

    fn make_config(mode: PermissionMode) -> Config {
        Config {
            permission_mode: mode,
            workdir: PathBuf::from("/tmp/workspace"),
            ..Config::default()
        }
    }

    #[test]
    fn read_only_mode_allows_tmux_and_cargo_bash_commands() {
        let enforcer = PermissionEnforcer::from_config(&make_config(PermissionMode::ReadOnly));

        assert_eq!(
            enforcer.check(
                "bash",
                &serde_json::json!({"command": "tmux-cli capture --pane=1"})
            ),
            PermissionDecision::Allow
        );
        assert_eq!(
            enforcer.check(
                "bash",
                &serde_json::json!({"command": "cargo test --features full"})
            ),
            PermissionDecision::Allow
        );
    }

    #[test]
    fn read_only_mode_denies_writes() {
        let enforcer = PermissionEnforcer::from_config(&make_config(PermissionMode::ReadOnly));

        assert!(matches!(
            enforcer.check("write_file", &serde_json::json!({"path": "notes.md"})),
            PermissionDecision::Deny {
                required: PermissionMode::WorkspaceWrite,
                ..
            }
        ));
        assert!(matches!(
            enforcer.check(
                "bash",
                &serde_json::json!({"command": "echo hi > notes.md"})
            ),
            PermissionDecision::Deny {
                required: PermissionMode::WorkspaceWrite,
                ..
            }
        ));
    }

    #[test]
    fn workspace_write_mode_requests_approval_for_destructive_bash() {
        let enforcer =
            PermissionEnforcer::from_config(&make_config(PermissionMode::WorkspaceWrite));

        assert!(matches!(
            enforcer.check("bash", &serde_json::json!({"command": "rm -rf /"})),
            PermissionDecision::Ask {
                required: PermissionMode::DangerFullAccess,
                ..
            }
        ));
    }

    #[test]
    fn allow_mode_bypasses_required_mode_checks() {
        let enforcer = PermissionEnforcer::from_config(&make_config(PermissionMode::Allow));

        assert_eq!(
            enforcer.check("write_file", &serde_json::json!({"path": "../notes.md"})),
            PermissionDecision::Allow
        );
    }

    #[test]
    fn permission_rules_override_mode_checks() {
        let mut config = make_config(PermissionMode::ReadOnly);
        config.permissions.rules = vec!["allow:bash(git:*)".to_string()];
        let enforcer = PermissionEnforcer::from_config(&config);

        assert_eq!(
            enforcer.check("bash", &serde_json::json!({"command": "git:status"})),
            PermissionDecision::Allow
        );
    }

    #[test]
    fn deny_rules_block_even_in_allow_mode() {
        let mut config = make_config(PermissionMode::Allow);
        config.permissions.rules = vec!["deny:tool(write_file)".to_string()];
        let enforcer = PermissionEnforcer::from_config(&config);

        assert!(matches!(
            enforcer.check("write_file", &serde_json::json!({"path": "notes.md"})),
            PermissionDecision::Deny { .. }
        ));
    }

    #[test]
    fn additional_directories_extend_workspace_boundary() {
        let mut config = make_config(PermissionMode::WorkspaceWrite);
        config
            .permissions
            .additional_directories
            .push(PathBuf::from("/tmp/extra"));
        let enforcer = PermissionEnforcer::from_config(&config);

        assert_eq!(
            enforcer.check(
                "write_file",
                &serde_json::json!({"path": "/tmp/extra/notes.md"})
            ),
            PermissionDecision::Allow
        );
    }

    #[test]
    fn structured_rule_arrays_are_honored_in_priority_order() {
        let mut config = make_config(PermissionMode::ReadOnly);
        config.permissions.allow = vec!["tool(read_file)".to_string()];
        config.permissions.ask = vec!["bash(git:*)".to_string()];
        config.permissions.deny = vec!["tool(write_file)".to_string()];
        let enforcer = PermissionEnforcer::from_config(&config);

        assert_eq!(
            enforcer.check("read_file", &serde_json::json!({"path": "notes.md"})),
            PermissionDecision::Allow
        );
        assert!(matches!(
            enforcer.check("bash", &serde_json::json!({"command": "git:status"})),
            PermissionDecision::Ask { .. }
        ));
        assert!(matches!(
            enforcer.check("write_file", &serde_json::json!({"path": "notes.md"})),
            PermissionDecision::Deny { .. }
        ));
    }

    #[test]
    fn spec_permission_defaults_to_declared_mode_with_runtime_escalation() {
        let enforcer =
            PermissionEnforcer::from_config(&make_config(PermissionMode::WorkspaceWrite));
        let spec = ToolSpec {
            name: "write_file".to_string(),
            description: "write".to_string(),
            input_schema: Value::Null,
            required_permission: PermissionMode::WorkspaceWrite,
            source: ToolSource::Builtin,
            level: ToolLevel::Primitive,
            tags: Vec::new(),
            aliases: Vec::new(),
            pack: "filesystem".to_string(),
            prompt_approval: false,
            visible_in_modes: vec![
                PermissionMode::WorkspaceWrite,
                PermissionMode::DangerFullAccess,
            ],
        };

        assert_eq!(
            enforcer.check_with_spec(&spec, &serde_json::json!({"path": "src/lib.rs"})),
            PermissionDecision::Allow
        );
        assert!(matches!(
            enforcer.check_with_spec(&spec, &serde_json::json!({"path": "/tmp/outside.rs"})),
            PermissionDecision::Ask {
                required: PermissionMode::DangerFullAccess,
                ..
            }
        ));
    }
}
