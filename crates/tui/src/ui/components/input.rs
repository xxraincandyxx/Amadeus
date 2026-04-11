// @amadeus-header
// summary: TUI component implementation for input.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::components::input
// - type: crate::ui::components::input::InputComponent
// uses:
// - type: crate::commands::ActiveCitationQuery
// - type: crate::commands::CitationCandidate
// - module: crate::ui::components::completion
// - module: crate::ui::get_colors
// - runtime: ratatui terminal rendering
// invariants:
// - The visible composer can render cite chips while the underlying buffer stores markdown links.
// side_effects: none
// tests:
// - tests/tui_snapshot_test.rs
// @end-amadeus-header

use std::path::{Path, PathBuf};

use amadeus_core::{
    apply_citation_candidate, filter_citation_candidates, find_active_citation_query,
    format_citation_markdown, normalize_pasted_path, parse_render_spans,
    scan_workspace_citation_candidates, ActiveCitationQuery, CitationCandidate,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tui_textarea::{CursorMove, TextArea};
use unicode_width::UnicodeWidthStr;

use crate::ui::components::completion::{render_completion_lines, CompletionState};
use crate::ui::get_colors;
use crate::ui::semantic_colors::SemanticColors;

/// Example text inside the `Try "…"` hint (configurable, English UI).
const TRY_PROMPT_ENV: &str = "AMADEUS_TRY_PROMPT";
const DEFAULT_TRY_PROMPT: &str = "how does src/main.rs work?";

fn try_prompt_from_env() -> String {
    match std::env::var(TRY_PROMPT_ENV) {
        Ok(s) => {
            let t = s.trim().to_string();
            if t.is_empty() {
                DEFAULT_TRY_PROMPT.to_string()
            } else {
                t
            }
        }
        Err(_) => DEFAULT_TRY_PROMPT.to_string(),
    }
}

/// Claude Code–style placeholder: same line as `❯`, sample replaces when user types.
fn composer_placeholder() -> String {
    format!("Try \"{}\"", try_prompt_from_env())
}

fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .nth(max_len.saturating_sub(1))
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}…", &s[..end])
    }
}

pub struct InputComponent {
    workdir: PathBuf,
    textarea: TextArea<'static>,
    history: Vec<String>,
    history_index: Option<usize>,
    current_draft: String,
    status_hint: Option<String>,
    completion: CompletionState,
    citation_candidates: Vec<CitationCandidate>,
    citation_matches: Vec<CitationCandidate>,
    active_citation_query: Option<ActiveCitationQuery>,
    citation_selected_index: usize,
    btw_dropup: Option<BtwDropupState>,
    shortcuts_visible: bool,
    placeholder_visible: bool,
}

struct BtwDropupState {
    lines: Vec<String>,
    is_error: bool,
}

impl InputComponent {
    pub fn new() -> Self {
        let workdir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::new_with_workdir(workdir)
    }

    pub fn new_with_workdir(workdir: PathBuf) -> Self {
        let mut textarea = TextArea::default();
        let colors = get_colors();
        let citation_candidates = scan_workspace_citation_candidates(&workdir).unwrap_or_default();

        textarea.set_block(Self::textarea_block());
        textarea.set_style(Style::default().fg(colors.text.primary));
        textarea.set_cursor_style(
            Style::default()
                .fg(colors.text.link)
                .add_modifier(Modifier::REVERSED),
        );
        textarea.set_placeholder_style(
            Style::default()
                .fg(colors.ui.comment)
                .add_modifier(Modifier::ITALIC),
        );
        textarea.set_placeholder_text(composer_placeholder());

        let mut input = Self {
            workdir,
            textarea,
            history: Vec::new(),
            history_index: None,
            current_draft: String::new(),
            status_hint: None,
            completion: CompletionState::new(),
            citation_candidates,
            citation_matches: Vec::new(),
            active_citation_query: None,
            citation_selected_index: 0,
            btw_dropup: None,
            shortcuts_visible: false,
            placeholder_visible: true,
        };
        input.refresh_suggestions();
        input
    }

    pub fn get_input(&self) -> String {
        self.textarea.lines().join("\n")
    }

