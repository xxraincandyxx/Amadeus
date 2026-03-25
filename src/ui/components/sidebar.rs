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

use crate::skills::Skill;
use crate::ui::get_colors;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SidebarKind {
    Files,
    Help,
    Skills,
    Context,
}

pub enum Sidebar {
    Files(FileSidebar),
    Help(HelpSidebar),
    Skills(SkillSidebar),
    Context(ContextSidebar),
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

        let colors = get_colors();
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
                let icon = if entry.is_dir { "📂" } else { "📄" };
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
                    Style::default().fg(colors.text.link).bg(colors.ui.dark)
                } else {
                    Style::default().fg(colors.text.primary)
                };

                ListItem::new(Line::from(vec![
                    Span::styled(indent, style),
                    Span::styled(
                        format!(" {} ", icon),
                        Style::default().fg(colors.text.accent),
                    ),
                    Span::styled(truncated_name, style),
                ]))
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .title(" EXPLORER ")
                .title_style(Style::default().fg(colors.ui.comment))
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(colors.border.default))
                .style(Style::default().bg(colors.background.primary)),
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

        let colors = get_colors();

        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(" ❯ ", Style::default().fg(colors.text.accent)),
                Span::styled("SHORTCUTS", Style::default().fg(colors.text.primary)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("   Enter ", Style::default().fg(colors.text.link)),
                Span::styled(" Send", Style::default().fg(colors.ui.comment)),
            ]),
            Line::from(vec![
                Span::styled("   A-Enter ", Style::default().fg(colors.text.link)),
                Span::styled(" New Line", Style::default().fg(colors.ui.comment)),
            ]),
            Line::from(vec![
                Span::styled("   Up/Down ", Style::default().fg(colors.text.link)),
                Span::styled(" History", Style::default().fg(colors.ui.comment)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" ❯ ", Style::default().fg(colors.text.accent)),
                Span::styled("SIDEBAR", Style::default().fg(colors.text.primary)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("   ^[Shift]B ", Style::default().fg(colors.text.link)),
                Span::styled(" Files", Style::default().fg(colors.ui.comment)),
            ]),
            Line::from(vec![
                Span::styled("   !B ", Style::default().fg(colors.text.link)),
                Span::styled(" Help", Style::default().fg(colors.ui.comment)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" ❯ ", Style::default().fg(colors.text.accent)),
                Span::styled("TOOLS", Style::default().fg(colors.text.primary)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("   ^O ", Style::default().fg(colors.text.link)),
                Span::styled(" Expand Tools", Style::default().fg(colors.ui.comment)),
            ]),
            Line::from(vec![
                Span::styled("   ^X i/k ", Style::default().fg(colors.text.link)),
                Span::styled(" Monitor Prev/Next", Style::default().fg(colors.ui.comment)),
            ]),
            Line::from(vec![
                Span::styled("   ^X j/l ", Style::default().fg(colors.text.link)),
                Span::styled(
                    " Monitor Back/Enter",
                    Style::default().fg(colors.ui.comment),
                ),
            ]),
            Line::from(vec![
                Span::styled("   ^[Alt]B ", Style::default().fg(colors.text.link)),
                Span::styled(" Background", Style::default().fg(colors.ui.comment)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" ❯ ", Style::default().fg(colors.text.accent)),
                Span::styled("THEMES", Style::default().fg(colors.text.primary)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("   ^T ", Style::default().fg(colors.text.link)),
                Span::styled(" Switch Theme", Style::default().fg(colors.ui.comment)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" ❯ ", Style::default().fg(colors.text.accent)),
                Span::styled("CONTEXT", Style::default().fg(colors.text.primary)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("   ^K ", Style::default().fg(colors.text.link)),
                Span::styled(" Compact History", Style::default().fg(colors.ui.comment)),
            ]),
            Line::from(vec![
                Span::styled("   /compact ", Style::default().fg(colors.text.link)),
                Span::styled(
                    " Compact via command",
                    Style::default().fg(colors.ui.comment),
                ),
            ]),
            Line::from(vec![
                Span::styled("   /context ", Style::default().fg(colors.text.link)),
                Span::styled(
                    " Context usage info",
                    Style::default().fg(colors.ui.comment),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" ❯ ", Style::default().fg(colors.text.accent)),
                Span::styled("SCROLLING", Style::default().fg(colors.text.primary)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("   Shift+↑/↓ ", Style::default().fg(colors.text.link)),
                Span::styled(" Line by Line", Style::default().fg(colors.ui.comment)),
            ]),
            Line::from(vec![
                Span::styled("   PgUp/PgDn ", Style::default().fg(colors.text.link)),
                Span::styled(" Page Scroll", Style::default().fg(colors.ui.comment)),
            ]),
            Line::from(vec![
                Span::styled("   ^Home/^End ", Style::default().fg(colors.text.link)),
                Span::styled(" Top/Bottom", Style::default().fg(colors.ui.comment)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" ❯ ", Style::default().fg(colors.text.accent)),
                Span::styled("SYSTEM", Style::default().fg(colors.text.primary)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("   Esc ", Style::default().fg(colors.text.link)),
                Span::styled(" Collapse", Style::default().fg(colors.ui.comment)),
            ]),
            Line::from(vec![
                Span::styled("   ^C ", Style::default().fg(colors.text.link)),
                Span::styled(" Exit", Style::default().fg(colors.ui.comment)),
            ]),
        ];

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .title(" COMMANDS ")
                .title_style(Style::default().fg(colors.ui.comment))
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(colors.border.default))
                .style(Style::default().bg(colors.background.primary)),
        );

        frame.render_widget(paragraph, area);
    }
}

impl Default for HelpSidebar {
    fn default() -> Self {
        Self::new()
    }
}

/// Sidebar for selecting skills/prompt templates.
pub struct SkillSidebar {
    skills: Vec<Skill>,
    selected: usize,
    scroll_offset: usize,
}

impl SkillSidebar {
    /// Create a new skill sidebar with the given skills.
    pub fn new(skills: Vec<Skill>) -> Self {
        Self {
            skills,
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Get the number of skills.
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Check if there are no skills.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Move selection up.
    pub fn select_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.ensure_selected_visible(0);
        }
    }

    /// Move selection down.
    pub fn select_down(&mut self, visible_count: usize) {
        if self.selected < self.skills.len().saturating_sub(1) {
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

    /// Get the currently selected skill.
    pub fn selected_skill(&self) -> Option<&Skill> {
        self.skills.get(self.selected)
    }

    /// Render the skill sidebar.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width < 10 {
            return;
        }

        let colors = get_colors();
        let visible_count = area.height.saturating_sub(4) as usize;

        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(" ❯ ", Style::default().fg(colors.text.accent)),
                Span::styled("SKILLS", Style::default().fg(colors.text.primary)),
            ]),
            Line::from(""),
        ];

        if self.skills.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "   No skills available",
                Style::default()
                    .fg(colors.ui.comment)
                    .add_modifier(Modifier::ITALIC),
            )]));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "   Add .md files to",
                Style::default().fg(colors.ui.comment),
            )]));
            lines.push(Line::from(vec![Span::styled(
                "   .amadeus/skills/",
                Style::default().fg(colors.text.link),
            )]));
        } else {
            for (i, skill) in self
                .skills
                .iter()
                .skip(self.scroll_offset)
                .take(visible_count)
                .enumerate()
            {
                let actual_index = i + self.scroll_offset;
                let is_selected = actual_index == self.selected;

                let style = if is_selected {
                    Style::default().fg(colors.text.link).bg(colors.ui.dark)
                } else {
                    Style::default().fg(colors.text.primary)
                };

                let prefix = if is_selected { " ▸ " } else { "   " };

                // Skill name line
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(colors.text.accent)),
                    Span::styled(&skill.name, style),
                ]));

                // Description line (truncated)
                let max_desc_len = (area.width as usize).saturating_sub(6);
                let desc = if skill.description.len() > max_desc_len {
                    format!(
                        "{}...",
                        &skill.description[..max_desc_len.saturating_sub(3)]
                    )
                } else {
                    skill.description.clone()
                };
                lines.push(Line::from(vec![Span::styled(
                    format!("     {}", desc),
                    Style::default().fg(colors.ui.comment),
                )]));
            }
        }

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .title(" SKILLS ")
                .title_style(Style::default().fg(colors.ui.comment))
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(colors.border.default))
                .style(Style::default().bg(colors.background.primary)),
        );

        frame.render_widget(paragraph, area);
    }
}

