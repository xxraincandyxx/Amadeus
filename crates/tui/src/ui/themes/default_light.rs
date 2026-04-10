// @amadeus-header
// summary: Built-in default light TUI theme definition.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::themes::default_light
// - type: crate::ui::themes::default_light::DefaultLight
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

pub struct DefaultLight;

impl Theme for DefaultLight {
    fn name(&self) -> &'static str {
        "Default Light"
    }

    fn theme_type(&self) -> ThemeType {
        ThemeType::Light
    }

    fn colors(&self) -> SemanticColors {
        SemanticColors::default_light()
    }
}
