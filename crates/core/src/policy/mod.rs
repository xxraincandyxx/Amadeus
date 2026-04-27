// @amadeus-header
// summary: Module root for the policy subsystem and its exports.
// layer: policy
// status: active
// feature_flags: none
// provides:
// - module: crate::policy
// - type: crate::policy::ApprovalMode
// - type: crate::policy::Policy
// uses:
// - module: crate::agent::config::Config
// - module: crate::tools::bash
// - protocol: serde serialization
// - format: JSON values
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - tests/mod.rs
// @end-amadeus-header

//! # Approval/Policy System
//!
//! Control which tools require approval before execution.
//!
//! ## Approval Modes
//!
//! - `Auto` - Execute all tools automatically
//! - `Ask` - Ask for dangerous operations
//! - `Strict` - Ask for all tool executions
//!
//! ## Configuration
//!
//! ```json
//! {
//!   "mode": "ask",
//!   "auto_approve": ["read_file", "glob", "grep"],
//!   "auto_deny": ["bash:rm -rf"],
//!   "dangerous_patterns": [
//!     ["bash", "sudo"],
//!     ["bash", "chmod"],
//!     ["write_file", "\\.env$"]
//!   ]
//! }
//! ```

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::agent::config::Config;
use crate::tools::bash::{classify_command, BashCommandKind};

/// Approval mode for tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalMode {
    /// Execute all tools automatically.
    Auto,
    /// Ask for dangerous operations only.
    #[default]
    Ask,
    /// Ask for all tool executions.
    Strict,
}

/// Policy configuration for tool approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    /// The approval mode.
    #[serde(default)]
    pub mode: ApprovalMode,
    /// Tools that are always auto-approved.
    #[serde(default)]
    pub auto_approve: Vec<String>,
    /// Tools that are always auto-denied.
    #[serde(default)]
    pub auto_deny: Vec<String>,
    /// Dangerous patterns: (tool_name, pattern_regex).
    #[serde(default)]
    pub dangerous_patterns: Vec<(String, String)>,
    /// Scoped invocation grants remembered from compatibility AlwaysApprove decisions.
    #[serde(default)]
    pub scoped_auto_approve: Vec<ScopedApprovalGrant>,
    /// Cache for compiled regex patterns.
    #[serde(skip)]
    dangerous_regex_cache: Vec<(String, Regex)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScopedApprovalGrant {
    pub tool: String,
    pub input_hash: u64,
}

impl Default for Policy {
    fn default() -> Self {
        let mut policy = Self {
            mode: ApprovalMode::Ask,
            auto_approve: vec![
                "read_file".to_string(),
                "glob".to_string(),
                "grep".to_string(),
                "todo".to_string(),
            ],
            auto_deny: vec![],
            dangerous_patterns: vec![
                ("bash".to_string(), "sudo".to_string()),
                ("bash".to_string(), "chmod\\s+777".to_string()),
                ("bash".to_string(), "rm\\s+-rf\\s+/".to_string()),
                ("bash".to_string(), "\\|\\s*sh".to_string()),
                ("bash".to_string(), "\\|\\s*bash".to_string()),
                ("write_file".to_string(), "\\.env$".to_string()),
                ("write_file".to_string(), "\\.pem$".to_string()),
                ("write_file".to_string(), "\\.key$".to_string()),
            ],
            scoped_auto_approve: Vec::new(),
            dangerous_regex_cache: Vec::new(),
        };
        policy.compile_patterns();
        policy
    }
}

impl Policy {
    /// Create a new policy with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create policy from configuration.
    pub fn from_config(_config: &Config) -> Self {
        // TODO: Load from config file
        Self::default()
    }

    /// Load policy from a JSON value.
    pub fn from_json(json: &Value) -> Result<Self, serde_json::Error> {
        let mut policy: Policy = serde_json::from_value(json.clone())?;
        // Pre-compile regex patterns
        policy.compile_patterns();
        Ok(policy)
    }

