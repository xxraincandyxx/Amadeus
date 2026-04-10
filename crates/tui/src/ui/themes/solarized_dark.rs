// @amadeus-header
// summary: Built-in solarized dark TUI theme definition.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::themes::solarized_dark
// - type: crate::ui::themes::solarized_dark::SolarizedDark
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

pub struct SolarizedDark;

impl Theme for SolarizedDark {
    fn name(&self) -> &'static str {
        "Solarized Dark"
    }

    fn theme_type(&self) -> ThemeType {
        ThemeType::Dark
    }

    fn colors(&self) -> SemanticColors {
        let bg_primary = Color::Rgb(0, 43, 54);
        let base0 = Color::Rgb(131, 148, 150);

        SemanticColors {
            text: TextColors {
                primary: Color::Rgb(253, 246, 227),
                secondary: base0,
                link: Color::Rgb(38, 139, 210),
                accent: Color::Rgb(211, 54, 130),
                response: Color::Rgb(253, 246, 227),
            },
            background: BackgroundColors {
                primary: bg_primary,
                message: interpolate_color(bg_primary, base0, 0.15),
                input: interpolate_color(bg_primary, base0, 0.1),
                diff: DiffColors {
                    added: Color::Rgb(0, 64, 48),
                    removed: Color::Rgb(64, 0, 32),
                },
            },
            border: BorderColors {
                default: Color::Rgb(7, 54, 66),
                focused: Color::Rgb(38, 139, 210),
            },
            ui: UiColors {
                comment: Color::Rgb(101, 123, 131),
                symbol: Color::Rgb(42, 161, 152),
                dark: Color::Rgb(7, 54, 66),
                gradient: [
                    Color::Rgb(38, 139, 210),
                    Color::Rgb(211, 54, 130),
                    Color::Rgb(42, 161, 152),
                ],
            },
            status: StatusColors {
                error: Color::Rgb(220, 50, 47),
                success: Color::Rgb(133, 153, 0),
                warning: Color::Rgb(181, 137, 0),
            },
            scrollbar: ScrollbarColors {
                thumb: base0,
                thumb_hover: Color::Rgb(38, 139, 210),
                track: Color::Rgb(7, 54, 66),
            },
        }
    }
}