/// Data used to populate the context sidebar.
#[derive(Debug, Clone)]
pub struct ContextInfo {
    pub model_name: String,
    pub context_window_size: u32,
    pub total_tokens: usize,
    /// System prompt tokens
    pub system_prompt_tokens: usize,
    /// Tool definitions tokens
    pub tools_tokens: usize,
    /// Conversation history tokens
    pub conversation_tokens: usize,
    /// Per-tool token estimates (name, estimated schema tokens)
    pub tool_details: Vec<(String, usize)>,
    /// Per-message token estimates (role, estimated tokens)
    pub message_details: Vec<(String, usize)>,
}

impl ContextInfo {
    /// Estimate total tokens for the context info.
    pub fn used_tokens(&self) -> usize {
        self.system_prompt_tokens + self.tools_tokens + self.conversation_tokens
    }

    /// Free space remaining in the context window.
    pub fn free_tokens(&self) -> usize {
        (self.context_window_size as usize).saturating_sub(self.used_tokens())
    }

    /// Usage percentage (0-100).
    pub fn usage_percent(&self) -> u8 {
        if self.context_window_size == 0 {
            return 0;
        }
        let pct = (self.used_tokens() as f64 / self.context_window_size as f64 * 100.0) as u8;
        pct.min(100)
    }

