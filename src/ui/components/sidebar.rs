use std::path::PathBuf;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use walkdir::WalkDir;

use crate::ui::colors::THEME;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SidebarKind {
    Files,
    Help,
}

pub enum Sidebar {
    Files(FileSidebar),
    Help(HelpSidebar),
}

pub struct FileSidebar {
    workdir: PathBuf,
    entries: Vec<FileEntry>,
    selected: usize,
    scroll_offset: usize,
}

#[derive(Debug, Clone)]
struct FileEntry {
    path: String,
    is_dir: bool,
    depth: usize,
}

impl FileSidebar {
    pub fn new(workdir: PathBuf) -> Self {
        let mut sidebar = Self {
            workdir,
            entries: Vec::new(),
            selected: 0,
            scroll_offset: 0,
        };
        sidebar.refresh();
        sidebar
    }

    pub fn refresh(&mut self) {
        self.entries.clear();
        let max_depth = 3;
        let max_entries = 100;

        for entry in WalkDir::new(&self.workdir)
            .min_depth(0)
            .max_depth(max_depth)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let path = e.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                !name.starts_with('.') && !name.starts_with("target")
            })
            .take(max_entries)
        {
            let path = entry.path();
            let relative = path.strip_prefix(&self.workdir).unwrap_or(path);
            let depth = relative.components().count().saturating_sub(1);

            self.entries.push(FileEntry {
                path: relative.display().to_string(),
                is_dir: entry.file_type().is_dir(),
                depth,
            });
        }
    }

    pub fn select_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.ensure_selected_visible(0);
        }
    }

    pub fn select_down(&mut self, visible_count: usize) {
        if self.selected < self.entries.len().saturating_sub(1) {
            self.selected += 1;
            self.ensure_selected_visible(visible_count);
        }
    }

    fn ensure_selected_visible(&mut self, visible_count: usize) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + visible_count && visible_count > 0 {
            self.scroll_offset = self.selected - visible_count + 1;
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width < 5 {
            return;
        }

        let content_width = area.width.saturating_sub(2) as usize;
        let indent_str = "  ";
        let visible_count = area.height.saturating_sub(2) as usize;

        let items: Vec<ListItem> = self
            .entries
            .iter()
            .skip(self.scroll_offset)
            .take(visible_count)
            .enumerate()
            .map(|(i, entry)| {
                let actual_index = i + self.scroll_offset;
                let indent = indent_str.repeat(entry.depth);
                let icon = if entry.is_dir { "📁" } else { "📄" };
                let name = entry.path.split('/').next_back().unwrap_or(&entry.path);

                let available_width =
                    content_width.saturating_sub(indent.width() + icon.width() + 3);
                let truncated_name = if name.width() > available_width {
                    let mut result = String::new();
                    let mut width = 0;
                    for c in name.chars() {
                        let char_width = c.width().unwrap_or(0);
                        if width + char_width + 3 > available_width {
                            break;
                        }
                        result.push(c);
                        width += char_width;
                    }
                    format!("{}…", result)
                } else {
                    name.to_string()
                };

                let style = if actual_index == self.selected {
                    Style::default()
                        .fg(THEME.cyan)
                        .bg(THEME.selection)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(THEME.fg)
                };

                ListItem::new(Line::from(vec![
                    Span::styled(indent, style),
                    Span::styled(format!(" {} ", icon), Style::default().fg(THEME.purple)),
                    Span::styled(truncated_name, style),
                ]))
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .title(" EXPLORER ")
                .title_style(
                    Style::default()
                        .fg(THEME.comment)
                        .add_modifier(Modifier::BOLD),
                )
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(THEME.border))
                .style(Style::default().bg(THEME.bg)),
        );

        frame.render_widget(list, area);
    }
}

pub struct HelpSidebar;

impl HelpSidebar {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width < 10 {
            return;
        }

        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(" ❯ ", Style::default().fg(THEME.purple)),
                Span::styled("SHORTCUTS", Style::default().fg(THEME.fg).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("   Enter ", Style::default().fg(THEME.cyan)),
                Span::styled(" Send", Style::default().fg(THEME.comment)),
            ]),
            Line::from(vec![
                Span::styled("   A-Enter ", Style::default().fg(THEME.cyan)),
                Span::styled(" New Line", Style::default().fg(THEME.comment)),
            ]),
            Line::from(vec![
                Span::styled("   Up/Down ", Style::default().fg(THEME.cyan)),
                Span::styled(" History", Style::default().fg(THEME.comment)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" ❯ ", Style::default().fg(THEME.purple)),
                Span::styled("SIDEBAR", Style::default().fg(THEME.fg).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("   ^B ", Style::default().fg(THEME.cyan)),
                Span::styled(" Files", Style::default().fg(THEME.comment)),
            ]),
            Line::from(vec![
                Span::styled("   !H ", Style::default().fg(THEME.cyan)),
                Span::styled(" Help", Style::default().fg(THEME.comment)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" ❯ ", Style::default().fg(THEME.purple)),
                Span::styled("SYSTEM", Style::default().fg(THEME.fg).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("   Esc ", Style::default().fg(THEME.cyan)),
                Span::styled(" Collapse", Style::default().fg(THEME.comment)),
            ]),
            Line::from(vec![
                Span::styled("   ^C ", Style::default().fg(THEME.cyan)),
                Span::styled(" Exit", Style::default().fg(THEME.comment)),
            ]),
        ];

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .title(" COMMANDS ")
                .title_style(
                    Style::default()
                        .fg(THEME.comment)
                        .add_modifier(Modifier::BOLD),
                )
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(THEME.border))
                .style(Style::default().bg(THEME.bg)),
        );

        frame.render_widget(paragraph, area);
    }
}

impl Default for HelpSidebar {
    fn default() -> Self {
        Self::new()
    }
}
