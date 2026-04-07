// @amadeus-header
// summary: Built-in dark red TUI theme definition.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::themes::dark_red
// - type: crate::ui::themes::dark_red::DarkRed
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

pub struct DarkRed;

impl Theme for DarkRed {
    fn name(&self) -> &'static str {
        "Dark Red"
    }

    fn theme_type(&self) -> ThemeType {
        ThemeType::Dark
    }

    fn colors(&self) -> SemanticColors {
        // True dark background with a very subtle warm/crimson undertone
        let bg_primary = Color::Rgb(10, 5, 5);
        let bg_dark = Color::Rgb(5, 2, 2);

        // Red-tinted grays to remove the orange/muddy feel
        let gray = Color::Rgb(130, 100, 100);
        let dark_gray = Color::Rgb(70, 40, 40);

        // Pure, visceral blood red (High red, very low green/blue)
        let blood_red = Color::Rgb(170, 15, 15);
        let dark_blood = Color::Rgb(110, 5, 5);
        let bright_blood = Color::Rgb(210, 20, 20);

        SemanticColors {
            text: TextColors {
                primary: Color::Rgb(230, 215, 215), // Very slightly tinted white
                secondary: gray,
                link: bright_blood,
                accent: blood_red,
                response: Color::Rgb(220, 210, 210),
            },
            background: BackgroundColors {
                primary: bg_primary,
                message: Color::Rgb(25, 12, 12),
                input: Color::Rgb(18, 8, 8),
                diff: DiffColors {
                    added: Color::Rgb(30, 50, 30),
                    removed: Color::Rgb(80, 15, 15),
                },
            },
            border: BorderColors {
                default: dark_gray,
                focused: bright_blood,
            },
            ui: UiColors {
                comment: gray,
                symbol: bright_blood, // Make symbols pop with striking red
                dark: dark_gray,
                gradient: [
                    Color::Rgb(60, 0, 0), // Deepest shadow blood
                    dark_blood,           // Dark blood
                    blood_red,            // Fresh blood
                ],
            },
            status: StatusColors {
                error: Color::Rgb(240, 10, 10), // Piercing pure red
                success: Color::Rgb(80, 160, 80),
                warning: Color::Rgb(200, 80, 20), // More rust/ember than orange
            },
            scrollbar: ScrollbarColors {
                thumb: dark_gray,
                thumb_hover: blood_red,
                track: bg_dark,
            },
        }
    }
}
