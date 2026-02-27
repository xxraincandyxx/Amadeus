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

    for tool in &group.tools {
        if lines.len() >= max_total_lines {
            break;
        }

        let status_color = match tool.status {
            ToolStatus::Pending => THEME.orange,
            ToolStatus::Success => THEME.green,
            ToolStatus::Error => THEME.red,
        };

        let status_icon = match tool.status {
            ToolStatus::Pending => "◐",
            ToolStatus::Success => "✓",
            ToolStatus::Error => "✗",
        };

        // Header line
        lines.push(Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled(
                format!("{} ", status_icon),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                tool.name.to_uppercase(),
                Style::default().fg(THEME.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" tool", Style::default().fg(THEME.comment)),
        ]));

        if tool.status == ToolStatus::Pending && tool.output.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("   │ ", Style::default().fg(THEME.border)),
                Span::styled(
                    "Running...",
                    Style::default()
                        .fg(THEME.comment)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        } else if !tool.is_collapsed {
            if let Some(cmd) = &tool.command {
                lines.push(Line::from(vec![
                    Span::styled("   │ ", Style::default().fg(THEME.border)),
                    Span::styled("$ ", Style::default().fg(THEME.purple)),
                    Span::styled(cmd.clone(), Style::default().fg(THEME.cyan)),
                ]));
            }

            if !tool.output.is_empty() {
                let output_lines: Vec<&str> = tool.output.lines().take(10).collect();
                let total_output_lines = tool.output.lines().count();

                for line_content in output_lines {
                    if lines.len() >= max_total_lines {
                        break;
                    }
                    lines.push(Line::from(vec![
                        Span::styled("   │ ", Style::default().fg(THEME.border)),
                        Span::styled(line_content.to_string(), Style::default().fg(THEME.comment)),
                    ]));
                }

                if total_output_lines > 10 {
                    lines.push(Line::from(vec![
                        Span::styled("   │ ", Style::default().fg(THEME.border)),
                        Span::styled(
                            format!("... ({} more lines)", total_output_lines - 10),
                            Style::default()
                                .fg(THEME.comment)
                                .add_modifier(Modifier::DIM),
                        ),
                    ]));
                }
            }
        }

        lines.push(Line::from(""));
    }

    lines
}
