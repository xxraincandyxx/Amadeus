use super::{Theme, ThemeType};
use crate::ui::semantic_colors::{
    interpolate_color, BackgroundColors, BorderColors, DiffColors, ScrollbarColors, SemanticColors,
    StatusColors, TextColors, UiColors,
};
use ratatui::style::Color;

pub struct GitHubLight;

impl Theme for GitHubLight {
    fn name(&self) -> &'static str {
        "GitHub Light"
    }

    fn theme_type(&self) -> ThemeType {
        ThemeType::Light
    }

    fn colors(&self) -> SemanticColors {
        let bg_primary = Color::Rgb(255, 255, 255);
        let gray = Color::Rgb(110, 118, 129);

        SemanticColors {
            text: TextColors {
                primary: Color::Rgb(36, 41, 46),
                secondary: gray,
                link: Color::Rgb(3, 102, 214),
                accent: Color::Rgb(111, 66, 193),
                response: Color::Rgb(36, 41, 46),
            },
            background: BackgroundColors {
                primary: bg_primary,
                message: interpolate_color(bg_primary, gray, 0.15),
                input: interpolate_color(bg_primary, gray, 0.1),
                diff: DiffColors {
                    added: Color::Rgb(236, 255, 236),
                    removed: Color::Rgb(255, 236, 236),
                },
            },
            border: BorderColors {
                default: Color::Rgb(225, 228, 232),
                focused: Color::Rgb(3, 102, 214),
            },
            ui: UiColors {
                comment: Color::Rgb(106, 115, 125),
                symbol: Color::Rgb(215, 58, 73),
                dark: Color::Rgb(246, 248, 250),
                gradient: [
                    Color::Rgb(3, 102, 214),
                    Color::Rgb(111, 66, 193),
                    Color::Rgb(215, 58, 73),
                ],
            },
            status: StatusColors {
                error: Color::Rgb(203, 36, 49),
                success: Color::Rgb(40, 167, 69),
                warning: Color::Rgb(159, 110, 28),
            },
            scrollbar: ScrollbarColors {
                thumb: gray,
                thumb_hover: Color::Rgb(3, 102, 214),
                track: Color::Rgb(246, 248, 250),
            },
        }
    }
}
