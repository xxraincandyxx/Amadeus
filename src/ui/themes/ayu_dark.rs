use super::{Theme, ThemeType};
use crate::ui::semantic_colors::{
    interpolate_color, BackgroundColors, BorderColors, DiffColors, ScrollbarColors, SemanticColors,
    StatusColors, TextColors, UiColors,
};
use ratatui::style::Color;

pub struct AyuDark;

impl Theme for AyuDark {
    fn name(&self) -> &'static str {
        "Ayu Dark"
    }

    fn theme_type(&self) -> ThemeType {
        ThemeType::Dark
    }

    fn colors(&self) -> SemanticColors {
        let bg_primary = Color::Rgb(15, 20, 25);
        let gray = Color::Rgb(92, 97, 108);

        SemanticColors {
            text: TextColors {
                primary: Color::Rgb(178, 182, 187),
                secondary: gray,
                link: Color::Rgb(59, 189, 232),
                accent: Color::Rgb(255, 107, 107),
                response: Color::Rgb(178, 182, 187),
            },
            background: BackgroundColors {
                primary: bg_primary,
                message: interpolate_color(bg_primary, gray, 0.15),
                input: interpolate_color(bg_primary, gray, 0.1),
                diff: DiffColors {
                    added: Color::Rgb(40, 80, 60),
                    removed: Color::Rgb(80, 40, 50),
                },
            },
            border: BorderColors {
                default: Color::Rgb(30, 35, 40),
                focused: Color::Rgb(59, 189, 232),
            },
            ui: UiColors {
                comment: Color::Rgb(92, 97, 108),
                symbol: Color::Rgb(255, 204, 102),
                dark: Color::Rgb(30, 35, 40),
                gradient: [
                    Color::Rgb(59, 189, 232),
                    Color::Rgb(255, 107, 107),
                    Color::Rgb(255, 204, 102),
                ],
            },
            status: StatusColors {
                error: Color::Rgb(255, 107, 107),
                success: Color::Rgb(134, 231, 171),
                warning: Color::Rgb(255, 204, 102),
            },
            scrollbar: ScrollbarColors {
                thumb: gray,
                thumb_hover: Color::Rgb(59, 189, 232),
                track: Color::Rgb(30, 35, 40),
            },
        }
    }
}
