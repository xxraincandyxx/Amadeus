// @amadeus-header
// summary: TUI component implementation for completion.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::components::completion
// - type: crate::ui::components::completion::Command
// - fn: crate::ui::components::completion::get_available_commands
// - type: crate::ui::components::completion::CompletionState
// - fn: crate::ui::components::completion::render_completion_lines
// uses:
// - module: crate::ui::get_colors
// - runtime: ratatui terminal rendering
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Runs external commands or subprocesses.
// tests:
// - tests/tui_snapshot_test.rs
// @end-amadeus-header

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::ui::get_colors;

const CMD_COL_WIDTH: usize = 32;
const MAX_VISIBLE_ITEMS: usize = 6;
const ELLIPSIS: &str = "…";

#[derive(Debug, Clone)]
pub struct Command {
    pub name: String,
    pub description: String,
}

impl Command {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
        }
    }
}

pub fn get_available_commands() -> Vec<Command> {
    vec![
        Command::new("/compact", "Trigger context compaction"),
        Command::new("/compress", "Trigger context compaction"),
        Command::new("/context", "Toggle context sidebar"),
        Command::new("/help", "Show available commands"),
        Command::new("/kill", "Kill an agent by name or ID"),
        Command::new("/new-agent", "Create a new agent with a specific profile"),
        Command::new("/agents", "List all active agents"),
    ]
}

pub struct CompletionState {
    commands: Vec<Command>,
    matches: Vec<Command>,
    selected_index: usize,
    visible: bool,
}

impl CompletionState {
    pub fn new() -> Self {
        let commands = get_available_commands();
        Self {
            matches: commands.clone(),
            commands,
            selected_index: 0,
            visible: false,
        }
    }

    pub fn update(&mut self, input: &str) -> bool {
        let input = input.trim();

        if !input.starts_with('/') {
            self.visible = false;
            return false;
        }

        let query = input.to_lowercase();

        self.matches = self
            .commands
            .iter()
            .filter(|cmd| {
                let name = cmd.name.to_lowercase();
                name.contains(&query) || cmd.description.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();

        self.selected_index = 0;
        self.visible = !self.matches.is_empty() && !input.is_empty();
        self.visible
    }

    pub fn select_up(&mut self) {
        if !self.matches.is_empty() {
            self.selected_index = self.selected_index.saturating_sub(1);
        }
    }

    pub fn select_down(&mut self) {
        if !self.matches.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.matches.len();
        }
    }

    pub fn selected(&self) -> Option<&Command> {
        self.matches.get(self.selected_index)
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn visible_count(&self) -> usize {
        MAX_VISIBLE_ITEMS.min(self.matches.len())
    }
}

impl Default for CompletionState {
    fn default() -> Self {
        Self::new()
    }
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
        format!("{}{ELLIPSIS}", &s[..end])
    }
}

pub fn render_completion_lines(frame: &mut Frame, area: Rect, state: &CompletionState) {
    if !state.is_visible() || state.matches.is_empty() || area.height == 0 {
        return;
    }

    let colors = get_colors();
    let width = area.width as usize;
    let desc_width = width.saturating_sub(CMD_COL_WIDTH).saturating_sub(4);

    let lines: Vec<Line<'static>> = state
        .matches
        .iter()
        .take(MAX_VISIBLE_ITEMS)
        .enumerate()
        .map(|(i, cmd)| {
            let name = format!("{:width$}", cmd.name, width = CMD_COL_WIDTH);
            let desc = if desc_width > 0 {
                truncate_with_ellipsis(&cmd.description, desc_width)
            } else {
                String::new()
            };

            let is_selected = i == state.selected_index;
            let style = if is_selected {
                Style::default()
                    .fg(colors.text.primary)
                    .bg(colors.background.message)
            } else {
                Style::default().fg(colors.text.secondary)
            };

            Line::from(Span::styled(format!("  {name}{desc}"), style))
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), area);
}