    pub fn clear(&mut self) {
        let input = self.get_input();
        if !input.trim().is_empty() {
            self.history.push(input);
        }
        self.textarea = TextArea::default();
        self.setup_textarea();
        self.history_index = None;
        self.current_draft.clear();
        self.clear_btw_dropup();
        self.refresh_suggestions();
    }

    fn textarea_block() -> Block<'static> {
        Block::default().borders(Borders::NONE)
    }

    fn setup_textarea(&mut self) {
        let colors = get_colors();

        self.textarea.set_block(Self::textarea_block());
        self.textarea
            .set_style(Style::default().fg(colors.text.primary));
        self.textarea.set_cursor_style(
            Style::default()
                .fg(colors.text.link)
                .add_modifier(Modifier::REVERSED),
        );
        self.textarea.set_placeholder_style(
            Style::default()
                .fg(colors.ui.comment)
                .add_modifier(Modifier::ITALIC),
        );
        let placeholder = if self.placeholder_visible {
            composer_placeholder()
        } else {
            String::new()
        };
        self.textarea.set_placeholder_text(placeholder);
    }

    pub fn set_placeholder_visible(&mut self, visible: bool) {
        self.placeholder_visible = visible;
        self.setup_textarea();
    }

    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }

        if self.history_index.is_none() {
            self.current_draft = self.get_input();
            self.history_index = Some(self.history.len() - 1);
        } else if let Some(idx) = self.history_index {
            if idx > 0 {
                self.history_index = Some(idx - 1);
            }
        }

        if let Some(idx) = self.history_index {
            self.set_text(&self.history[idx].clone());
        }
    }

    pub fn history_down(&mut self) {
        if let Some(idx) = self.history_index {
            if idx + 1 >= self.history.len() {
                self.history_index = None;
                self.set_text(&self.current_draft.clone());
            } else {
                self.history_index = Some(idx + 1);
                self.set_text(&self.history[idx + 1].clone());
            }
        }
    }

    fn set_text(&mut self, text: &str) {
        self.set_text_and_cursor(text, None);
    }

    fn set_text_and_cursor(&mut self, text: &str, cursor: Option<(usize, usize)>) {
        let lines: Vec<String> = text.lines().map(String::from).collect();
        self.textarea = TextArea::new(lines);
        self.setup_textarea();
        if let Some((row, col)) = cursor {
            self.textarea
                .move_cursor(CursorMove::Jump(row as u16, col as u16));
        } else {
            self.textarea.move_cursor(CursorMove::End);
        }
        self.refresh_suggestions();
    }

    pub fn insert_newline(&mut self) {
        self.clear_btw_dropup();
        self.textarea.insert_newline();
        self.refresh_suggestions();
    }

    pub fn handle_char(&mut self, c: char) {
        self.clear_btw_dropup();
        self.textarea.insert_char(c);
        self.refresh_suggestions();
    }

    pub fn handle_backspace(&mut self) {
        self.clear_btw_dropup();
        self.textarea.delete_char();
        self.refresh_suggestions();
    }

    pub fn handle_delete(&mut self) {
        self.clear_btw_dropup();
        self.textarea.delete_next_char();
        self.refresh_suggestions();
    }

    pub fn move_cursor_left(&mut self) {
        self.clear_btw_dropup();
        self.textarea.move_cursor(CursorMove::Back);
        self.refresh_suggestions();
    }

    pub fn move_cursor_right(&mut self) {
        self.clear_btw_dropup();
        self.textarea.move_cursor(CursorMove::Forward);
        self.refresh_suggestions();
    }

    pub fn move_cursor_line_start(&mut self) {
        self.clear_btw_dropup();
        self.textarea.move_cursor(CursorMove::Head);
        self.refresh_suggestions();
    }

    pub fn move_cursor_line_end(&mut self) {
        self.clear_btw_dropup();
        self.textarea.move_cursor(CursorMove::End);
        self.refresh_suggestions();
    }

    pub fn move_cursor_word_forward(&mut self) {
        self.clear_btw_dropup();
        self.textarea.move_cursor(CursorMove::WordForward);
        self.refresh_suggestions();
    }

    pub fn move_cursor_word_back(&mut self) {
        self.clear_btw_dropup();
        self.textarea.move_cursor(CursorMove::WordBack);
        self.refresh_suggestions();
    }

    pub fn move_cursor_up(&mut self) {
        self.textarea.move_cursor(CursorMove::Up);
        self.refresh_suggestions();
    }

    pub fn move_cursor_down(&mut self) {
        self.textarea.move_cursor(CursorMove::Down);
        self.refresh_suggestions();
    }

    pub fn delete_line_by_end(&mut self) {
        self.clear_btw_dropup();
        self.textarea.delete_line_by_end();
        self.refresh_suggestions();
    }

    pub fn delete_line_by_head(&mut self) {
        self.clear_btw_dropup();
        self.textarea.delete_line_by_head();
        self.refresh_suggestions();
    }

    pub fn delete_word(&mut self) {
        self.clear_btw_dropup();
        self.textarea.delete_word();
        self.refresh_suggestions();
    }

    pub fn delete_next_word(&mut self) {
        self.clear_btw_dropup();
        self.textarea.delete_next_word();
        self.refresh_suggestions();
    }

    pub fn handle_paste(&mut self, pasted: &str) {
        self.clear_btw_dropup();
        let normalized = if !pasted.contains('\n') && !pasted.contains('\r') {
            normalize_pasted_path(pasted)
        } else {
            None
        };

        if let Some(path) = normalized {
            if path.exists()
                && (path.is_file() || path.is_dir())
                && self.path_is_visible_to_citation(&path)
            {
                if let Some(markdown) = format_citation_markdown(&path) {
                    self.textarea.insert_str(markdown);
                    self.refresh_suggestions();
                    return;
                }
            }
        }

        self.textarea.insert_str(pasted);
        self.refresh_suggestions();
    }

    fn shortcuts_lines(width: u16) -> Vec<Line<'static>> {
        let colors = get_colors();
        let key_style = Style::default().fg(colors.text.accent);
        let desc_style = Style::default().fg(colors.text.secondary);

        let shortcuts: &[&[(&str, &str)]] = &[
            &[
                ("Enter", "Send message"),
                ("Ctrl+C", "Cancel / Exit"),
                ("Alt+B", "Files sidebar"),
            ],
            &[
                ("Ctrl+Enter", "New line"),
                ("Esc", "Normal mode"),
                ("Alt+S", "Skills sidebar"),
            ],
            &[
                ("Up/Down", "History"),
                ("Ctrl+O", "Expand tools"),
                ("Ctrl+]", "To sub-agent"),
            ],
            &[
                ("Tab", "Next session"),
                ("Ctrl+K", "Compact history"),
                ("Ctrl+[", "To parent"),
            ],
            &[
                ("Shift+Tab", "Prev session"),
                ("Ctrl+Bksp", "Close session"),
                ("Shift+Up/Dn", "Scroll"),
            ],
        ];

        let col_w = (width as usize) / 3;
        let mut lines = Vec::with_capacity(shortcuts.len());
        for row in shortcuts {
            let mut spans = Vec::new();
            for (ci, (key, desc)) in row.iter().enumerate() {
                if ci > 0 {
                    spans.push(Span::raw(" "));
                }
                let key_part = format!(" {:<14}", key);
                let desc_part = desc.to_string();
                let cell = format!("{}{}", key_part, desc_part);
                let padded = if cell.len() < col_w {
                    format!("{:<width$}", cell, width = col_w)
                } else {
                    cell
                };
                let split = key_part.len();
                spans.push(Span::styled(padded[..split].to_string(), key_style));
                spans.push(Span::styled(padded[split..].to_string(), desc_style));
            }
            lines.push(Line::from(spans));
        }

        lines
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let colors = get_colors();
        let rule_style = Style::default().fg(colors.ui.dark);
        let hint_height = u16::from(self.status_hint.is_some());
        self.refresh_suggestions();

        let comp_rows = if self.citation_completion_is_visible() {
            self.citation_visible_count() as u16
        } else if let Some(dropup) = self.btw_dropup.as_ref() {
            dropup.lines.len() as u16
        } else if self.completion.is_visible() {
            self.completion.visible_count() as u16
        } else {
            0
        };

        let non_inner = 3u16.saturating_add(hint_height).saturating_add(comp_rows);
        if area.height <= non_inner {
            return;
        }
        let inner_h = area.height.saturating_sub(non_inner).max(1);

        let mut constraints: Vec<Constraint> = vec![Constraint::Length(1)];
        if self.status_hint.is_some() {
            constraints.push(Constraint::Length(1));
        }
        if comp_rows > 0 {
            constraints.push(Constraint::Length(comp_rows));
        }
        constraints.push(Constraint::Length(inner_h));
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(1));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let mut idx = 0;
        let top_rule_idx = idx;
        idx += 1;
        let hint_idx = if self.status_hint.is_some() {
            let h = idx;
            idx += 1;
            Some(h)
        } else {
            None
        };
        let comp_idx = if comp_rows > 0 {
            let c = idx;
            idx += 1;
            Some(c)
        } else {
            None
        };
        let inner_idx = idx;
        idx += 1;
        let bottom_rule_idx = idx;
        idx += 1;
        let shortcuts_idx = idx;

        let w = area.width.max(1) as usize;
        let rule: String = "─".repeat(w);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(rule.clone(), rule_style))),
            chunks[top_rule_idx],
        );

        if self.shortcuts_visible {
            let shortcut_lines = Self::shortcuts_lines(area.width);
            frame.render_widget(Paragraph::new(shortcut_lines), chunks[inner_idx]);
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(rule, rule_style))),
                chunks[bottom_rule_idx],
            );
            return;
        }

        if let Some(hi) = hint_idx {
            if let Some(hint) = &self.status_hint {
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        format!("  {hint}"),
                        Style::default().fg(colors.text.secondary),
                    ))),
                    chunks[hi],
                );
            }
        }

        if let Some(ci) = comp_idx {
            if self.btw_dropup_is_visible() {
                self.render_btw_dropup(frame, chunks[ci]);
            } else if self.citation_completion_is_visible() {
                self.render_citation_lines(frame, chunks[ci]);
            } else {
                render_completion_lines(frame, chunks[ci], &self.completion);
            }
        }

        let inner = chunks[inner_idx];
        self.textarea.set_block(Self::textarea_block());
        let placeholder = if self.placeholder_visible {
            composer_placeholder()
        } else {
            String::new()
        };
        self.textarea.set_placeholder_text(placeholder);

        const PROMPT: &str = "❯ ";
        let gutter_w = (PROMPT.width() as u16).clamp(1, inner.width);
        let ta_w = inner.width.saturating_sub(gutter_w);
        if ta_w == 0 || inner.height == 0 {
            return;
        }
        let ta_rect = Rect {
            x: inner.x.saturating_add(gutter_w),
            y: inner.y,
            width: ta_w,
            height: inner.height,
        };
        let prompt_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: gutter_w,
            height: 1.min(inner.height),
        };

        frame.render_widget(&self.textarea, ta_rect);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                PROMPT,
                Style::default().fg(colors.text.accent),
            )))
            .alignment(Alignment::Left),
            prompt_rect,
        );

        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(rule, rule_style))),
            chunks[bottom_rule_idx],
        );

        if comp_rows == 0 {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "  ? for shortcuts",
                    Style::default().fg(colors.ui.comment),
                ))),
                chunks[shortcuts_idx],
            );
        }
    }

    /// Get the currently selected completion command, if any.
    pub fn get_completion(&self) -> Option<String> {
        if self.citation_completion_is_visible() {
            self.citation_matches
                .get(self.citation_selected_index)
                .map(|candidate| candidate.label.clone())
        } else {
            self.completion.selected().map(|c| c.name.clone())
        }
    }

    /// Check if completion popup is visible.
    pub fn completion_is_visible(&self) -> bool {
        self.btw_dropup_is_visible() || self.citation_completion_is_visible() || self.completion.is_visible()
    }

    /// Move selection up in completion list.
    pub fn completion_select_up(&mut self) {
        if self.btw_dropup_is_visible() {
            return;
        }
        if self.citation_completion_is_visible() {
            self.citation_selected_index = self.citation_selected_index.saturating_sub(1);
        } else {
            self.completion.select_up();
        }
    }

    /// Move selection down in completion list.
    pub fn completion_select_down(&mut self) {
        if self.btw_dropup_is_visible() {
            return;
        }
        if self.citation_completion_is_visible() {
            if !self.citation_matches.is_empty() {
                self.citation_selected_index =
                    (self.citation_selected_index + 1) % self.citation_matches.len();
            }
        } else {
            self.completion.select_down();
        }
    }

    pub fn force_show_completion(&mut self) {
        let input = self.get_input();
        self.clear_btw_dropup();
        self.completion.update(&input);
        self.refresh_suggestions();
    }

    /// Apply the selected completion (replace input with command).
    pub fn apply_completion(&mut self) {
        if self.btw_dropup_is_visible() {
            return;
        }
        if self.citation_completion_is_visible() {
            if let (Some(query), Some(candidate)) = (
                self.active_citation_query.clone(),
                self.citation_matches
                    .get(self.citation_selected_index)
                    .cloned(),
            ) {
                let applied = apply_citation_candidate(&self.get_input(), &query, &candidate);
                self.set_text_and_cursor(&applied.text, Some(applied.cursor));
            }
        } else if let Some(cmd) = self.completion.selected() {
            let lines: Vec<String> = vec![cmd.name.clone()];
            self.textarea = TextArea::new(lines);
            self.setup_textarea();
            self.refresh_suggestions();
        }
    }

    pub fn height(&self) -> u16 {
        if self.shortcuts_visible {
            const SHORTCUT_ROWS: u16 = 5;
            const SHORTCUT_CHROME: u16 = 4;
            return SHORTCUT_ROWS.saturating_add(SHORTCUT_CHROME);
        }

        let lines = self.textarea.lines();
        const INNER_COLS: usize = 76;
        fn line_visual_rows(line: &str, cols: usize) -> u16 {
            let w = line.width();
            if w == 0 {
                return 1;
            }
            ((w.saturating_add(cols).saturating_sub(1)) / cols).max(1) as u16
        }

        let mut editor_h: u16 = 0;
        for line in lines.iter() {
            editor_h = editor_h.saturating_add(line_visual_rows(line, INNER_COLS));
        }
        if editor_h == 0 {
            editor_h = 1;
        }

        let chrome = 3u16.saturating_add(u16::from(self.status_hint.is_some()));
        let total = chrome.saturating_add(editor_h);
        total.max(chrome.saturating_add(1)).min(15)
    }

    pub fn completion_height(&self) -> u16 {
        if let Some(dropup) = self.btw_dropup.as_ref() {
            dropup.lines.len() as u16
        } else if self.citation_completion_is_visible() {
            self.citation_visible_count() as u16
        } else if self.completion.is_visible() {
            self.completion.visible_count() as u16
        } else {
            0
        }
    }

    /// Get input statistics: (character count, line count)
    pub fn get_stats(&self) -> (usize, usize) {
        let lines = self.textarea.lines();
        let line_count = lines.len();
        let char_count: usize = lines.iter().map(|l| l.chars().count()).sum();
        (char_count, line_count)
    }

    pub fn set_status_hint(&mut self, hint: Option<String>) {
        self.status_hint = hint;
    }

    pub fn show_shortcuts(&mut self, visible: bool) {
        self.shortcuts_visible = visible;
    }

    pub fn is_shortcuts_visible(&self) -> bool {
        self.shortcuts_visible
    }

    pub fn set_btw_dropup(
        &mut self,
        _command: impl Into<String>,
        content: impl Into<String>,
        is_error: bool,
    ) {
        let lines = content
            .into()
            .lines()
            .take(4)
            .map(str::to_string)
            .collect::<Vec<_>>();
        self.btw_dropup = Some(BtwDropupState {
            lines: if lines.is_empty() { vec![String::new()] } else { lines },
            is_error,
        });
    }

    pub fn clear_btw_dropup(&mut self) {
        self.btw_dropup = None;
    }

    pub fn btw_dropup_is_visible(&self) -> bool {
        self.btw_dropup.is_some()
    }

    fn citation_completion_is_visible(&self) -> bool {
        self.active_citation_query.is_some() && !self.citation_matches.is_empty()
    }

    fn citation_visible_count(&self) -> usize {
        6.min(self.citation_matches.len())
    }

    fn refresh_suggestions(&mut self) {
        let input = self.get_input();
        self.completion.update(&input);

        let cursor = self.textarea.cursor();
        self.active_citation_query = find_active_citation_query(&input, cursor);
        if let Some(query) = &self.active_citation_query {
            self.citation_matches =
                filter_citation_candidates(&self.citation_candidates, &query.query, 6);
            if self.citation_selected_index >= self.citation_matches.len() {
                self.citation_selected_index = 0;
            }
        } else {
            self.citation_matches.clear();
            self.citation_selected_index = 0;
        }
    }

    fn render_citation_lines(&self, frame: &mut Frame, area: Rect) {
        if !self.citation_completion_is_visible() || area.height == 0 {
            return;
        }

        let colors = get_colors();
        let width = area.width as usize;
        let desc_width = width.saturating_sub(24).saturating_sub(4);
        let lines: Vec<Line<'static>> = self
            .citation_matches
            .iter()
            .take(self.citation_visible_count())
            .enumerate()
            .map(|(idx, candidate)| {
                let token = format!("@{:<22}", candidate.label);
                let desc = if desc_width > 0 {
                    truncate_with_ellipsis(&candidate.relative_path, desc_width)
                } else {
                    String::new()
                };
                let is_selected = idx == self.citation_selected_index;
                let style = if is_selected {
                    Style::default()
                        .fg(colors.text.primary)
                        .bg(colors.background.message)
                } else {
                    Style::default().fg(colors.text.secondary)
                };

                Line::from(Span::styled(format!("  {token}{desc}"), style))
            })
            .collect();

        frame.render_widget(Paragraph::new(lines), area);
    }

    fn render_btw_dropup(&self, frame: &mut Frame, area: Rect) {
        let Some(dropup) = self.btw_dropup.as_ref() else {
            return;
        };
        if area.height == 0 {
            return;
        }

        let colors = get_colors();
        let body_style = if dropup.is_error {
            Style::default().fg(colors.status.error)
        } else {
            Style::default().fg(colors.text.secondary)
        };

        let width = area.width.saturating_sub(2) as usize;
        let lines = dropup
            .lines
            .iter()
            .map(|line| {
                let text = truncate_with_ellipsis(line, width.max(1));
                Line::from(Span::styled(format!("  {text}"), body_style))
            })
            .collect::<Vec<_>>();

        frame.render_widget(Paragraph::new(lines), area);
    }

    fn visible_input_lines(&self) -> Vec<Line<'static>> {
        let colors = get_colors();
        let input = self.get_input();
        let spans = parse_render_spans(&input);
        let lines = self.textarea.lines();

        lines
            .iter()
            .enumerate()
            .map(|(line_index, line)| {
                let line_spans = spans
                    .iter()
                    .filter(|span| span.line_index == line_index)
                    .cloned()
                    .collect::<Vec<_>>();
                build_visible_line(line, &line_spans, colors)
            })
            .collect()
    }

    fn path_is_visible_to_citation(&self, path: &Path) -> bool {
        if path.is_absolute() {
            path.starts_with(&self.workdir) || path.exists()
        } else {
            self.workdir.join(path).exists()
        }
    }
}

