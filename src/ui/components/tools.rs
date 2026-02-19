use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::ui::colors::THEME;

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool_name: String,
    pub command: Option<String>,
    pub output: String,
    pub is_error: bool,
    pub is_collapsed: bool,
}

pub struct ToolPanel {
    results: Vec<ToolResult>,
    scroll_offset: usize,
}

impl ToolPanel {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
            scroll_offset: 0,
        }
    }

    pub fn add_result(&mut self, result: ToolResult) {
        self.results.push(result);
    }

    pub fn clear(&mut self) {
        self.results.clear();
        self.scroll_offset = 0;
    }

    pub fn toggle_collapse(&mut self, index: usize) {
        if let Some(result) = self.results.get_mut(index) {
            result.is_collapsed = !result.is_collapsed;
        }
    }

    pub fn collapse_all(&mut self) {
        for result in &mut self.results {
            result.is_collapsed = true;
        }
    }

    pub fn expand_all(&mut self) {
        for result in &mut self.results {
            result.is_collapsed = false;
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if self.results.is_empty() {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();
        let tool_border_color = if self.results.iter().any(|r| r.is_error) {
            THEME.red
        } else {
            THEME.green
        };

        for result in &self.results {
            let header_style = Style::default()
                .fg(tool_border_color)
                .add_modifier(Modifier::BOLD);

            let header = if result.is_collapsed {
                let status = if result.is_error { "✗" } else { "✓" };
                format!(
                    " {} {} {}",
                    status,
                    result.tool_name,
                    result.command.as_deref().unwrap_or("")
                )
            } else {
                format!(
                    " {} {}",
                    result.tool_name,
                    result.command.as_deref().unwrap_or("")
                )
            };

            lines.push(Line::from(vec![
                Span::styled("┌─ ", Style::default().fg(THEME.border)),
                Span::styled(header, header_style),
                Span::styled(" ─", Style::default().fg(THEME.border)),
            ]));

            if !result.is_collapsed {
                if let Some(cmd) = &result.command {
                    lines.push(Line::from(vec![
                        Span::styled("│ ", Style::default().fg(THEME.border)),
                        Span::styled("$ ", Style::default().fg(THEME.purple)),
                        Span::styled(cmd, Style::default().fg(THEME.cyan)),
                    ]));
                }

                let output_style = if result.is_error {
                    Style::default().fg(THEME.red)
                } else {
                    Style::default().fg(THEME.fg)
                };

                for line in result.output.lines().take(20) {
                    lines.push(Line::from(vec![
                        Span::styled("│ ", Style::default().fg(THEME.border)),
                        Span::styled(line, output_style),
                    ]));
                }

                if result.output.lines().count() > 20 {
                    lines.push(Line::from(vec![
                        Span::styled("│ ", Style::default().fg(THEME.border)),
                        Span::styled("... (truncated)", Style::default().fg(THEME.comment)),
                    ]));
                }
            }

            lines.push(Line::from(vec![Span::styled(
                "└──",
                Style::default().fg(THEME.border),
            )]));
        }

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::NONE)
                .style(Style::default().bg(THEME.bg)),
        );

        frame.render_widget(paragraph, area);
    }

    pub fn has_results(&self) -> bool {
        !self.results.is_empty()
    }
}

impl Default for ToolPanel {
    fn default() -> Self {
        Self::new()
    }
}
