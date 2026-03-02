use super::{Theme, ThemeType};
use crate::ui::semantic_colors::{
    interpolate_color, BackgroundColors, BorderColors, DiffColors, ScrollbarColors, SemanticColors,
    StatusColors, TextColors, UiColors,
};
use ratatui::style::Color;

pub struct AtomOneDark;

impl Theme for AtomOneDark {
    fn name(&self) -> &'static str {
        "Atom One Dark"
    }

    fn theme_type(&self) -> ThemeType {
        ThemeType::Dark
    }

    fn colors(&self) -> SemanticColors {
        let bg_primary = Color::Rgb(40, 44, 52);
        let gray = Color::Rgb(92, 99, 112);

        SemanticColors {
            text: TextColors {
                primary: Color::Rgb(171, 178, 191),
                secondary: gray,
                link: Color::Rgb(97, 175, 239),
                accent: Color::Rgb(198, 120, 221),
                response: Color::Rgb(171, 178, 191),
            },
            background: BackgroundColors {
                primary: bg_primary,
                message: interpolate_color(bg_primary, gray, 0.15),
                input: interpolate_color(bg_primary, gray, 0.1),
                diff: DiffColors {
                    added: Color::Rgb(80, 120, 80),
                    removed: Color::Rgb(120, 80, 80),
                },
            },
            border: BorderColors {
                default: Color::Rgb(55, 60, 70),
                focused: Color::Rgb(97, 175, 239),
            },
            ui: UiColors {
                comment: Color::Rgb(92, 99, 112),
                symbol: Color::Rgb(86, 182, 194),
                dark: Color::Rgb(33, 37, 43),
                gradient: [
                    Color::Rgb(97, 175, 239),
                    Color::Rgb(198, 120, 221),
                    Color::Rgb(224, 108, 117),
                ],
            },
            status: StatusColors {
                error: Color::Rgb(224, 108, 117),
                success: Color::Rgb(152, 195, 121),
                warning: Color::Rgb(229, 192, 123),
            },
            scrollbar: ScrollbarColors {
                thumb: gray,
                thumb_hover: Color::Rgb(97, 175, 239),
                track: Color::Rgb(33, 37, 43),
            },
        }
    }
}
