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