    /// Percentage of the context window used by a token count.
    fn pct_of(&self, tokens: usize) -> f64 {
        if self.context_window_size == 0 {
            return 0.0;
        }
        tokens as f64 / self.context_window_size as f64 * 100.0
    }

    /// Format a token count for display.
    fn fmt_tokens(n: usize) -> String {
        if n >= 1_000_000 {
            format!("{:.1}M", n as f64 / 1_000_000.0)
        } else if n >= 1000 {
            format!("{:.1}k", n as f64 / 1000.0)
        } else {
            n.to_string()
        }
    }
}

/// Sidebar showing context window usage breakdown.
pub struct ContextSidebar {
    info: ContextInfo,
}

impl ContextSidebar {
    pub fn new(info: ContextInfo) -> Self {
        Self { info }
    }

    fn render_bar(&self, width: u16) -> Line<'static> {
        let bar_width = (width as usize).min(24);
        let pct = self.info.usage_percent();
        let filled = ((pct as usize) * bar_width) / 100;
        let empty = bar_width.saturating_sub(filled);

        let fill_style = if pct >= 90 {
            Style::default().fg(ratatui::style::Color::Rgb(220, 50, 47))
        } else if pct >= 70 {
            Style::default().fg(ratatui::style::Color::Rgb(181, 137, 0))
        } else {
            Style::default().fg(ratatui::style::Color::Rgb(133, 153, 0))
        };

