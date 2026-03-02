//! # Approval Dialog Component
//!
//! Displays an approval dialog for tool execution.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::ui::get_colors;

/// Response from an approval dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Unique ID for this approval request.
    pub request_id: String,
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
            request_id: uuid::Uuid::new_v4().to_string(),
            tool_name: tool_name.to_string(),
            input: input.clone(),
            reason: reason.to_string(),
            selected: 0,
        }
    }

    /// Create a new approval dialog with a specific request ID.
    pub fn with_id(id: String, tool_name: &str, input: &serde_json::Value, reason: &str) -> Self {
        Self {
            request_id: id,
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

    /// Render the dialog as a floating modal.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let colors = get_colors();

        // Calculate dialog size
        let dialog_width = (area.width.saturating_sub(4)).min(70).max(40);
        let dialog_height = 14u16; // Fixed height for simplicity

        // Center the dialog
        let dialog_x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
        let dialog_y = area.y + (area.height.saturating_sub(dialog_height)) / 2;

        let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

        // Clear the area first
        frame.render_widget(Clear, dialog_area);

        // Build the dialog content
        let mut lines = Vec::new();

        // Title
        lines.push(Line::from(Span::styled(
            " Approval Required ",
            Style::default()
                .fg(colors.status.warning)
                .add_modifier(Modifier::BOLD),
        )));

        lines.push(Line::default());

        // Tool info
        lines.push(Line::from(vec![
            Span::styled("Tool: ", Style::default().fg(colors.text.secondary)),
            Span::styled(&self.tool_name, Style::default().fg(colors.text.accent)),
        ]));

        // Reason (truncate if too long)
        let reason_text = if self.reason.len() > 50 {
            format!("{}...", &self.reason[..47])
        } else {
            self.reason.clone()
        };
        lines.push(Line::from(vec![
            Span::styled("Reason: ", Style::default().fg(colors.text.secondary)),
            Span::styled(reason_text, Style::default().fg(colors.status.warning)),
        ]));

        lines.push(Line::default());

        // Options
        for (i, option) in self.options().iter().enumerate() {
            let (prefix, style) = if i == self.selected {
                (
                    "► ",
                    Style::default()
                        .fg(colors.text.primary)
                        .bg(colors.ui.dark)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                (
                    "  ",
                    Style::default().fg(colors.text.secondary),
                )
            };
            lines.push(Line::from(Span::styled(
                format!("{}{}", prefix, option),
                style,
            )));
        }

        lines.push(Line::default());

        // Help text
        lines.push(Line::from(Span::styled(
            "↑/↓: Select  y: Yes  n: No  a: Always  Esc: Cancel",
            Style::default().fg(colors.ui.comment),
        )));

        // Render as bordered paragraph
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(colors.status.warning))
            .style(Style::default().bg(colors.background.primary));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, dialog_area);
    }

    /// Render the dialog as simple text lines (for non-TUI use).
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
