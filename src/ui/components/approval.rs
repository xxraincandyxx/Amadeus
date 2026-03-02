//! # Approval Dialog Component
//!
//! Displays an approval dialog for tool execution.

/// Response from an approval dialog.
#[derive(Debug, Clone)]
pub enum ApprovalResponse {
    /// Approve the tool execution.
    Approve,
    /// Deny the tool execution.
    Deny,
    /// Approve and remember for future (auto-approve).
    AlwaysApprove,
}

/// Approval dialog for tool execution.
pub struct ApprovalDialog {
    /// Tool name requiring approval.
    pub tool_name: String,
    /// Tool input.
    pub input: serde_json::Value,
    /// Reason why approval is needed.
    pub reason: String,
    /// Currently selected option.
    pub selected: usize,
}

impl ApprovalDialog {
    /// Create a new approval dialog.
    pub fn new(tool_name: &str, input: &serde_json::Value, reason: &str) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            input: input.clone(),
            reason: reason.to_string(),
            selected: 0,
        }
    }

    /// Get the options available.
    pub fn options(&self) -> [&'static str; 3] {
        ["Approve", "Deny", "Always Approve"]
    }

    /// Select the next option.
    pub fn select_next(&mut self) {
        self.selected = (self.selected + 1) % 3;
    }

    /// Select the previous option.
    pub fn select_previous(&mut self) {
        self.selected = if self.selected == 0 {
            2
        } else {
            self.selected - 1
        };
    }

    /// Get the selected response.
    pub fn get_response(&self) -> ApprovalResponse {
        match self.selected {
            0 => ApprovalResponse::Approve,
            1 => ApprovalResponse::Deny,
            2 => ApprovalResponse::AlwaysApprove,
            _ => ApprovalResponse::Approve,
        }
    }

    /// Render the dialog as simple text lines.
    pub fn render_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();

        lines.push("=== Tool Execution Approval Required ===".to_string());
        lines.push(String::new());
        lines.push(format!("Tool: {}", self.tool_name));
        lines.push(format!("Reason: {}", self.reason));
        lines.push(String::new());
        lines.push("Input:".to_string());

        let input_str = serde_json::to_string_pretty(&self.input).unwrap_or_default();
        let truncated = if input_str.len() > 200 {
            format!("{}...", &input_str[..200])
        } else {
            input_str
        };
        lines.push(truncated);
        lines.push(String::new());

        for (i, option) in self.options().iter().enumerate() {
            let line = format!(
                "{} {}",
                if i == self.selected { ">" } else { " " },
                option
            );
            lines.push(line);
        }

        lines.push(String::new());
        lines.push("↑/↓: Select  Enter: Confirm  Esc: Cancel".to_string());

        lines
    }
}
