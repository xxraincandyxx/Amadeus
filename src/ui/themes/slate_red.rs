use super::{Theme, ThemeType};
use crate::ui::semantic_colors::{
    BackgroundColors, BorderColors, DiffColors, ScrollbarColors, SemanticColors, StatusColors,
    TextColors, UiColors,
};
use ratatui::style::Color;

pub struct SlateRed;

impl Theme for SlateRed {
    fn name(&self) -> &'static str {
        "Slate Red"
    }

    fn theme_type(&self) -> ThemeType {
        ThemeType::Dark
    }

    fn colors(&self) -> SemanticColors {
        // Dark gray base — no red in backgrounds
        let bg_primary = Color::Rgb(26, 26, 26);
        let bg_dark = Color::Rgb(18, 18, 18);

        // Grays — neutral, no warm tint
        let gray = Color::Rgb(120, 120, 120);
        let dark_gray = Color::Rgb(50, 50, 50);

        // Dark red — used only for accents (borders, links, symbols)
        let accent_red = Color::Rgb(140, 20, 20);
        let bright_red = Color::Rgb(180, 30, 30);

        SemanticColors {
            text: TextColors {
                primary: Color::Rgb(220, 220, 220), // Off-white, not pure white
                secondary: gray,
                link: accent_red,                      // Red accent for links
                accent: bright_red,                   // Red accent for emphasis
                response: Color::Rgb(200, 200, 200), // Slightly dimmer for responses
            },
            background: BackgroundColors {
                primary: bg_primary,
                message: Color::Rgb(35, 35, 35),    // Subtle lift from primary
                input: Color::Rgb(22, 22, 22),      // Slightly darker than primary
                diff: DiffColors {
                    added: Color::Rgb(30, 50, 30), // Muted green — stays neutral
                    removed: Color::Rgb(60, 20, 20), // Muted red
                },
            },
            border: BorderColors {
                default: dark_gray,   // Neutral gray border
                focused: accent_red, // Red accent on focus
            },
            ui: UiColors {
                comment: gray,            // Neutral gray for comments
                symbol: accent_red,       // Red accent for symbols (arrows, bullets)
                dark: dark_gray,          // Neutral dark gray
                gradient: [
                    Color::Rgb(30, 10, 10), // Very subtle red undertone in gradient
                    Color::Rgb(50, 15, 15),
                    Color::Rgb(70, 20, 20),
                ],
            },
            status: StatusColors {
                error: bright_red,                    // Semantic red for errors
                success: Color::Rgb(70, 140, 70),    // Muted green
                warning: Color::Rgb(160, 100, 30),  // Muted amber
            },
            scrollbar: ScrollbarColors {
                thumb: dark_gray,
                thumb_hover: accent_red, // Red on hover
                track: bg_dark,
            },
        }
    }
}