impl Default for InputComponent {
    fn default() -> Self {
        Self::new()
    }
}

fn build_visible_line(
    line: &str,
    spans: &[amadeus_core::CitationRenderSpan],
    colors: SemanticColors,
) -> Line<'static> {
    let mut rendered = Vec::new();
    let mut cursor = 0usize;
    let chars = line.chars().collect::<Vec<_>>();

    for span in spans {
        if span.start_col > cursor {
            rendered.push(Span::styled(
                chars[cursor..span.start_col].iter().collect::<String>(),
                Style::default().fg(colors.text.primary),
            ));
        }

        let raw_width = chars[span.start_col..span.end_col]
            .iter()
            .collect::<String>()
            .width();
        let visible = format!("@{}", span.label);
        let visible_width = visible.width();
        let padding = raw_width.saturating_sub(visible_width);

        rendered.push(Span::styled(
            visible,
            Style::default()
                .fg(colors.text.link)
                .bg(colors.background.message),
        ));
        if padding > 0 {
            rendered.push(Span::styled(
                " ".repeat(padding),
                Style::default()
                    .fg(colors.background.primary)
                    .bg(colors.background.primary),
            ));
        }
        cursor = span.end_col;
    }

    if cursor < chars.len() {
        rendered.push(Span::styled(
            chars[cursor..].iter().collect::<String>(),
            Style::default().fg(colors.text.primary),
        ));
    }

    if rendered.is_empty() {
        Line::from(String::new())
    } else {
        Line::from(rendered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use tempfile::tempdir;

    fn temp_input() -> (tempfile::TempDir, InputComponent) {
        let temp = tempdir().expect("tempdir");
        let input = InputComponent::new_with_workdir(temp.path().to_path_buf());
        (temp, input)
    }

    #[test]
    fn test_input_new() {
        let input = InputComponent::new();
        assert!(input.get_input().is_empty());
        assert!(input.history.is_empty());
    }

    #[test]
    fn test_input_handle_char() {
        let mut input = InputComponent::new();
        input.handle_char('a');
        input.handle_char('b');
        input.handle_char('c');
        assert_eq!(input.get_input(), "abc");
    }

    #[test]
    fn test_input_backspace() {
        let mut input = InputComponent::new();
        input.handle_char('a');
        input.handle_char('b');
        input.handle_backspace();
        assert_eq!(input.get_input(), "a");
    }

    #[test]
    fn test_input_clear() {
        let mut input = InputComponent::new();
        input.handle_char('t');
        input.handle_char('e');
        input.handle_char('s');
        input.handle_char('t');
        input.clear();
        assert!(input.get_input().is_empty());
        assert_eq!(input.history.len(), 1);
    }

    #[test]
    fn test_input_multiline() {
        let mut input = InputComponent::new();
        input.handle_char('a');
        input.insert_newline();
        input.handle_char('b');
        assert_eq!(input.get_input(), "a\nb");
    }

    #[test]
    fn test_get_stats_empty() {
        let input = InputComponent::new();
        let (chars, lines) = input.get_stats();
        assert_eq!(chars, 0);
        assert_eq!(lines, 1); // Empty textarea has 1 line
    }

    #[test]
    fn test_get_stats_single_line() {
        let mut input = InputComponent::new();
        for c in "hello world".chars() {
            input.handle_char(c);
        }
        let (chars, lines) = input.get_stats();
        assert_eq!(chars, 11);
        assert_eq!(lines, 1);
    }

    #[test]
    fn test_get_stats_multiline() {
        let mut input = InputComponent::new();
        input.handle_char('a');
        input.insert_newline();
        input.handle_char('b');
        input.insert_newline();
        input.handle_char('c');
        let (chars, lines) = input.get_stats();
        assert_eq!(chars, 3);
        assert_eq!(lines, 3);
    }

    #[test]
    fn test_get_stats_unicode() {
        let mut input = InputComponent::new();
        for c in "你好世界".chars() {
            input.handle_char(c);
        }
        let (chars, lines) = input.get_stats();
        assert_eq!(chars, 4);
        assert_eq!(lines, 1);
    }

    #[test]
    fn test_height_minimum() {
        let input = InputComponent::new();
        // top + inner(1) + bottom + shortcuts = 4 when no status hint
        assert!(input.height() >= 4);
    }

    #[test]
    fn test_height_grows_with_lines() {
        let mut input = InputComponent::new();
        let initial_height = input.height();

        for _ in 0..10 {
            input.insert_newline();
        }

        assert!(input.height() > initial_height);
    }

    #[test]
    fn test_history_navigation() {
        let mut input = InputComponent::new();

        // Add some history
        input.handle_char('1');
        input.clear();
        input.handle_char('2');
        input.clear();
        input.handle_char('3');
        input.clear();

        assert_eq!(input.history.len(), 3);

        // Navigate up
        input.history_up();
        assert_eq!(input.get_input(), "3");

        input.history_up();
        assert_eq!(input.get_input(), "2");

        input.history_up();
        assert_eq!(input.get_input(), "1");

        // Navigate down
        input.history_down();
        assert_eq!(input.get_input(), "2");

        input.history_down();
        assert_eq!(input.get_input(), "3");

        input.history_down();
        // Should return to draft (empty)
        assert!(input.get_input().is_empty() || input.get_input() == "");
    }

    #[test]
    fn test_cursor_movement() {
        let mut input = InputComponent::new();
        input.handle_char('a');
        input.handle_char('b');
        input.handle_char('c');

        input.move_cursor_left();
        input.handle_char('x');

        // Should have inserted 'x' before 'c'
        assert!(input.get_input().contains('x'));
    }

    #[test]
    fn test_shortcuts_overlay_default_off() {
        let input = InputComponent::new();
        assert!(!input.shortcuts_visible);
    }

    #[test]
    fn test_toggle_shortcuts_overlay() {
        let mut input = InputComponent::new();
        input.show_shortcuts(true);
        assert!(input.shortcuts_visible);
        input.show_shortcuts(false);
        assert!(!input.shortcuts_visible);
    }

    #[test]
    fn test_render_shortcuts_overlay_lines() {
        let lines = InputComponent::shortcuts_lines(80);
        assert!(!lines.is_empty(), "should produce shortcut lines");
        assert!(
            lines.len() >= 3,
            "should have at least 3 rows for 3-column layout"
        );
    }

    #[test]
    fn test_clear_empty_doesnt_add_to_history() {
        let mut input = InputComponent::new();
        input.clear(); // Clear empty input
        assert!(input.history.is_empty());

        input.handle_char(' ');
        input.clear(); // Clear whitespace-only
        assert!(input.history.is_empty());
    }

    #[test]
    fn test_citation_completion_inserts_markdown_and_renders_token() {
        let (temp, mut input) = temp_input();
        fs::write(temp.path().join("reviewer.md"), "review").expect("write reviewer");
        input.citation_candidates =
            scan_workspace_citation_candidates(temp.path()).expect("scan citations");

        input.handle_char('@');
        input.handle_char('r');
        input.handle_char('e');
        input.handle_char('v');

        assert!(input.citation_completion_is_visible());
        assert_eq!(input.get_completion().as_deref(), Some("reviewer.md"));

        input.apply_completion();
        let expected_path = temp
            .path()
            .join("reviewer.md")
            .canonicalize()
            .expect("canonical reviewer");

        assert_eq!(
            input.get_input(),
            format!("[reviewer.md]({})", expected_path.display())
        );
        let rendered = input
            .visible_input_lines()
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("@reviewer.md"));
    }

    #[test]
    fn test_citation_completion_reports_popup_height() {
        let (temp, mut input) = temp_input();
        fs::write(temp.path().join("reviewer.md"), "review").expect("write reviewer");
        input.citation_candidates =
            scan_workspace_citation_candidates(temp.path()).expect("scan citations");

        input.handle_char('@');
        input.handle_char('r');

        assert!(input.citation_completion_is_visible());
        assert!(input.completion_height() > 0);
    }

    #[test]
    fn test_btw_dropup_reports_popup_height() {
        let (_temp, mut input) = temp_input();
        input.set_btw_dropup("/btw", "Usage: /btw <question>", false);

        assert!(input.btw_dropup_is_visible());
        assert_eq!(input.completion_height(), 1);
    }

    #[test]
    fn test_editing_clears_btw_dropup() {
        let (_temp, mut input) = temp_input();
        input.set_btw_dropup("/btw", "Usage: /btw <question>", false);
        input.handle_char('a');

        assert!(!input.btw_dropup_is_visible());
    }

    #[test]
    fn test_raw_at_query_remains_visible_before_citation_is_accepted() {
        let (_temp, mut input) = temp_input();

        input.handle_char('@');
        input.handle_char('r');
        input.handle_char('e');
        input.handle_char('v');

        let rendered = input
            .visible_input_lines()
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("@rev"));
    }

    #[test]
    fn test_path_paste_becomes_citation_markdown() {
        let (temp, mut input) = temp_input();
        let path = temp.path().join("reviewer.md");
        fs::write(&path, "review").expect("write reviewer");
        let expected_path = path.canonicalize().expect("canonical reviewer");

        input.handle_paste(&path.display().to_string());

        assert_eq!(
            input.get_input(),
            format!("[reviewer.md]({})", expected_path.display())
        );
    }

    #[test]
    fn test_directory_paste_becomes_citation_markdown() {
        let (temp, mut input) = temp_input();
        let path = temp.path().join("docs");
        fs::create_dir_all(&path).expect("create docs dir");
        let expected_path = path.canonicalize().expect("canonical docs dir");

        input.handle_paste(&path.display().to_string());

        assert_eq!(
            input.get_input(),
            format!("[docs]({})", expected_path.display())
        );
    }

    #[test]
    fn test_multiline_paste_is_inserted_as_text() {
        let (_temp, mut input) = temp_input();

        input.handle_paste("alpha\nbeta");

        assert_eq!(input.get_input(), "alpha\nbeta");
    }
}
