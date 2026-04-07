// @amadeus-header
// summary: TUI module code for vim.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::vim
// - type: crate::ui::vim::VimMode
// - type: crate::ui::vim::VimAction
// - type: crate::ui::vim::VimHandler
// uses:
// - runtime: crossterm terminal events
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects: none
// tests:
// - tests/tui_snapshot_test.rs
// @end-amadeus-header

//! # Vim Mode Handler
//!
//! Provides vim-style keybindings for the TUI.
//!
//! ## Modes
//!
//! - `Normal` - Navigation mode (j/k to scroll, i to enter input mode)
//! - `Input` - Text input mode (Escape to return to normal mode)
//! - `Command` - Command mode (press : to enter commands)
//!
//! ## Keybindings (Normal Mode)
//!
//! | Key | Action |
//! |-----|--------|
//! | j | Scroll down |
//! | k | Scroll up |
//! | g | Go to top |
//! | G | Go to bottom |
//! | i | Enter input mode |
//! | : | Enter command mode |
//! | q | Quit |
//! | Ctrl+C | Force quit |
//! | Escape | Return to normal mode |

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Vim modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VimMode {
    /// Normal mode for navigation.
    #[default]
    Normal,
    /// Input mode for text entry.
    Input,
    /// Command mode for commands.
    Command,
}

/// Result of handling a key in vim mode.
#[derive(Debug, Clone)]
pub enum VimAction {
    /// No action needed.
    None,
    /// Scroll down by n lines.
    ScrollDown(usize),
    /// Scroll up by n lines.
    ScrollUp(usize),
    /// Go to top of content.
    ScrollToTop,
    /// Go to bottom of content.
    ScrollToBottom,
    /// Scroll down by page.
    ScrollPageDown,
    /// Scroll up by page.
    ScrollPageUp,
    /// Switch to input mode.
    EnterInputMode,
    /// Switch to normal mode.
    EnterNormalMode,
    /// Switch to command mode.
    EnterCommandMode,
    /// Quit the application.
    Quit,
    /// Submit the current input.
    SubmitInput,
    /// Toggle file sidebar.
    ToggleFileSidebar,
    /// Toggle help sidebar.
    ToggleHelpSidebar,
    /// Next theme.
    NextTheme,
    /// Start new session.
    NewSession,
}

/// Vim handler for key processing.
#[derive(Debug, Clone)]
pub struct VimHandler {
    /// Current mode.
    pub mode: VimMode,
}

impl Default for VimHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl VimHandler {
    /// Create a new vim handler.
    pub fn new() -> Self {
        Self {
            mode: VimMode::Normal,
        }
    }

    /// Handle a key event and return the appropriate action.
    pub fn handle_key(&mut self, key: KeyEvent) -> VimAction {
        match self.mode {
            VimMode::Normal => self.handle_normal_key(key),
            VimMode::Input => self.handle_input_key(key),
            VimMode::Command => self.handle_command_key(key),
        }
    }

    /// Handle a key in normal mode.
    fn handle_normal_key(&mut self, key: KeyEvent) -> VimAction {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Char('j')) => VimAction::ScrollDown(1),
            (KeyModifiers::NONE, KeyCode::Char('k')) => VimAction::ScrollUp(1),
            (KeyModifiers::NONE, KeyCode::Char('g')) => VimAction::ScrollToTop,
            (KeyModifiers::SHIFT, KeyCode::Char('G')) => VimAction::ScrollToBottom,
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => VimAction::ScrollPageDown,
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => VimAction::ScrollPageUp,
            (KeyModifiers::NONE, KeyCode::Char('i')) => {
                self.mode = VimMode::Input;
                VimAction::EnterInputMode
            }
            (KeyModifiers::NONE, KeyCode::Char(':')) => {
                self.mode = VimMode::Command;
                VimAction::EnterCommandMode
            }
            (KeyModifiers::NONE, KeyCode::Char('q')) => VimAction::Quit,
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => VimAction::Quit,
            (KeyModifiers::NONE, KeyCode::Esc) => VimAction::EnterNormalMode,
            (KeyModifiers::NONE, KeyCode::Up) => VimAction::ScrollUp(1),
            (KeyModifiers::NONE, KeyCode::Down) => VimAction::ScrollDown(1),
            (KeyModifiers::CONTROL, KeyCode::Char('b')) => VimAction::ToggleFileSidebar,
            (KeyModifiers::SUPER, KeyCode::Char('b')) => VimAction::ToggleFileSidebar,
            (KeyModifiers::CONTROL, KeyCode::Char('t')) => VimAction::NextTheme,
            (KeyModifiers::CONTROL, KeyCode::Char('n')) => VimAction::NewSession,
            _ => VimAction::None,
        }
    }

    /// Handle a key in input mode.
    fn handle_input_key(&mut self, key: KeyEvent) -> VimAction {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.mode = VimMode::Normal;
                VimAction::EnterNormalMode
            }
            (KeyModifiers::NONE, KeyCode::Enter) => VimAction::SubmitInput,
            _ => VimAction::None,
        }
    }

    /// Handle a key in command mode.
    fn handle_command_key(&mut self, key: KeyEvent) -> VimAction {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.mode = VimMode::Normal;
                VimAction::EnterNormalMode
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.mode = VimMode::Normal;
                VimAction::SubmitInput
            }
            _ => VimAction::None,
        }
    }

    /// Set the mode.
    pub fn set_mode(&mut self, mode: VimMode) {
        self.mode = mode;
    }

    /// Check if in input mode.
    pub fn is_input_mode(&self) -> bool {
        matches!(self.mode, VimMode::Input)
    }

    /// Check if in normal mode.
    pub fn is_normal_mode(&self) -> bool {
        matches!(self.mode, VimMode::Normal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vim_scroll() {
        let mut handler = VimHandler::new();

        let action = handler.handle_key(KeyEvent::from(KeyCode::Char('j')));
        assert!(matches!(action, VimAction::ScrollDown(1)));

        let action = handler.handle_key(KeyEvent::from(KeyCode::Char('k')));
        assert!(matches!(action, VimAction::ScrollUp(1)));

        let action = handler.handle_key(KeyEvent::from(KeyCode::Char('g')));
        assert!(matches!(action, VimAction::ScrollToTop));

        let action = handler.handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT));
        assert!(matches!(action, VimAction::ScrollToBottom));
    }

    #[test]
    fn test_vim_mode_switch() {
        let mut handler = VimHandler::new();

        // Enter input mode
        let action = handler.handle_key(KeyEvent::from(KeyCode::Char('i')));
        assert!(matches!(action, VimAction::EnterInputMode));
        assert!(handler.is_input_mode());

        // Escape back to normal mode
        let action = handler.handle_key(KeyEvent::from(KeyCode::Esc));
        assert!(matches!(action, VimAction::EnterNormalMode));
        assert!(handler.is_normal_mode());
    }
}
