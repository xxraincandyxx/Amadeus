use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders},
    Frame,
};
use tui_textarea::{CursorMove, TextArea};
use unicode_width::UnicodeWidthStr;

use crate::ui::components::completion::{render_completion, CompletionState};
use crate::ui::get_colors;

pub struct InputComponent {
    textarea: TextArea<'static>,
    history: Vec<String>,
    history_index: Option<usize>,
    current_draft: String,
    status_hint: Option<String>,
    completion: CompletionState,
}

impl InputComponent {
    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        let colors = get_colors();

        textarea.set_block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(colors.border.default))
                .title(" ❯ PROMPT ")
                .title_style(
                    Style::default()
                        .fg(colors.text.accent)
                        .add_modifier(Modifier::BOLD),
                ),
        );
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
        textarea.set_placeholder_text(" Type a message... (Enter: send, Alt+Enter: newline)");

        Self {
            textarea,
            history: Vec::new(),
            history_index: None,
            current_draft: String::new(),
            status_hint: None,
            completion: CompletionState::new(),
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

    fn setup_textarea(&mut self) {
        let colors = get_colors();

        self.textarea.set_block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(colors.border.default))
                .title(" ❯ PROMPT ")
                .title_style(
                    Style::default()
                        .fg(colors.text.accent)
                        .add_modifier(Modifier::BOLD),
                ),
        );
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
        self.textarea
            .set_placeholder_text(" Type a message... (Enter: send, Alt+Enter: newline)");
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

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if area.height < 3 {
            return;
        }

        // Update title with stats
        let colors = get_colors();
        let (chars, lines) = self.get_stats();
        let multi_indicator = if lines > 1 { " [multi]" } else { "" };
        let mut title = format!(
            " ❯ PROMPT [{} chars, {} line{}]{} ",
            chars,
            lines,
            if lines == 1 { "" } else { "s" },
            multi_indicator
        );
        if let Some(hint) = &self.status_hint {
            title.push_str("• ");
            title.push_str(hint);
            title.push(' ');
        }

        self.textarea.set_block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(colors.border.default))
                .title(title)
                .title_style(
                    Style::default()
                        .fg(colors.text.accent)
                        .add_modifier(Modifier::BOLD),
                ),
        );

        frame.render_widget(&self.textarea, area);

        // Render completion popup
        let input_text = self.textarea.lines().join("\n");
        if self.completion.update(&input_text) {
            render_completion(frame, area, &self.completion, area);
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

    /// Apply the selected completion (replace input with command).
    pub fn apply_completion(&mut self) {
        if let Some(cmd) = self.completion.selected() {
            // Replace current input with the command
            let lines: Vec<String> = vec![cmd.name.clone()];
            self.textarea = TextArea::new(lines);
            // Re-setup the textarea styling
            let colors = get_colors();
            self.textarea.set_block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(colors.border.default))
                    .title(" ❯ PROMPT ")
                    .title_style(
                        Style::default()
                            .fg(colors.text.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
            );
            self.textarea
                .set_style(Style::default().fg(colors.text.primary));
        }
    }

    pub fn height(&self) -> u16 {
        let lines = self.textarea.lines();
        let line_count = lines.len();
        let max_line_width = lines.iter().map(|l| l.width()).max().unwrap_or(0);

        let height_by_lines = line_count + 2;
        let height_by_width = (max_line_width / 80) + 2;

        (height_by_lines.max(height_by_width) as u16).clamp(4, 12)
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
    fn test_clear_empty_doesnt_add_to_history() {
        let mut input = InputComponent::new();
        input.clear(); // Clear empty input
        assert!(input.history.is_empty());

        input.handle_char(' ');
        input.clear(); // Clear whitespace-only
        assert!(input.history.is_empty());
    }
}
