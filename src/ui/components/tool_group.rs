use std::time::Instant;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolStatus {
    Pending,
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub command: Option<String>,
    pub output: String,
    pub status: ToolStatus,
    pub is_collapsed: bool,
    /// Progress message for long-running operations.
    pub progress_message: Option<String>,
    /// Progress percentage (0-100).
    pub progress_percent: Option<u8>,
}

impl ToolCall {
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            command: None,
            output: String::new(),
            status: ToolStatus::Pending,
            is_collapsed: false,
            progress_message: None,
            progress_percent: None,
        }
    }

    pub fn with_command(mut self, command: String) -> Self {
        self.command = Some(command);
        self
    }

    pub fn complete(mut self, output: String, is_error: bool) -> Self {
        self.output = output;
        self.status = if is_error {
            ToolStatus::Error
        } else {
            ToolStatus::Success
        };
        self.is_collapsed = false;
        self.progress_message = None;
        self.progress_percent = None;
        self
    }
}

/// Default number of tools to show before collapsing
const DEFAULT_COLLAPSE_THRESHOLD: usize = 5;

#[derive(Debug, Clone)]
pub struct ToolGroup {
    pub tools: Vec<ToolCall>,
    pub timestamp: Instant,
    /// Whether the group is expanded (showing all tools)
    pub is_expanded: bool,
    /// Number of tools to show before collapsing (0 = no collapsing)
    pub collapse_threshold: usize,
}

