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

pub struct InputComponent {
    textarea: TextArea<'static>,
    history: Vec<String>,
    history_index: Option<usize>,
    current_draft: String,
    status_hint: Option<String>,
    completion: CompletionState,
    shortcuts_visible: bool,
}

impl InputComponent {
    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        let colors = get_colors();

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

        Self {
            textarea,
            history: Vec::new(),
            history_index: None,
            current_draft: String::new(),
            status_hint: None,
            completion: CompletionState::new(),
            shortcuts_visible: false,
        }
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
        self.textarea.set_placeholder_text(composer_placeholder());
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
        let lines: Vec<String> = text.lines().map(String::from).collect();
        self.textarea = TextArea::new(lines);
        self.setup_textarea();
        self.textarea.move_cursor(CursorMove::End);
    }

    pub fn insert_newline(&mut self) {
        self.textarea.insert_newline();
    }

    pub fn handle_char(&mut self, c: char) {
        self.textarea.insert_char(c);
    }

    pub fn handle_backspace(&mut self) {
        self.textarea.delete_char();
    }

    pub fn handle_delete(&mut self) {
        self.textarea.delete_next_char();
    }

    pub fn move_cursor_left(&mut self) {
        self.textarea.move_cursor(CursorMove::Back);
    }

    pub fn move_cursor_right(&mut self) {
        self.textarea.move_cursor(CursorMove::Forward);
    }

    pub fn move_cursor_line_start(&mut self) {
        self.textarea.move_cursor(CursorMove::Head);
    }

    pub fn move_cursor_line_end(&mut self) {
        self.textarea.move_cursor(CursorMove::End);
    }

    pub fn move_cursor_word_forward(&mut self) {
        self.textarea.move_cursor(CursorMove::WordForward);
    }

    pub fn move_cursor_word_back(&mut self) {
        self.textarea.move_cursor(CursorMove::WordBack);
    }

    pub fn move_cursor_up(&mut self) {
        self.textarea.move_cursor(CursorMove::Up);
    }

    pub fn move_cursor_down(&mut self) {
        self.textarea.move_cursor(CursorMove::Down);
    }

    pub fn delete_line_by_end(&mut self) {
        self.textarea.delete_line_by_end();
    }

    pub fn delete_line_by_head(&mut self) {
        self.textarea.delete_line_by_head();
    }

    pub fn delete_word(&mut self) {
        self.textarea.delete_word();
    }

    pub fn delete_next_word(&mut self) {
        self.textarea.delete_next_word();
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
                ("Ctrl+]", "Next session"),
            ],
            &[
                ("Tab", "Accept completion"),
                ("Ctrl+K", "Compact history"),
                ("Esc", "Prev session"),
            ],
            &[
                ("Shift+Tab", "Prev completion"),
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

        let input_text = self.textarea.lines().join("\n");
        let comp_visible = self.completion.update(&input_text);
        let comp_rows = if comp_visible {
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
            render_completion_lines(frame, chunks[ci], &self.completion);
        }

        let inner = chunks[inner_idx];
        self.textarea.set_block(Self::textarea_block());
        self.textarea.set_placeholder_text(composer_placeholder());

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

        if !comp_visible {
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
        self.completion.selected().map(|c| c.name.clone())
    }

    /// Check if completion popup is visible.
    pub fn completion_is_visible(&self) -> bool {
        self.completion.is_visible()
    }

    /// Move selection up in completion list.
    pub fn completion_select_up(&mut self) {
        self.completion.select_up();
    }

    /// Move selection down in completion list.
    pub fn completion_select_down(&mut self) {
        self.completion.select_down();
    }

    pub fn force_show_completion(&mut self) {
        let input = self.get_input();
        self.completion.update(&input);
    }

    /// Apply the selected completion (replace input with command).
    pub fn apply_completion(&mut self) {
        if let Some(cmd) = self.completion.selected() {
            let lines: Vec<String> = vec![cmd.name.clone()];
            self.textarea = TextArea::new(lines);
            self.setup_textarea();
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
        if self.completion.is_visible() {
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
}

impl Default for InputComponent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
