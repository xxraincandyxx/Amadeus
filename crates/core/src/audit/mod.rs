// @amadeus-header
// summary: Read-only audit reports for tool inventory and permission exposure.
// layer: infra
// status: active
// feature_flags: none
// provides:
// - module: crate::audit
// - type: crate::audit::ToolAuditReport
// - type: crate::audit::ToolAuditEntry
// - fn: crate::audit::tool_audit_report
// uses:
// - module: crate::tools
// - module: crate::permissions
// - protocol: serde serialization
// invariants:
// - Audit reports are deterministic and do not execute tools.
// side_effects: none
// tests:
// - cmd: cargo test -p core audit --features full
// @end-amadeus-header

//! Read-only audit reporting for tool and permission surfaces.

use serde::{Deserialize, Serialize};

use crate::permissions::PermissionMode;
use crate::tools::{ToolRegistry, ToolSource};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolAuditEntry {
    pub name: String,
    pub pack: String,
    pub source: ToolSource,
    pub required_permission: PermissionMode,
    pub visible_in_modes: Vec<PermissionMode>,
    pub aliases: Vec<String>,
    pub prompt_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolAuditReport {
    pub total_tools: usize,
    pub entries: Vec<ToolAuditEntry>,
}

impl ToolAuditReport {
    pub fn to_markdown(&self) -> String {
        let mut lines = vec![
            "# Tool Audit".to_string(),
            String::new(),
            format!("Total tools: {}", self.total_tools),
            String::new(),
        ];

        for entry in &self.entries {
            lines.push(format!(
                "- `{}` [{}] requires `{}` from `{}`",
                entry.name,
                entry.source.as_str(),
                entry.required_permission.as_str(),
                entry.pack
            ));
        }

        lines.join("\n")
    }
}

pub fn tool_audit_report(registry: &ToolRegistry) -> ToolAuditReport {
    let mut entries = registry
        .catalog()
        .specs()
        .into_iter()
        .map(|spec| ToolAuditEntry {
            name: spec.name.clone(),
            pack: spec.pack.clone(),
            source: spec.source,
            required_permission: spec.required_permission,
            visible_in_modes: spec.visible_in_modes.clone(),
            aliases: spec.aliases.clone(),
            prompt_approval: spec.prompt_approval,
        })
        .collect::<Vec<_>>();

    entries.sort_by(|a, b| a.name.cmp(&b.name));

    ToolAuditReport {
        total_tools: entries.len(),
        entries,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::agent::config::Config;
    use crate::tools::{TodoManager, ToolRegistry};

    use super::tool_audit_report;

    #[test]
    fn tool_audit_report_is_deterministic() {
        let config = Config::default();
        let registry = ToolRegistry::with_defaults_and_todo(
            &config,
            Arc::new(std::sync::RwLock::new(TodoManager::new())),
        );

        let first = tool_audit_report(&registry);
        let second = tool_audit_report(&registry);

        assert_eq!(first, second);
        assert!(first.total_tools > 0);
    }
}