    /// Compile regex patterns for dangerous operations.
    fn compile_patterns(&mut self) {
        self.dangerous_regex_cache = self
            .dangerous_patterns
            .iter()
            .filter_map(|(tool, pattern)| Regex::new(pattern).ok().map(|re| (tool.clone(), re)))
            .collect();
    }

    /// Check if a tool needs approval.
    ///
    /// Returns `true` if the tool execution should be blocked for approval.
    pub fn needs_approval(&self, tool: &str, input: &Value) -> bool {
        match self.mode {
            ApprovalMode::Auto => false,
            ApprovalMode::Strict => {
                // In strict mode, check auto_approve list
                !self.is_auto_approved(tool, input)
            }
            ApprovalMode::Ask => {
                // Check auto-approve first
                if self.is_auto_approved(tool, input) {
                    return false;
                }
                // Check auto-deny
                if self.is_auto_denied(tool, input) {
                    return true;
                }
                if tool == "bash" {
                    match self.bash_command_kind(input) {
                        BashCommandKind::ReadOnly => {}
                        BashCommandKind::WorkspaceWrite | BashCommandKind::Destructive => {
                            return true;
                        }
                    }
                }
                // Check dangerous patterns
                self.is_dangerous(tool, input)
            }
        }
    }

    /// Check if the tool is auto-approved.
    fn is_auto_approved(&self, tool: &str, input: &Value) -> bool {
        self.scoped_auto_approve
            .iter()
            .any(|grant| grant.tool == tool && grant.input_hash == input_hash(input))
            || self.auto_approve.iter().any(|t| t == tool)
    }

    /// Check if the tool is auto-denied.
    fn is_auto_denied(&self, tool: &str, _input: &Value) -> bool {
        self.auto_deny.iter().any(|t| {
            if t.contains(':') {
                let parts: Vec<&str> = t.splitn(2, ':').collect();
                parts[0] == tool
            } else {
                t == tool
            }
        })
    }

    /// Check if the tool input matches a dangerous pattern.
    fn is_dangerous(&self, tool: &str, input: &Value) -> bool {
        // Get the relevant string from input to check
        let input_str = self.extract_check_string(input);

        // Check against dangerous patterns
        for (pattern_tool, regex) in &self.dangerous_regex_cache {
            if pattern_tool == tool && regex.is_match(&input_str) {
                return true;
            }
        }

        false
    }

    fn bash_command_kind(&self, input: &Value) -> BashCommandKind {
        input
            .get("command")
            .and_then(|value| value.as_str())
            .map(classify_command)
            .unwrap_or(BashCommandKind::ReadOnly)
    }

    /// Extract a string from input for pattern checking.
    fn extract_check_string(&self, input: &Value) -> String {
        // For bash tool, check the command
        if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
            return cmd.to_string();
        }
        // For file tools, check the path
        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
            return path.to_string();
        }
        // Default: serialize to string
        input.to_string()
    }

    /// Get the reason why approval is needed.
    pub fn approval_reason(&self, tool: &str, input: &Value) -> String {
        let input_str = self.extract_check_string(input);

        if self.is_auto_denied(tool, input) {
            return format!("Tool '{}' is in the auto-deny list", tool);
        }

        if tool == "bash" {
            match self.bash_command_kind(input) {
                BashCommandKind::WorkspaceWrite => {
                    return format!(
                        "Tool '{}' requires approval for workspace-write command",
                        tool
                    );
                }
                BashCommandKind::Destructive => {
                    return format!("Tool '{}' requires approval for destructive command", tool);
                }
                BashCommandKind::ReadOnly => {}
            }
        }

        for (pattern_tool, regex) in &self.dangerous_regex_cache {
            if pattern_tool == tool && regex.is_match(&input_str) {
                return format!(
                    "Tool '{}' matches dangerous pattern: {}",
                    tool,
                    regex.as_str()
                );
            }
        }

        format!(
            "Tool '{}' requires approval in {} mode",
            tool,
            match self.mode {
                ApprovalMode::Auto => "auto",
                ApprovalMode::Ask => "ask",
                ApprovalMode::Strict => "strict",
            }
        )
    }

    /// Add a tool to the auto-approve list.
    pub fn add_auto_approve(&mut self, tool: &str) {
        if !self.auto_approve.iter().any(|t| t == tool) {
            self.auto_approve.push(tool.to_string());
        }
    }

    /// Remember one exact tool invocation as approved.
    pub fn add_scoped_auto_approve(&mut self, tool: &str, input: &Value) {
        let grant = ScopedApprovalGrant {
            tool: tool.to_string(),
            input_hash: input_hash(input),
        };
        if !self.scoped_auto_approve.contains(&grant) {
            self.scoped_auto_approve.push(grant);
        }
    }

    /// Add a tool pattern to the auto-approve list.
    /// Format: "tool" or "tool:pattern"
    pub fn add_auto_approve_pattern(&mut self, pattern: &str) {
        if !self.auto_approve.iter().any(|t| t == pattern) {
            self.auto_approve.push(pattern.to_string());
        }
    }

    /// Set the approval mode.
    pub fn set_mode(&mut self, mode: ApprovalMode) {
        self.mode = mode;
    }
}

