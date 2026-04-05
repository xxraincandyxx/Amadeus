// @amadeus-header
// summary: TUI module code for semantic colors.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::semantic_colors
// - type: crate::ui::semantic_colors::TextColors
// - type: crate::ui::semantic_colors::DiffColors
// - type: crate::ui::semantic_colors::BackgroundColors
// - type: crate::ui::semantic_colors::BorderColors
// - type: crate::ui::semantic_colors::UiColors
// - type: crate::ui::semantic_colors::StatusColors
// - type: crate::ui::semantic_colors::ScrollbarColors
// uses:
// - runtime: ratatui terminal rendering
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects: none
// tests:
// - tests/tui_snapshot_test.rs
// @end-amadeus-header

use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextColors {
    pub primary: Color,
    pub secondary: Color,
    pub link: Color,
    pub accent: Color,
    pub response: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiffColors {
    pub added: Color,
    pub removed: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackgroundColors {
    pub primary: Color,
    pub message: Color,
    pub input: Color,
    pub diff: DiffColors,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderColors {
    pub default: Color,
    pub focused: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiColors {
    pub comment: Color,
    pub symbol: Color,
    pub dark: Color,
    pub gradient: [Color; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusColors {
    pub error: Color,
    pub success: Color,
    pub warning: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollbarColors {
    pub thumb: Color,
    pub thumb_hover: Color,
    pub track: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SemanticColors {
    pub text: TextColors,
    pub background: BackgroundColors,
    pub border: BorderColors,
    pub ui: UiColors,
    pub status: StatusColors,
    pub scrollbar: ScrollbarColors,
}

impl SemanticColors {
    pub fn default_dark() -> Self {
        let bg_primary = Color::Rgb(30, 30, 46);
        let gray = Color::Rgb(108, 112, 134);

        Self {
            text: TextColors {
                primary: Color::Reset,
                secondary: gray,
                link: Color::Rgb(137, 180, 250),
                accent: Color::Rgb(203, 166, 247),
                response: Color::Reset,
            },
            background: BackgroundColors {
                primary: bg_primary,
                message: interpolate_color(bg_primary, gray, 0.15),
                input: interpolate_color(bg_primary, gray, 0.1),
                diff: DiffColors {
                    added: Color::Rgb(40, 53, 11),
                    removed: Color::Rgb(67, 0, 0),
                },
            },
            border: BorderColors {
                default: interpolate_color(bg_primary, gray, 0.2),
                focused: Color::Rgb(137, 180, 250),
            },
            ui: UiColors {
                comment: gray,
                symbol: Color::Rgb(137, 220, 235),
                dark: interpolate_color(bg_primary, gray, 0.2),
                gradient: [
                    Color::Rgb(71, 150, 228),
                    Color::Rgb(132, 122, 206),
                    Color::Rgb(195, 103, 127),
                ],
            },
            status: StatusColors {
                error: Color::Rgb(243, 139, 168),
                success: Color::Rgb(166, 227, 161),
                warning: Color::Rgb(249, 226, 175),
            },
            scrollbar: ScrollbarColors {
                thumb: gray,
                thumb_hover: Color::Rgb(137, 180, 250),
                track: interpolate_color(bg_primary, gray, 0.1),
            },
        }
    }

    pub fn default_light() -> Self {
        let bg_primary = Color::Rgb(250, 250, 250);
        let gray = Color::Rgb(151, 160, 176);

        Self {
            text: TextColors {
                primary: Color::Rgb(0, 0, 0),
                secondary: gray,
                link: Color::Rgb(59, 130, 246),
                accent: Color::Rgb(139, 92, 246),
                response: Color::Rgb(0, 0, 0),
            },
            background: BackgroundColors {
                primary: bg_primary,
                message: interpolate_color(bg_primary, gray, 0.15),
                input: interpolate_color(bg_primary, gray, 0.1),
                diff: DiffColors {
                    added: Color::Rgb(198, 234, 216),
                    removed: Color::Rgb(255, 204, 204),
                },
            },
            border: BorderColors {
                default: interpolate_color(bg_primary, gray, 0.2),
                focused: Color::Rgb(59, 130, 246),
            },
            ui: UiColors {
                comment: gray,
                symbol: Color::Rgb(6, 182, 212),
                dark: interpolate_color(bg_primary, gray, 0.2),
                gradient: [
                    Color::Rgb(71, 150, 228),
                    Color::Rgb(132, 122, 206),
                    Color::Rgb(195, 103, 127),
                ],
            },
            status: StatusColors {
                error: Color::Rgb(221, 76, 76),
                success: Color::Rgb(60, 168, 75),
                warning: Color::Rgb(213, 164, 10),
            },
            scrollbar: ScrollbarColors {
                thumb: gray,
                thumb_hover: Color::Rgb(59, 130, 246),
                track: interpolate_color(bg_primary, gray, 0.1),
            },
        }
    }
}

pub fn interpolate_color(color1: Color, color2: Color, factor: f32) -> Color {
    let (r1, g1, b1) = match color1 {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    };
    let (r2, g2, b2) = match color2 {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (255, 255, 255),
    };

    let factor = factor.clamp(0.0, 1.0);
    let r = (r1 as f32 + (r2 as f32 - r1 as f32) * factor) as u8;
    let g = (g1 as f32 + (g2 as f32 - g1 as f32) * factor) as u8;
    let b = (b1 as f32 + (b2 as f32 - b1 as f32) * factor) as u8;

    Color::Rgb(r, g, b)
}

impl Default for SemanticColors {
    fn default() -> Self {
        Self::default_dark()
    }
}
