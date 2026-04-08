// @amadeus-header
// summary: Generic modal dialog for slash-command selection flows.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::components::slash_dialog
// - type: crate::ui::components::slash_dialog::SlashDialog
// - type: crate::ui::components::slash_dialog::SlashDialogItem
// uses:
// - module: crate::ui::get_colors
// - runtime: ratatui terminal rendering
// invariants:
// - Dialog selection order remains stable for keyboard navigation.
// side_effects: none
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

use crate::ui::get_colors;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashDialogItem {
    pub label: String,
    pub detail: Option<String>,
}

impl SlashDialogItem {
    pub fn new(label: impl Into<String>, detail: Option<String>) -> Self {
        Self {
            label: label.into(),
            detail,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SlashDialog {
    title: String,
    subtitle: Option<String>,
    details: Vec<String>,
    footer: String,
    items: Vec<SlashDialogItem>,
    selected: usize,
}

impl SlashDialog {
    pub fn new(
        title: impl Into<String>,
        subtitle: Option<String>,
        details: Vec<String>,
        footer: impl Into<String>,
        items: Vec<SlashDialogItem>,
    ) -> Self {
        Self {
            title: title.into(),
            subtitle,
            details,
            footer: footer.into(),
            items,
            selected: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn selected(&self) -> Option<usize> {
        if self.items.is_empty() {
            None
        } else {
            Some(self.selected.min(self.items.len().saturating_sub(1)))
        }
    }

    pub fn select_previous(&mut self) {
        if !self.items.is_empty() {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let colors = get_colors();
        let dialog_width = area.width.saturating_sub(4).clamp(40, 76);
        let content_height = 8u16
            .saturating_add(self.details.len() as u16)
            .saturating_add((self.items.len().min(8) * 2) as u16);
        let dialog_height = content_height.clamp(10, area.height.saturating_sub(2).max(10));
        let dialog_x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
        let dialog_y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
        let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

        frame.render_widget(Clear, dialog_area);

        let mut lines = Vec::new();
        lines.push(Line::from(Span::styled(
            self.title.clone(),
            Style::default()
                .fg(colors.text.primary)
                .add_modifier(Modifier::BOLD),
        )));

        if let Some(subtitle) = &self.subtitle {
            lines.push(Line::from(Span::styled(
                subtitle.clone(),
                Style::default().fg(colors.text.secondary),
            )));
        }

        if !self.details.is_empty() {
            lines.push(Line::from(""));
            for detail in &self.details {
                lines.push(Line::from(Span::styled(
                    detail.clone(),
                    Style::default().fg(colors.text.secondary),
                )));
            }
        }

        if !self.items.is_empty() {
            lines.push(Line::from(""));
            for (index, item) in self.items.iter().enumerate().take(8) {
                let selected = self.selected == index;
                let prefix = if selected { "❯ " } else { "  " };
                let line_style = if selected {
                    Style::default()
                        .fg(colors.text.primary)
                        .bg(colors.background.message)
                } else {
                    Style::default().fg(colors.text.secondary)
                };
                lines.push(Line::from(Span::styled(
                    format!("{}{}. {}", prefix, index + 1, item.label),
                    line_style,
                )));
                if let Some(detail) = &item.detail {
                    lines.push(Line::from(Span::styled(
                        format!("   {}", detail),
                        Style::default().fg(colors.ui.comment),
                    )));
                }
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            self.footer.clone(),
            Style::default().fg(colors.text.accent),
        )));

        frame.render_widget(Paragraph::new(lines), dialog_area);
    }
}
