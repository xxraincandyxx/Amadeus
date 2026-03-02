use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders},
    Frame,
};
use tui_textarea::{CursorMove, TextArea};
use unicode_width::UnicodeWidthStr;

use crate::ui::get_colors;

pub struct InputComponent {
    textarea: TextArea<'static>,
    history: Vec<String>,
    history_index: Option<usize>,
    current_draft: String,
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

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if area.height < 3 {
            return;
        }
        frame.render_widget(&self.textarea, area);
    }

    pub fn height(&self) -> u16 {
        let lines = self.textarea.lines();
        let line_count = lines.len();
        let max_line_width = lines.iter().map(|l| l.width()).max().unwrap_or(0);

        let height_by_lines = line_count + 2;
        let height_by_width = (max_line_width / 80) + 2;

        (height_by_lines.max(height_by_width) as u16).clamp(4, 12)
    }
}

impl Default for InputComponent {
    fn default() -> Self {
        Self::new()
    }
}
