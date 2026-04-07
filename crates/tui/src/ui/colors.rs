// @amadeus-header
// summary: TUI module code for colors.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::colors
// - type: crate::ui::colors::Theme
// - const: crate::ui::colors::THEME
// - type: crate::ui::colors::Palette
// - fn: crate::ui::colors::print_command
// - fn: crate::ui::colors::print_tool_result
// uses:
// - runtime: ratatui terminal rendering
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Writes output to stdout or stderr.
// tests:
// - tests/tui_snapshot_test.rs
// @end-amadeus-header

use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub purple: Color,
    pub cyan: Color,
    pub green: Color,
    pub pink: Color,
    pub orange: Color,
    pub red: Color,
    pub comment: Color,
    pub current_line: Color,
    pub selection: Color,
    pub user_msg: Color,
    pub assistant_msg: Color,
    pub tool_bg: Color,
    pub border: Color,
}

impl Theme {
    pub fn dracula() -> Self {
        Self {
            bg: Color::Rgb(12, 12, 12),
            fg: Color::Rgb(220, 220, 220),
            purple: Color::Rgb(120, 50, 50),
            cyan: Color::Rgb(140, 140, 160),
            green: Color::Rgb(100, 160, 100),
            pink: Color::Rgb(160, 100, 100),
            orange: Color::Rgb(180, 140, 80),
            red: Color::Rgb(180, 80, 80),
            comment: Color::Rgb(90, 90, 90),
            current_line: Color::Rgb(25, 25, 25),
            selection: Color::Rgb(40, 35, 35),
            user_msg: Color::Rgb(150, 130, 130),
            assistant_msg: Color::Rgb(120, 50, 50),
            tool_bg: Color::Rgb(12, 12, 12),
            border: Color::Rgb(50, 50, 50),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dracula()
    }
}

pub static THEME: Theme = Theme {
    bg: Color::Rgb(12, 12, 12),
    fg: Color::Rgb(220, 220, 220),
    purple: Color::Rgb(120, 50, 50),
    cyan: Color::Rgb(140, 140, 160),
    green: Color::Rgb(100, 160, 100),
    pink: Color::Rgb(160, 100, 100),
    orange: Color::Rgb(180, 140, 80),
    red: Color::Rgb(180, 80, 80),
    comment: Color::Rgb(90, 90, 90),
    current_line: Color::Rgb(25, 25, 25),
    selection: Color::Rgb(40, 35, 35),
    user_msg: Color::Rgb(150, 130, 130),
    assistant_msg: Color::Rgb(120, 50, 50),
    tool_bg: Color::Rgb(12, 12, 12),
    border: Color::Rgb(50, 50, 50),
};

pub struct Palette;

impl Palette {
    pub fn header() -> String {
        "🎣".to_string()
    }

    pub fn prompt() -> String {
        ">> ".to_string()
    }

    pub fn command(cmd: &str) -> String {
        format!("$ {}", cmd)
    }

    pub fn tool_result() -> String {
        "✓".to_string()
    }

    pub fn error(msg: &str) -> String {
        format!("✗ {}", msg)
    }

    pub fn info(msg: &str) -> String {
        format!("ℹ {}", msg)
    }
}

pub fn print_command(cmd: &str) {
    println!("{}", Palette::command(cmd));
}

pub fn print_tool_result(output: &str) {
    if output.is_empty() {
        println!("(empty)");
        return;
    }

    let truncated = if output.len() > 50000 {
        &output[..50000]
    } else {
        output
    };

    println!("{}", truncated);
}
