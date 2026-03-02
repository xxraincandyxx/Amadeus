pub mod dracula;
pub mod github_dark;
pub mod github_light;
pub mod solarized_dark;
pub mod solarized_light;
pub mod atom_one_dark;
pub mod ayu_dark;
pub mod default_dark;
pub mod default_light;

pub use dracula::Dracula;
pub use github_dark::GitHubDark;
pub use github_light::GitHubLight;
pub use solarized_dark::SolarizedDark;
pub use solarized_light::SolarizedLight;
pub use atom_one_dark::AtomOneDark;
pub use ayu_dark::AyuDark;
pub use default_dark::DefaultDark;
pub use default_light::DefaultLight;

use crate::ui::semantic_colors::SemanticColors;
use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeType {
    Dark,
    Light,
    Ansi,
}

pub trait Theme: Send + Sync {
    fn name(&self) -> &'static str;
    fn theme_type(&self) -> ThemeType;
    fn colors(&self) -> SemanticColors;
    fn background(&self) -> Color {
        self.colors().background.primary
    }
}
