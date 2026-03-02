use crate::ui::semantic_colors::SemanticColors;
use crate::ui::themes::{
    AtomOneDark, AyuDark, DefaultDark, DefaultLight, Dracula, GitHubDark, GitHubLight,
    SolarizedDark, SolarizedLight, Theme, ThemeType,
};
use std::sync::{Arc, RwLock};

pub struct ThemeManager {
    themes: Vec<Arc<dyn Theme>>,
    active_theme: Arc<dyn Theme>,
}

impl ThemeManager {
    pub fn new() -> Self {
        let themes: Vec<Arc<dyn Theme>> = vec![
            Arc::new(DefaultDark),
            Arc::new(DefaultLight),
            Arc::new(Dracula),
            Arc::new(GitHubDark),
            Arc::new(GitHubLight),
            Arc::new(SolarizedDark),
            Arc::new(SolarizedLight),
            Arc::new(AtomOneDark),
            Arc::new(AyuDark),
        ];

        let active_theme = themes[0].clone();

        Self {
            themes,
            active_theme,
        }
    }

    pub fn get_active_theme(&self) -> &Arc<dyn Theme> {
        &self.active_theme
    }

    pub fn get_colors(&self) -> SemanticColors {
        self.active_theme.colors()
    }

    pub fn set_theme(&mut self, name: &str) -> bool {
        if let Some(theme) = self.themes.iter().find(|t| t.name() == name) {
            self.active_theme = theme.clone();
            true
        } else {
            false
        }
    }

    pub fn get_available_themes(&self) -> Vec<(&'static str, ThemeType)> {
        self.themes
            .iter()
            .map(|t| (t.name(), t.theme_type()))
            .collect()
    }

    pub fn next_theme(&mut self) {
        if let Some(current_idx) = self
            .themes
            .iter()
            .position(|t| t.name() == self.active_theme.name())
        {
            let next_idx = (current_idx + 1) % self.themes.len();
            self.active_theme = self.themes[next_idx].clone();
        }
    }

    pub fn previous_theme(&mut self) {
        if let Some(current_idx) = self
            .themes
            .iter()
            .position(|t| t.name() == self.active_theme.name())
        {
            let prev_idx = if current_idx == 0 {
                self.themes.len() - 1
            } else {
                current_idx - 1
            };
            self.active_theme = self.themes[prev_idx].clone();
        }
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

lazy_static::lazy_static! {
    pub static ref THEME_MANAGER: RwLock<ThemeManager> = RwLock::new(ThemeManager::new());
}

pub fn get_theme() -> Arc<dyn Theme> {
    THEME_MANAGER.read().unwrap().get_active_theme().clone()
}

pub fn get_colors() -> SemanticColors {
    THEME_MANAGER.read().unwrap().get_colors()
}

pub fn set_theme(name: &str) -> bool {
    THEME_MANAGER.write().unwrap().set_theme(name)
}

pub fn next_theme() {
    THEME_MANAGER.write().unwrap().next_theme();
}

pub fn previous_theme() {
    THEME_MANAGER.write().unwrap().previous_theme();
}

pub fn get_available_themes() -> Vec<(&'static str, ThemeType)> {
    THEME_MANAGER.read().unwrap().get_available_themes()
}
