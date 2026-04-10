// @amadeus-header
// summary: Built-in default dark TUI theme definition.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::themes::default_dark
// - type: crate::ui::themes::default_dark::DefaultDark
// uses:
// - module: crate::ui::semantic_colors::SemanticColors
// invariants:
// - Theme definitions keep semantic roles visually consistent.
// side_effects: none
// tests:
// - tests/tui_snapshot_test.rs
// @end-amadeus-header

use super::{Theme, ThemeType};
use crate::ui::semantic_colors::SemanticColors;

pub struct DefaultDark;

impl Theme for DefaultDark {
    fn name(&self) -> &'static str {
        "Default Dark"
    }

    fn theme_type(&self) -> ThemeType {
        ThemeType::Dark
    }

    fn colors(&self) -> SemanticColors {
        SemanticColors::default_dark()
    }
}
