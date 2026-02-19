use std::time::Instant;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::ui::colors::THEME;

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
        self
    }
}

#[derive(Debug, Clone)]
pub struct ToolGroup {
    pub tools: Vec<ToolCall>,
    pub timestamp: Instant,
}

impl ToolGroup {
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            timestamp: Instant::now(),
        }
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

pub fn render_tool_group_with_limit(
    group: &ToolGroup,
    _area: Rect,
    max_total_lines: usize,
) -> Vec<Line<'static>> {
    if group.tools.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();

    let tools_with_output: Vec<&ToolCall> = group
        .tools
        .iter()
        .filter(|t| !t.output.is_empty() || t.status == ToolStatus::Pending)
        .collect();

    let tools_one_line = group.tools.len().saturating_sub(tools_with_output.len());

    let border_overhead = 2;
    let available_for_output = max_total_lines
        .saturating_sub(border_overhead + tools_one_line)
        .max(1);

    let lines_per_tool = if tools_with_output.is_empty() {
        1
    } else {
        (available_for_output / tools_with_output.len()).max(1)
    };

    for tool in &group.tools {
        if lines.len() >= max_total_lines {
            break;
        }

        let border_color = match tool.status {
            ToolStatus::Pending => THEME.orange,
            ToolStatus::Success => THEME.green,
            ToolStatus::Error => THEME.red,
        };

        let header_style = Style::default()
            .fg(border_color)
            .add_modifier(Modifier::BOLD);

        let status_icon = match tool.status {
            ToolStatus::Pending => "◐",
            ToolStatus::Success => "✓",
            ToolStatus::Error => "✗",
        };

        let header_text = if tool.is_collapsed {
            format!(
                " {} {} {}",
                status_icon,
                tool.name,
                tool.command.as_deref().unwrap_or("")
            )
        } else {
            format!(" {} {}", tool.name, tool.command.as_deref().unwrap_or(""))
        };

        lines.push(Line::from(vec![
            Span::styled("┌─ ".to_string(), Style::default().fg(THEME.border)),
            Span::styled(header_text, header_style),
            Span::styled(" ─".to_string(), Style::default().fg(THEME.border)),
        ]));

        if tool.status == ToolStatus::Pending && tool.output.is_empty() {
            if lines.len() < max_total_lines {
                lines.push(Line::from(vec![
                    Span::styled("│ ".to_string(), Style::default().fg(THEME.border)),
                    Span::styled("Running...".to_string(), Style::default().fg(THEME.comment)),
                ]));
            }
        } else if !tool.is_collapsed && !tool.output.is_empty() {
            let output_style = if tool.status == ToolStatus::Error {
                Style::default().fg(THEME.red)
            } else {
                Style::default().fg(THEME.fg)
            };

            if let Some(cmd) = &tool.command {
                if lines.len() < max_total_lines {
                    lines.push(Line::from(vec![
                        Span::styled("│ ".to_string(), Style::default().fg(THEME.border)),
                        Span::styled("$ ".to_string(), Style::default().fg(THEME.purple)),
                        Span::styled(cmd.clone(), Style::default().fg(THEME.cyan)),
                    ]));
                }
            }

            let remaining = max_total_lines.saturating_sub(lines.len() + 1);
            let max_output_lines = remaining.min(lines_per_tool);
            let output_lines: Vec<&str> = tool.output.lines().take(max_output_lines).collect();
            let total_output_lines = tool.output.lines().count();

            for line_content in output_lines {
                if lines.len() >= max_total_lines {
                    break;
                }
                lines.push(Line::from(vec![
                    Span::styled("│ ".to_string(), Style::default().fg(THEME.border)),
                    Span::styled(line_content.to_string(), output_style),
                ]));
            }

            if total_output_lines > max_output_lines && lines.len() < max_total_lines {
                let remaining_count = total_output_lines.saturating_sub(max_output_lines);
                lines.push(Line::from(vec![
                    Span::styled("│ ".to_string(), Style::default().fg(THEME.border)),
                    Span::styled(
                        format!("... ({} more lines)", remaining_count),
                        Style::default().fg(THEME.comment),
                    ),
                ]));
            }
        }

        if lines.len() < max_total_lines {
            lines.push(Line::from(vec![Span::styled(
                "└──".to_string(),
                Style::default().fg(THEME.border),
            )]));
        }
    }

    lines
}