fn input_hash(input: &Value) -> u64 {
    let mut hasher = DefaultHasher::new();
    input.to_string().hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_default() {
        let policy = Policy::new();
        assert_eq!(policy.mode, ApprovalMode::Ask);
        assert!(policy.auto_approve.contains(&"read_file".to_string()));
        assert!(policy.auto_approve.contains(&"todo".to_string()));
    }

    #[test]
    fn test_needs_approval_auto_mode() {
        let mut policy = Policy::new();
        policy.mode = ApprovalMode::Auto;

        // In auto mode, nothing needs approval
        assert!(!policy.needs_approval("bash", &serde_json::json!({"command": "rm -rf /"})));
    }

    #[test]
    fn test_needs_approval_strict_mode() {
        let mut policy = Policy::new();
        policy.mode = ApprovalMode::Strict;

        // In strict mode, everything needs approval except auto_approve list
        assert!(policy.needs_approval("bash", &serde_json::json!({"command": "ls"})));
        assert!(!policy.needs_approval("read_file", &serde_json::json!({"path": "test.txt"})));
    }

    #[test]
    fn test_dangerous_pattern() {
        let policy = Policy::new();

        // Sudo commands are dangerous
        assert!(policy.needs_approval("bash", &serde_json::json!({"command": "sudo apt install"})));

        // Writing to .env is dangerous
        assert!(policy.needs_approval(
            "write_file",
            &serde_json::json!({"path": ".env", "content": ""})
        ));

        // Normal commands are fine
        assert!(!policy.needs_approval("bash", &serde_json::json!({"command": "ls -la"})));
    }

    #[test]
    fn test_default_policy_blocks_rm_rf_root() {
        let policy = Policy::new();

        assert!(policy.needs_approval("bash", &serde_json::json!({"command": "rm -rf /"})));
    }

    #[test]
    fn test_auto_approve_read_tools() {
        let policy = Policy::new();

        // Read-only tools don't need approval
        assert!(!policy.needs_approval("read_file", &serde_json::json!({"path": "test.txt"})));
        assert!(!policy.needs_approval("glob", &serde_json::json!({"pattern": "*.rs"})));
        assert!(!policy.needs_approval("grep", &serde_json::json!({"pattern": "test"})));
        assert!(!policy.needs_approval("todo", &serde_json::json!({"items": []})));
    }

    #[test]
    fn test_bash_workspace_write_requires_approval() {
        let policy = Policy::new();

        assert!(policy.needs_approval("bash", &serde_json::json!({"command": "echo hi > out.txt"})));
        assert!(policy.needs_approval(
            "bash",
            &serde_json::json!({"command": "sed -i 's/old/new/' file.txt"})
        ));
    }

    #[test]
    fn test_bash_workspace_write_reason_is_specific() {
        let policy = Policy::new();

        let reason =
            policy.approval_reason("bash", &serde_json::json!({"command": "echo hi > out.txt"}));

        assert!(reason.contains("workspace-write"));
    }
}