        let mut spans = vec![
            Span::styled("█".repeat(filled), fill_style),
            Span::styled(
                "░".repeat(empty),
                Style::default().fg(ratatui::style::Color::Rgb(88, 88, 88)),
            ),
        ];

        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("{}%", pct),
            if pct >= 90 {
                Style::default().fg(ratatui::style::Color::Rgb(220, 50, 47))
            } else if pct >= 70 {
                Style::default().fg(ratatui::style::Color::Rgb(181, 137, 0))
            } else {
                Style::default().fg(ratatui::style::Color::Rgb(147, 161, 161))
            },
        ));

        Line::from(spans)
    }

    fn section_line(colors: &crate::ui::semantic_colors::SemanticColors) -> Line<'static> {
        Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled("---", Style::default().fg(colors.ui.dark)),
        ])
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width < 16 {
            return;
        }

        let colors = get_colors();
        let content_width = area.width.saturating_sub(2) as usize;
        let visible_lines = area.height.saturating_sub(2) as usize;
        let mut lines: Vec<Line> = Vec::new();

        // Title + model
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(" ◈ ", Style::default().fg(colors.text.accent)),
            Span::styled(
                &self.info.model_name,
                Style::default().fg(colors.text.primary),
            ),
        ]));
        lines.push(Line::from(""));

        // Usage bar
        let used = ContextInfo::fmt_tokens(self.info.used_tokens());
        let total = ContextInfo::fmt_tokens(self.info.context_window_size as usize);
        lines.push(Line::from(vec![Span::styled(
            format!(" {} / {} tokens", used, total),
            Style::default().fg(colors.text.secondary),
        )]));
        lines.push(self.render_bar(area.width.saturating_sub(2)));
        lines.push(Line::from(""));

        // Breakdown sections
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                "ESTIMATED USAGE BY CATEGORY",
                Style::default().fg(colors.ui.comment),
            ),
        ]));
        lines.push(Self::section_line(&colors));

        // System prompt
        let sys_pct = self.info.pct_of(self.info.system_prompt_tokens);
        lines.push(Line::from(vec![
            Span::styled("  ○ ", Style::default().fg(colors.text.accent)),
            Span::styled("System Prompt", Style::default().fg(colors.text.primary)),
            Span::styled(
                format!(
                    ": {} tokens ({:.1}%)",
                    ContextInfo::fmt_tokens(self.info.system_prompt_tokens),
                    sys_pct
                ),
                Style::default().fg(colors.text.secondary),
            ),
        ]));

        // Tools
        let tools_pct = self.info.pct_of(self.info.tools_tokens);
        lines.push(Line::from(vec![
            Span::styled("  ○ ", Style::default().fg(colors.text.accent)),
            Span::styled("Tools", Style::default().fg(colors.text.primary)),
            Span::styled(
                format!(
                    ": {} tokens ({:.1}%)",
                    ContextInfo::fmt_tokens(self.info.tools_tokens),
                    tools_pct
                ),
                Style::default().fg(colors.text.secondary),
            ),
        ]));

        // Conversation
        let conv_pct = self.info.pct_of(self.info.conversation_tokens);
        lines.push(Line::from(vec![
            Span::styled("  ○ ", Style::default().fg(colors.text.accent)),
            Span::styled("Conversation", Style::default().fg(colors.text.primary)),
            Span::styled(
                format!(
                    ": {} tokens ({:.1}%)",
                    ContextInfo::fmt_tokens(self.info.conversation_tokens),
                    conv_pct
                ),
                Style::default().fg(colors.text.secondary),
            ),
        ]));

        // Free space
        let free = self.info.free_tokens();
        let free_pct = self.info.pct_of(free);
        lines.push(Line::from(vec![
            Span::styled("  ○ ", Style::default().fg(colors.status.success)),
            Span::styled("Free Space", Style::default().fg(colors.text.primary)),
            Span::styled(
                format!(
                    ": {} tokens ({:.1}%)",
                    ContextInfo::fmt_tokens(free),
                    free_pct
                ),
                Style::default().fg(colors.text.secondary),
            ),
        ]));

        lines.push(Self::section_line(&colors));

        // Tool details
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("TOOLS", Style::default().fg(colors.ui.comment)),
        ]));
        lines.push(Line::from(""));

        for (name, tokens) in &self.info.tool_details {
            lines.push(Line::from(vec![
                Span::styled("    └ ", Style::default().fg(colors.ui.dark)),
                Span::styled(
                    truncate_str(name, content_width.saturating_sub(20)),
                    Style::default().fg(colors.text.primary),
                ),
                Span::styled(
                    format!(": {}", ContextInfo::fmt_tokens(*tokens)),
                    Style::default().fg(colors.text.secondary),
                ),
            ]));
        }

        lines.push(Self::section_line(&colors));

        // Message details
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("MESSAGES", Style::default().fg(colors.ui.comment)),
        ]));
        lines.push(Line::from(""));

        for (role, tokens) in &self.info.message_details {
            let icon = match role.as_str() {
                "system" => "SYS",
                "user" => "USR",
                "assistant" => "AST",
                _ => "???",
            };
            lines.push(Line::from(vec![
                Span::styled("    └ ", Style::default().fg(colors.ui.dark)),
                Span::styled(icon, Style::default().fg(colors.text.link)),
                Span::styled(
                    truncate_str(
                        &format!(" {}: {}", role, ContextInfo::fmt_tokens(*tokens)),
                        content_width.saturating_sub(20),
                    ),
                    Style::default().fg(colors.text.secondary),
                ),
            ]));
        }

        // Trim to visible lines
        lines = lines.into_iter().take(visible_lines).collect();

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .title(" CONTEXT ")
                .title_style(Style::default().fg(colors.ui.comment))
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(colors.border.default))
                .style(Style::default().bg(colors.background.primary)),
        );

        frame.render_widget(paragraph, area);
    }
}

/// Truncate a string to fit within `max` character cells.
fn truncate_str(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}
