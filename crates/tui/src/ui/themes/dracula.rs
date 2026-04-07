// @amadeus-header
// summary: Built-in dracula TUI theme definition.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::themes::dracula
// - type: crate::ui::themes::dracula::Dracula
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
    BackgroundColors, BorderColors, DiffColors, ScrollbarColors, SemanticColors, StatusColors,
    TextColors, UiColors,
};
use ratatui::style::Color;

pub struct Dracula;

impl Theme for Dracula {
    fn name(&self) -> &'static str {
        "Dracula"
    }

    fn theme_type(&self) -> ThemeType {
        ThemeType::Dark
    }

    fn colors(&self) -> SemanticColors {
        let bg_primary = Color::Rgb(40, 42, 54);
        let gray = Color::Rgb(98, 114, 164);

        SemanticColors {
            text: TextColors {
                primary: Color::Rgb(248, 248, 242),
                secondary: gray,
                link: Color::Rgb(139, 233, 253),
                accent: Color::Rgb(189, 147, 249),
                response: Color::Rgb(248, 248, 242),
            },
            background: BackgroundColors {
                primary: bg_primary,
                message: Color::Rgb(33, 35, 44),
                input: Color::Rgb(44, 46, 58),
                diff: DiffColors {
                    added: Color::Rgb(40, 80, 60),
                    removed: Color::Rgb(80, 40, 50),
                },
            },
            border: BorderColors {
                default: Color::Rgb(68, 71, 90),
                focused: Color::Rgb(139, 233, 253),
            },
            ui: UiColors {
                comment: gray,
                symbol: Color::Rgb(139, 233, 253),
                dark: Color::Rgb(68, 71, 90),
                gradient: [
                    Color::Rgb(255, 121, 198),
                    Color::Rgb(189, 147, 249),
                    Color::Rgb(139, 233, 253),
                ],
            },
            status: StatusColors {
                error: Color::Rgb(255, 85, 85),
                success: Color::Rgb(80, 250, 123),
                warning: Color::Rgb(255, 184, 108),
            },
            scrollbar: ScrollbarColors {
                thumb: gray,
                thumb_hover: Color::Rgb(139, 233, 253),
                track: Color::Rgb(68, 71, 90),
            },
        }
    }
}
