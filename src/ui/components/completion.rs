//! # Command Completion Component
//!
//! Provides auto-completion for slash commands in the TUI.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem},
};

use crate::ui::get_colors;

/// A command available for auto-completion.
#[derive(Debug, Clone)]
pub struct Command {
    /// The command name (e.g., "/new-agent")
    pub name: String,
    /// Brief description of what the command does
    pub description: String,
    /// Detailed usage info
    pub usage: Option<String>,
}

impl Command {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            usage: None,
        }
    }

    pub fn with_usage(mut self, usage: &str) -> Self {
        self.usage = Some(usage.to_string());
        self
    }
}

/// Returns all available commands.
pub fn get_available_commands() -> Vec<Command> {
    vec![
        Command::new("/new-agent", "Create a new agent with a specific profile")
            .with_usage("/new-agent [profile] - Profiles: default, debug, docs, review"),
        Command::new("/agents", "List all active agents"),
        Command::new("/kill", "Kill an agent by name or ID").with_usage("/kill <agent-name>"),
        Command::new("/compact", "Trigger context compaction"),
        Command::new("/compress", "Trigger context compaction"),
        Command::new("/context", "Toggle context sidebar"),
        Command::new("/help", "Show available commands"),
    ]
}

/// State for command auto-completion.
pub struct CompletionState {
    /// All available commands
    commands: Vec<Command>,
    /// Filtered commands matching current input
    matches: Vec<Command>,
    /// Index of the selected match
    selected_index: usize,
    /// Whether completion popup is visible
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

    /// Update completion based on current input.
    /// Returns true if completion should be shown.
    pub fn update(&mut self, input: &str) -> bool {
        let input = input.trim();

        // Only show completion for commands starting with /
        if !input.starts_with('/') {
            self.visible = false;
            return false;
        }

        // Filter commands
        self.matches = self
            .commands
            .iter()
            .filter(|cmd| cmd.name.to_lowercase().starts_with(&input.to_lowercase()))
            .cloned()
            .collect();

        // Reset selection
        self.selected_index = 0;

        // Show popup if there are matches and user typed a command prefix
        self.visible = !self.matches.is_empty() && !input.is_empty();
        self.visible
    }

    /// Move selection up.
    pub fn select_up(&mut self) {
        if !self.matches.is_empty() {
            self.selected_index = self.selected_index.saturating_sub(1);
        }
    }

    /// Move selection down.
    pub fn select_down(&mut self) {
        if !self.matches.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.matches.len();
        }
    }

    /// Get the currently selected command.
    pub fn selected(&self) -> Option<&Command> {
        self.matches.get(self.selected_index)
    }

    /// Check if completion is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get the number of matches.
    pub fn _match_count(&self) -> usize {
        self.matches.len()
    }
}

impl Default for CompletionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Render the completion popup (minimal, theme-driven—Claude-style flat list).
///
/// The popup floats **above** `input_rect` so it doesn't collide with the status
/// bar / footer that sit below the composer.
pub fn render_completion(
    frame: &mut ratatui::Frame,
    area: Rect,
    state: &CompletionState,
    input_rect: Rect,
    _popup_below: Option<Rect>,
) {
    if !state.is_visible() || state.matches.is_empty() {
        return;
    }

    let colors = get_colors();
    let max_items = 6.min(state.matches.len());
    let list_rows = max_items as u16;
    let popup_height = list_rows.saturating_add(1).max(2);
    let target_w = input_rect.width.clamp(24, 72);
    let popup_width = target_w.min(area.width.saturating_sub(input_rect.x));
    let popup_y = input_rect.y.saturating_sub(popup_height);
    let popup_area = Rect::new(input_rect.x, popup_y, popup_width, popup_height);

    let items: Vec<ListItem<'static>> = state
        .matches
        .iter()
        .take(max_items)
        .enumerate()
        .map(|(i, cmd)| {
            let is_selected = i == state.selected_index;
            let style = if is_selected {
                Style::default()
                    .fg(colors.text.primary)
                    .bg(colors.background.message)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(colors.text.secondary)
            };

            let content = if let Some(usage) = &cmd.usage {
                format!("{}  {}", cmd.name, usage)
            } else {
                format!("{}  {}", cmd.name, cmd.description)
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                .border_style(Style::default().fg(colors.ui.dark))
                .style(Style::default().bg(colors.background.input)),
        )
        .highlight_style(
            Style::default()
                .fg(colors.text.primary)
                .bg(colors.background.message)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(list, popup_area);
}
