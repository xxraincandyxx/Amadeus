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

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self, visible_count: usize) {
        let max_scroll = self.entries.len().saturating_sub(visible_count);
        self.scroll_offset = self.scroll_offset.saturating_add(1).min(max_scroll);
    }

    pub fn selected_path(&self) -> Option<&str> {
        self.entries.get(self.selected).map(|e| e.path.as_str())
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
                let icon = if entry.is_dir { "📁 " } else { "📄 " };
                let name = entry.path.split('/').next_back().unwrap_or(&entry.path);

                let available_width =
                    content_width.saturating_sub(indent.width() + icon.width() + 2);
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
                    Span::raw(" "),
                    Span::styled(indent, style),
                    Span::styled(icon, style),
                    Span::styled(truncated_name, style),
                ]))
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .title(" Files ")
                .title_style(
                    Style::default()
                        .fg(THEME.purple)
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
            Line::from(Span::styled(
                " Shortcuts",
                Style::default()
                    .fg(THEME.purple)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::raw(" "),
                Span::styled("Enter", Style::default().fg(THEME.cyan)),
                Span::styled("  Send message", Style::default().fg(THEME.fg)),
            ]),
            Line::from(vec![
                Span::raw(" "),
                Span::styled("Ctrl+↵", Style::default().fg(THEME.cyan)),
                Span::styled("  New line", Style::default().fg(THEME.fg)),
            ]),
            Line::from(vec![
                Span::raw(" "),
                Span::styled("↑/↓", Style::default().fg(THEME.cyan)),
                Span::styled("    History", Style::default().fg(THEME.fg)),
            ]),
            Line::from(vec![
                Span::raw(" "),
                Span::styled("PgUp", Style::default().fg(THEME.cyan)),
                Span::styled("  Scroll up", Style::default().fg(THEME.fg)),
            ]),
            Line::from(vec![
                Span::raw(" "),
                Span::styled("PgDn", Style::default().fg(THEME.cyan)),
                Span::styled("  Scroll down", Style::default().fg(THEME.fg)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                " Sidebar",
                Style::default()
                    .fg(THEME.purple)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::raw(" "),
                Span::styled("⌘B", Style::default().fg(THEME.cyan)),
                Span::styled("   File tree", Style::default().fg(THEME.fg)),
            ]),
            Line::from(vec![
                Span::raw(" "),
                Span::styled("⌥B", Style::default().fg(THEME.cyan)),
                Span::styled("   This help", Style::default().fg(THEME.fg)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                " General",
                Style::default()
                    .fg(THEME.purple)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::raw(" "),
                Span::styled("Esc", Style::default().fg(THEME.cyan)),
                Span::styled("   Collapse", Style::default().fg(THEME.fg)),
            ]),
            Line::from(vec![
                Span::raw(" "),
                Span::styled("q", Style::default().fg(THEME.cyan)),
                Span::styled("     Exit", Style::default().fg(THEME.fg)),
            ]),
        ];

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .title(" Help ")
                .title_style(
                    Style::default()
                        .fg(THEME.purple)
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
