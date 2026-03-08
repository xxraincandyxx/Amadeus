use std::time::Instant;

use ratatui::style::Color;

use crate::ui::constants::COLOR_CYCLE_DURATION_MS;

const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Debug, Clone)]
pub struct GradientColor {
    r: f64,
    g: f64,
    b: f64,
}

impl GradientColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f64,
            g: g as f64,
            b: b as f64,
        }
    }

    pub fn to_color(&self) -> Color {
        Color::Rgb(
            self.r.round().clamp(0.0, 255.0) as u8,
            self.g.round().clamp(0.0, 255.0) as u8,
            self.b.round().clamp(0.0, 255.0) as u8,
        )
    }

    pub fn lerp(&self, other: &Self, t: f64) -> Self {
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
        }
    }
}

const BRAND_COLORS: [GradientColor; 6] = [
    GradientColor::new(162, 93, 220), // Purple
    GradientColor::new(66, 133, 244), // Blue
    GradientColor::new(0, 188, 212),  // Cyan
    GradientColor::new(52, 168, 83),  // Green
    GradientColor::new(251, 188, 4),  // Yellow
    GradientColor::new(234, 67, 53),  // Red
];

pub struct GeminiSpinner {
    frame: usize,
    start_time: Instant,
    active: bool,
}

impl GeminiSpinner {
    pub fn new() -> Self {
        Self {
            frame: 0,
            start_time: Instant::now(),
            active: false,
        }
    }

    pub fn start(&mut self) {
        self.active = true;
        self.start_time = Instant::now();
    }

    pub fn stop(&mut self) {
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn tick(&mut self) {
        self.frame = (self.frame + 1) % SPINNER_FRAMES.len();
    }

    fn get_gradient_color(&self, progress: f64) -> GradientColor {
        let gradient_colors: Vec<&GradientColor> = BRAND_COLORS
            .iter()
            .chain(std::iter::once(&BRAND_COLORS[0]))
            .collect();

        let segments = gradient_colors.len() - 1;
        let segment_progress = progress * segments as f64;
        let segment_index = segment_progress.floor() as usize;
        let local_progress = segment_progress - segment_index as f64;

        let clamped_index = segment_index.min(segments - 1);
        let next_index = (clamped_index + 1).min(segments);

        gradient_colors[clamped_index].lerp(gradient_colors[next_index], local_progress)
    }

    pub fn get_frame(&self) -> &'static str {
        SPINNER_FRAMES[self.frame]
    }

    pub fn frame_index(&self) -> usize {
        self.frame
    }

    pub fn get_current_color(&self) -> Color {
        if !self.active {
            return Color::Reset;
        }

        let elapsed = self.start_time.elapsed().as_millis() as u64;
        let cycle_duration = COLOR_CYCLE_DURATION_MS;
        let progress = (elapsed % cycle_duration) as f64 / cycle_duration as f64;

        self.get_gradient_color(progress).to_color()
    }

    pub fn get_static_spinner(&self, color: Color) -> (String, Color) {
        (SPINNER_FRAMES[self.frame].to_string(), color)
    }
}

impl Default for GeminiSpinner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gradient_color_lerp() {
        let c1 = GradientColor::new(255, 0, 0);
        let c2 = GradientColor::new(0, 0, 255);

        let mid = c1.lerp(&c2, 0.5);
        assert_eq!(mid.r, 127.5);
        assert_eq!(mid.g, 0.0);
        assert_eq!(mid.b, 127.5);
    }

    #[test]
    fn test_gradient_color_to_color() {
        let c = GradientColor::new(128, 64, 192);
        let color = c.to_color();

        match color {
            Color::Rgb(r, g, b) => {
                assert_eq!(r, 128);
                assert_eq!(g, 64);
                assert_eq!(b, 192);
            }
            _ => panic!("Expected Rgb color"),
        }
    }
}
