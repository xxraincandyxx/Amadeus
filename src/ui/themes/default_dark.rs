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
