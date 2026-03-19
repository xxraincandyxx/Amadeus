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
        // True dark background (near black)
        let bg_primary = Color::Rgb(12, 12, 12);
        let bg_dark = Color::Rgb(8, 8, 8);
        // Muted gray for secondary text
        let gray = Color::Rgb(90, 90, 90);
        // Dark gray for borders/dividers
        let dark_gray = Color::Rgb(50, 50, 50);
        // Subtle blood red for accents only
        let blood_red = Color::Rgb(120, 50, 50);

        SemanticColors {
            text: TextColors {
                primary: Color::Rgb(220, 220, 220),
                secondary: gray,
                link: Color::Rgb(150, 130, 130),
                accent: blood_red,
                response: Color::Rgb(200, 200, 200),
            },
            background: BackgroundColors {
                primary: bg_primary,
                message: bg_primary,
                input: Color::Rgb(18, 18, 18),
                diff: DiffColors {
                    added: Color::Rgb(20, 40, 20),
                    removed: Color::Rgb(50, 20, 20),
                },
            },
            border: BorderColors {
                default: dark_gray,
                focused: Color::Rgb(100, 60, 60),
            },
            ui: UiColors {
                comment: gray,
                symbol: Color::Rgb(140, 100, 100),
                dark: dark_gray,
                gradient: [
                    Color::Rgb(80, 40, 40),
                    Color::Rgb(100, 50, 50),
                    Color::Rgb(120, 60, 60),
                ],
            },
            status: StatusColors {
                error: Color::Rgb(180, 80, 80),
                success: Color::Rgb(100, 160, 100),
                warning: Color::Rgb(180, 140, 80),
            },
            scrollbar: ScrollbarColors {
                thumb: dark_gray,
                thumb_hover: Color::Rgb(100, 60, 60),
                track: bg_dark,
            },
        }
    }
}
