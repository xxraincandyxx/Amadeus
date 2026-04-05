// @amadeus-header
// summary: Built-in github dark TUI theme definition.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::themes::github_dark
// - type: crate::ui::themes::github_dark::GitHubDark
// uses:
// - runtime: ratatui terminal rendering
// invariants:
// - Theme definitions keep semantic roles visually consistent.
// side_effects: none
// tests:
// - tests/tui_snapshot_test.rs
// @end-amadeus-header

use super::{Theme, ThemeType};
use crate::ui::semantic_colors::{
    interpolate_color, BackgroundColors, BorderColors, DiffColors, ScrollbarColors, SemanticColors,
    StatusColors, TextColors, UiColors,
};
use ratatui::style::Color;

pub struct GitHubDark;

impl Theme for GitHubDark {
    fn name(&self) -> &'static str {
        "GitHub Dark"
    }

    fn theme_type(&self) -> ThemeType {
        ThemeType::Dark
    }

    fn colors(&self) -> SemanticColors {
        let bg_primary = Color::Rgb(13, 17, 23);
        let gray = Color::Rgb(139, 148, 158);

        SemanticColors {
            text: TextColors {
                primary: Color::Rgb(201, 209, 217),
                secondary: gray,
                link: Color::Rgb(88, 166, 255),
                accent: Color::Rgb(163, 113, 247),
                response: Color::Rgb(201, 209, 217),
            },
            background: BackgroundColors {
                primary: bg_primary,
                message: interpolate_color(bg_primary, gray, 0.15),
                input: interpolate_color(bg_primary, gray, 0.1),
                diff: DiffColors {
                    added: Color::Rgb(46, 160, 67),
                    removed: Color::Rgb(248, 81, 73),
                },
            },
            border: BorderColors {
                default: Color::Rgb(48, 54, 61),
                focused: Color::Rgb(88, 166, 255),
            },
            ui: UiColors {
                comment: Color::Rgb(110, 118, 129),
                symbol: Color::Rgb(255, 123, 114),
                dark: Color::Rgb(33, 38, 45),
                gradient: [
                    Color::Rgb(88, 166, 255),
                    Color::Rgb(163, 113, 247),
                    Color::Rgb(255, 123, 114),
                ],
            },
            status: StatusColors {
                error: Color::Rgb(248, 81, 73),
                success: Color::Rgb(63, 185, 80),
                warning: Color::Rgb(210, 153, 34),
            },
            scrollbar: ScrollbarColors {
                thumb: gray,
                thumb_hover: Color::Rgb(88, 166, 255),
                track: Color::Rgb(33, 38, 45),
            },
        }
    }
}
