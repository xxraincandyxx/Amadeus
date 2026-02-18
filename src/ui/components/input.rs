use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{Block, Borders},
    Frame,
};
use tui_textarea::{CursorMove, TextArea};
use unicode_width::UnicodeWidthStr;

pub struct InputComponent {
    textarea: TextArea<'static>,
    history: Vec<String>,
    history_index: Option<usize>,
    current_draft: String,
}

impl InputComponent {
    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        textarea.set_block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(crate::ui::colors::THEME.border)),
        );
        textarea.set_style(Style::default().fg(crate::ui::colors::THEME.fg));
        textarea.set_cursor_style(Style::default().fg(crate::ui::colors::THEME.cyan));
        textarea.set_placeholder_style(Style::default().fg(crate::ui::colors::THEME.comment));
        textarea
            .set_placeholder_text(" Type your message... (Enter to send, Ctrl+Enter for newline)");

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
        self.textarea.set_block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(crate::ui::colors::THEME.border)),
        );
        self.textarea
            .set_style(Style::default().fg(crate::ui::colors::THEME.fg));
        self.textarea
            .set_cursor_style(Style::default().fg(crate::ui::colors::THEME.cyan));
        self.textarea
            .set_placeholder_style(Style::default().fg(crate::ui::colors::THEME.comment));
        self.textarea
            .set_placeholder_text(" Type your message... (Enter to send, Ctrl+Enter for newline)");
        self.history_index = None;
        self.current_draft.clear();
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
        self.textarea.set_block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(crate::ui::colors::THEME.border)),
        );
        self.textarea
            .set_style(Style::default().fg(crate::ui::colors::THEME.fg));
        self.textarea
            .set_cursor_style(Style::default().fg(crate::ui::colors::THEME.cyan));
        self.textarea
            .set_placeholder_style(Style::default().fg(crate::ui::colors::THEME.comment));
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

        (height_by_lines.max(height_by_width) as u16).min(10).max(3)
    }
}

impl Default for InputComponent {
    fn default() -> Self {
        Self::new()
    }
}
