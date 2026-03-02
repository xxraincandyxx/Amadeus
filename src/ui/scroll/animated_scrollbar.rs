use std::time::{Duration, Instant};

use ratatui::style::Color;

use crate::ui::semantic_colors::interpolate_color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationPhase {
    Hidden,
    FadeIn,
    Visible,
    FadeOut,
}

#[derive(Debug, Clone)]
pub struct AnimatedScrollbar {
    phase: AnimationPhase,
    opacity: f32,
    animation_start: Instant,
    last_interaction: Instant,
    unfocused_color: Color,
    focused_color: Color,
}

const FADE_IN_DURATION_MS: u64 = 200;
const VISIBLE_DURATION_MS: u64 = 1000;
const FADE_OUT_DURATION_MS: u64 = 300;

impl AnimatedScrollbar {
    pub fn new(unfocused_color: Color, focused_color: Color) -> Self {
        Self {
            phase: AnimationPhase::Hidden,
            opacity: 0.0,
            animation_start: Instant::now(),
            last_interaction: Instant::now(),
            unfocused_color,
            focused_color,
        }
    }

    pub fn flash(&mut self) {
        self.last_interaction = Instant::now();

        match self.phase {
            AnimationPhase::Hidden => {
                self.phase = AnimationPhase::FadeIn;
                self.animation_start = Instant::now();
            }
            AnimationPhase::FadeIn | AnimationPhase::Visible => {
                self.animation_start = Instant::now();
            }
            AnimationPhase::FadeOut => {
                self.phase = AnimationPhase::FadeIn;
                self.animation_start = Instant::now();
            }
        }
    }

    pub fn hide(&mut self) {
        self.phase = AnimationPhase::Hidden;
        self.opacity = 0.0;
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.animation_start);

        match self.phase {
            AnimationPhase::Hidden => {
                self.opacity = 0.0;
            }
            AnimationPhase::FadeIn => {
                let progress = elapsed.as_millis() as f32 / FADE_IN_DURATION_MS as f32;
                if progress >= 1.0 {
                    self.opacity = 1.0;
                    self.phase = AnimationPhase::Visible;
                    self.animation_start = now;
                } else {
                    self.opacity = progress;
                }
            }
            AnimationPhase::Visible => {
                self.opacity = 1.0;
                if elapsed >= Duration::from_millis(VISIBLE_DURATION_MS) {
                    self.phase = AnimationPhase::FadeOut;
                    self.animation_start = now;
                }
            }
            AnimationPhase::FadeOut => {
                let progress = elapsed.as_millis() as f32 / FADE_OUT_DURATION_MS as f32;
                if progress >= 1.0 {
                    self.opacity = 0.0;
                    self.phase = AnimationPhase::Hidden;
                } else {
                    self.opacity = 1.0 - progress;
                }
            }
        }
    }

    pub fn thumb_color(&self) -> Color {
        interpolate_color(self.unfocused_color, self.focused_color, self.opacity)
    }

    pub fn is_visible(&self) -> bool {
        self.phase != AnimationPhase::Hidden || self.opacity > 0.0
    }

    pub fn opacity(&self) -> f32 {
        self.opacity
    }
}