impl ToolGroup {
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            timestamp: Instant::now(),
            is_expanded: false,
            collapse_threshold: DEFAULT_COLLAPSE_THRESHOLD,
        }
    }

    /// Returns an iterator over the visible tools based on expansion state
    pub fn visible_tools(&self) -> impl Iterator<Item = &ToolCall> {
        let count = if self.is_expanded || self.collapse_threshold == 0 {
            self.tools.len()
        } else {
            self.collapse_threshold.min(self.tools.len())
        };
        self.tools.iter().take(count)
    }

    /// Returns the number of tools hidden by collapsing
    pub fn hidden_count(&self) -> usize {
        if self.is_expanded || self.collapse_threshold == 0 {
            0
        } else {
            self.tools.len().saturating_sub(self.collapse_threshold)
        }
    }

    /// Toggle expansion state
    pub fn toggle_expand(&mut self) {
        self.is_expanded = !self.is_expanded;
    }

    pub fn add_tool(&mut self, tool: ToolCall) {
        self.tools.push(tool);
    }

    pub fn update_tool(&mut self, id: &str, output: String, is_error: bool) {
        if let Some(tool) = self.tools.iter_mut().find(|t| t.id == id) {
            tool.output = output;
            tool.status = if is_error {
                ToolStatus::Error
            } else {
                ToolStatus::Success
            };
        }
    }

    pub fn has_pending(&self) -> bool {
        self.tools.iter().any(|t| t.status == ToolStatus::Pending)
    }

    pub fn all_collapsed(&self) -> bool {
        self.tools.iter().all(|t| t.is_collapsed)
    }

    pub fn collapse_all(&mut self) {
        for tool in &mut self.tools {
            tool.is_collapsed = true;
        }
    }

    pub fn expand_all(&mut self) {
        for tool in &mut self.tools {
            tool.is_collapsed = false;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolGroup {
    fn default() -> Self {
        Self::new()
    }
}

/// Truncate a string to a maximum length with ellipsis
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let keep = max_len.saturating_sub(3);
        let truncated: String = s.chars().take(keep).collect();
        format!("{truncated}...")
    }
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if text.is_empty() || max_width == 0 {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;

    for ch in text.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if current_width + ch_width > max_width && !current.is_empty() {
            lines.push(current);
            current = String::new();
            current_width = 0;
        }

        current.push(ch);
        current_width += ch_width;
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

pub fn render_tool_group_with_limit(
    group: &ToolGroup,
    area: Rect,
    max_total_lines: usize,
) -> Vec<Line<'static>> {
    let colors = crate::ui::theme_manager::get_colors();
    if group.tools.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let hidden_count = group.hidden_count();
    let available_width = area.width.max(16) as usize;
    let body_width = available_width.saturating_sub(6).max(8);
    let command_width = available_width.saturating_sub(8).max(8);

    for tool in group.visible_tools() {
        if lines.len() >= max_total_lines {
            break;
        }

        let status_color = match tool.status {
            ToolStatus::Pending => colors.status.warning,
            ToolStatus::Success => colors.status.success,
            ToolStatus::Error => colors.status.error,
        };

        // Hierarchical icon: filled for pending, empty for completed
        let icon = match tool.status {
            ToolStatus::Pending => "◉",
            ToolStatus::Success => "○",
            ToolStatus::Error => "○",
        };

        // Truncate command/args for preview
        let args_preview = tool
            .command
            .as_ref()
            .map(|c| truncate(c, 40))
            .unwrap_or_default();

        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                icon,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ", Style::default()),
            Span::styled(
                truncate(&tool.name, body_width.saturating_sub(2)),
                Style::default()
                    .fg(colors.text.primary)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        if !args_preview.is_empty() {
            for wrapped in wrap_text(&format!("({args_preview})"), body_width) {
                if lines.len() >= max_total_lines {
                    break;
                }
                lines.push(Line::from(vec![
                    Span::styled("    │ ", Style::default().fg(colors.border.default)),
                    Span::styled(wrapped, Style::default().fg(colors.ui.comment)),
                ]));
            }
        }

        if tool.status == ToolStatus::Pending && tool.output.is_empty() {
            // Show progress information if available
            if let Some(ref msg) = tool.progress_message {
                let progress_text = if let Some(percent) = tool.progress_percent {
                    format!("{} [{}%]", msg, percent)
                } else {
                    msg.clone()
                };
                for wrapped in wrap_text(&progress_text, body_width) {
                    if lines.len() >= max_total_lines {
                        break;
                    }
                    lines.push(Line::from(vec![
                        Span::styled("    │ ", Style::default().fg(colors.border.default)),
                        Span::styled(
                            wrapped,
                            Style::default()
                                .fg(colors.ui.symbol)
                                .add_modifier(Modifier::ITALIC),
                        ),
                    ]));
                }
            } else {
                lines.push(Line::from(vec![
                    Span::styled("    │ ", Style::default().fg(colors.border.default)),
                    Span::styled(
                        "Running...",
                        Style::default()
                            .fg(colors.ui.comment)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]));
            }
        } else if !tool.is_collapsed {
            if let Some(cmd) = &tool.command {
                for (idx, wrapped) in wrap_text(cmd, command_width).into_iter().enumerate() {
                    if lines.len() >= max_total_lines {
                        break;
                    }
                    let command_prefix = if idx == 0 { "$ " } else { "  " };
                    lines.push(Line::from(vec![
                        Span::styled("    │ ", Style::default().fg(colors.border.default)),
                        Span::styled(command_prefix, Style::default().fg(colors.text.accent)),
                        Span::styled(wrapped, Style::default().fg(colors.ui.symbol)),
                    ]));
                }
            }

            if !tool.output.is_empty() {
                let output_lines: Vec<&str> = tool.output.lines().take(10).collect();
                let total_output_lines = tool.output.lines().count();

                for line_content in output_lines {
                    for wrapped in wrap_text(line_content, body_width) {
                        if lines.len() >= max_total_lines {
                            break;
                        }
                        lines.push(Line::from(vec![
                            Span::styled("    │ ", Style::default().fg(colors.border.default)),
                            Span::styled(wrapped, Style::default().fg(colors.ui.comment)),
                        ]));
                    }
                    if lines.len() >= max_total_lines {
                        break;
                    }
                }

                if total_output_lines > 10 {
                    lines.push(Line::from(vec![
                        Span::styled("    │ ", Style::default().fg(colors.border.default)),
                        Span::styled(
                            format!("... ({} more lines)", total_output_lines - 10),
                            Style::default()
                                .fg(colors.ui.comment)
                                .add_modifier(Modifier::DIM),
                        ),
                    ]));
                }
            }
        }

        lines.push(Line::from(""));
    }

    // Add summary line if there are hidden tools
    if hidden_count > 0 {
        lines.push(Line::from(vec![
            Span::styled(
                format!("+{} more tool uses ", hidden_count),
                Style::default().fg(colors.ui.comment),
            ),
            Span::styled("(ctrl+o to expand)", Style::default().fg(colors.ui.symbol)),
        ]));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_group_new() {
        let group = ToolGroup::new();
        assert!(group.tools.is_empty());
        assert!(!group.is_expanded);
        assert_eq!(group.collapse_threshold, 5);
    }

    #[test]
    fn test_tool_group_hidden_count() {
        let mut group = ToolGroup::new();

        // No tools = no hidden
        assert_eq!(group.hidden_count(), 0);

        // Add 3 tools (below threshold)
        for i in 0..3 {
            group.add_tool(ToolCall::new(format!("tool_{}", i), "test".to_string()));
        }
        assert_eq!(group.hidden_count(), 0);

        // Add more tools (above threshold)
        for i in 3..8 {
            group.add_tool(ToolCall::new(format!("tool_{}", i), "test".to_string()));
        }
        // 8 tools, threshold 5 = 3 hidden
        assert_eq!(group.hidden_count(), 3);

        // When expanded, no hidden
        group.is_expanded = true;
        assert_eq!(group.hidden_count(), 0);
    }

    #[test]
    fn test_tool_group_visible_tools() {
        let mut group = ToolGroup::new();

        // Add 7 tools
        for i in 0..7 {
            group.add_tool(ToolCall::new(format!("tool_{}", i), "test".to_string()));
        }

        // Not expanded: should see only 5 (threshold)
        let visible: Vec<_> = group.visible_tools().collect();
        assert_eq!(visible.len(), 5);

        // Expanded: should see all 7
        group.is_expanded = true;
        let visible: Vec<_> = group.visible_tools().collect();
        assert_eq!(visible.len(), 7);
    }

    #[test]
    fn test_tool_group_toggle_expand() {
        let mut group = ToolGroup::new();
        assert!(!group.is_expanded);

        group.toggle_expand();
        assert!(group.is_expanded);

        group.toggle_expand();
        assert!(!group.is_expanded);
    }

    #[test]
    fn test_tool_group_zero_threshold() {
        let mut group = ToolGroup::new();
        group.collapse_threshold = 0;

        // Add tools
        for i in 0..10 {
            group.add_tool(ToolCall::new(format!("tool_{}", i), "test".to_string()));
        }

        // With threshold 0, all tools should be visible even when not expanded
        assert_eq!(group.hidden_count(), 0);
        let visible: Vec<_> = group.visible_tools().collect();
        assert_eq!(visible.len(), 10);
    }

    #[test]
    fn test_tool_call_new() {
        let tool = ToolCall::new("id123".to_string(), "bash".to_string());
        assert_eq!(tool.id, "id123");
        assert_eq!(tool.name, "bash");
        assert_eq!(tool.status, ToolStatus::Pending);
        assert!(!tool.is_collapsed);
        assert!(tool.command.is_none());
        assert!(tool.output.is_empty());
    }

    #[test]
    fn test_tool_call_with_command() {
        let tool =
            ToolCall::new("id".to_string(), "bash".to_string()).with_command("ls -la".to_string());
        assert_eq!(tool.command, Some("ls -la".to_string()));
    }

    #[test]
    fn test_tool_call_complete() {
        let tool = ToolCall::new("id".to_string(), "bash".to_string())
            .with_command("ls".to_string())
            .complete("file1\nfile2".to_string(), false);

        assert_eq!(tool.output, "file1\nfile2");
        assert_eq!(tool.status, ToolStatus::Success);
        assert!(!tool.is_collapsed);
    }

    #[test]
    fn test_tool_call_complete_error() {
        let tool = ToolCall::new("id".to_string(), "bash".to_string())
            .complete("command failed".to_string(), true);

        assert_eq!(tool.status, ToolStatus::Error);
    }

    #[test]
    fn test_tool_status_variants() {
        assert_ne!(ToolStatus::Pending, ToolStatus::Success);
        assert_ne!(ToolStatus::Success, ToolStatus::Error);
        assert_ne!(ToolStatus::Pending, ToolStatus::Error);
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("exactly10!", 10), "exactly10!");
        assert_eq!(truncate("this is a very long string", 10), "this is...");
    }

    #[test]
    fn test_truncate_handles_unicode_without_panicking() {
        assert_eq!(truncate("你好世界朋友", 5), "你好...");
    }

    #[test]
    fn test_render_tool_group_empty() {
        let group = ToolGroup::new();
        let lines = render_tool_group_with_limit(&group, Rect::default(), 100);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_render_tool_group_with_tools() {
        let mut group = ToolGroup::new();
        group.add_tool(
            ToolCall::new("t1".to_string(), "bash".to_string())
                .with_command("ls".to_string())
                .complete("file1\nfile2".to_string(), false),
        );

        let lines = render_tool_group_with_limit(&group, Rect::default(), 100);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_render_tool_group_with_hidden() {
        let mut group = ToolGroup::new();
        // Add 7 tools (threshold 5 = 2 hidden)
        for i in 0..7 {
            group.add_tool(
                ToolCall::new(format!("t{}", i), "test".to_string())
                    .complete(format!("output {}", i), false),
            );
        }

        let lines = render_tool_group_with_limit(&group, Rect::default(), 100);

        // Should include summary line with "+2 more tool uses"
        let has_summary = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.content.contains("+2 more tool uses"))
        });
        assert!(has_summary);
    }

    #[test]
    fn test_render_tool_group_expanded() {
        let mut group = ToolGroup::new();
        group.is_expanded = true;

        // Add 7 tools
        for i in 0..7 {
            group.add_tool(
                ToolCall::new(format!("t{}", i), "test".to_string())
                    .complete(format!("output {}", i), false),
            );
        }

        let lines = render_tool_group_with_limit(&group, Rect::default(), 100);

        // Should NOT include summary line when expanded
        let has_summary = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.content.contains("more tool uses"))
        });
        assert!(!has_summary);
    }

    #[test]
    fn test_render_tool_group_wraps_long_command_and_output() {
        let mut group = ToolGroup::new();
        group.add_tool(
            ToolCall::new("t1".to_string(), "bash".to_string())
                .with_command("git diff -- src/ui/app.rs src/ui/components/messages.rs".to_string())
                .complete(
                    "diff --git a/.github/pull_request_template.md b/.github/pull_request_template.md".to_string(),
                    false,
                ),
        );

        let area = Rect::new(0, 0, 24, 10);
        let lines = render_tool_group_with_limit(&group, area, 100);

        assert!(lines.len() > 4);
        assert!(lines.iter().all(|line| line.width() <= area.width as usize));
    }
}
