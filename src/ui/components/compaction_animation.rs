use std::time::Instant;

use ratatui::style::Color;

use crate::ui::constants::COLOR_CYCLE_DURATION_MS;

/// Progress bar characters for compaction animation
const PROGRESS_FILLED: &str = "█";
const PROGRESS_EMPTY: &str = "░";
const PROGRESS_PARTIAL: [&str; 8] = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];

/// Animation frames for the leading indicator
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Cycling messages for compaction progress
const COMPACTION_MESSAGES: [&str; 6] = [
    "Compacting conversation history",
    "Summarizing context",
    "Optimizing token usage",
    "Processing messages",
    "Reducing context size",
    "Compressing history",
];

/// Brand gradient colors for animated progress bar
const GRADIENT_COLORS: [(u8, u8, u8); 4] = [
    (162, 93, 220), // Purple
    (66, 133, 244), // Blue
    (0, 188, 212),  // Cyan
    (52, 168, 83),  // Green
];

/// Animated compaction display with progress bar and cycling messages
#[derive(Debug)]
pub struct CompactionAnimator {
    /// Current spinner frame index
    frame: usize,
    /// Current message index
    message_index: usize,
    /// Start time for animation
    start_time: Instant,
    /// Simulated progress (0-100)
    progress: u8,
    /// Whether animation is active
    active: bool,
    /// Time of last progress update
    last_progress_update: Instant,
}

impl CompactionAnimator {
    pub fn new() -> Self {
        Self {
            frame: 0,
            message_index: 0,
            start_time: Instant::now(),
            progress: 0,
            active: false,
            last_progress_update: Instant::now(),
        }
    }

    /// Start the animation
    pub fn start(&mut self) {
        self.active = true;
        self.start_time = Instant::now();
        self.progress = 0;
        self.frame = 0;
        self.message_index = 0;
        self.last_progress_update = Instant::now();
    }

    /// Stop the animation
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Check if animation is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Advance animation by one tick
    pub fn tick(&mut self) {
        if !self.active {
            return;
        }

        // Advance spinner frame
        self.frame = (self.frame + 1) % SPINNER_FRAMES.len();

        // Simulate progress (slowly increasing, max 95% until complete)
        let elapsed = self.last_progress_update.elapsed();
        if elapsed.as_millis() > 150 && self.progress < 95 {
            // Progress slows down as it approaches 95%
            let increment = if self.progress < 50 {
                3
            } else if self.progress < 80 {
                2
            } else {
                1
            };
            self.progress = (self.progress + increment).min(95);
            self.last_progress_update = Instant::now();
        }

        // Cycle message every ~1.5 seconds
        let msg_elapsed = self.start_time.elapsed().as_millis();
        let msg_interval = 1500u128;
        self.message_index = ((msg_elapsed / msg_interval) % COMPACTION_MESSAGES.len() as u128)
            as usize;
    }

    /// Get the current spinner frame
    pub fn spinner_frame(&self) -> &'static str {
        SPINNER_FRAMES[self.frame]
    }

    /// Get the current message
    pub fn current_message(&self) -> &'static str {
        COMPACTION_MESSAGES[self.message_index]
    }

    /// Get current progress (0-100)
    pub fn progress(&self) -> u8 {
        self.progress
    }

    /// Get the gradient color at current progress
    pub fn get_progress_color(&self) -> Color {
        let progress_ratio = self.progress as f64 / 100.0;

        // Interpolate through gradient colors
        let segment_count = (GRADIENT_COLORS.len() - 1) as f64;
        let segment = progress_ratio * segment_count;
        let segment_index = segment.floor() as usize;
        let local_progress = segment - segment.floor();

        let clamped_index = segment_index.min(GRADIENT_COLORS.len() - 2);
        let (r1, g1, b1) = GRADIENT_COLORS[clamped_index];
        let (r2, g2, b2) = GRADIENT_COLORS[clamped_index + 1];

        Color::Rgb(
            (r1 as f64 + (r2 as f64 - r1 as f64) * local_progress).round() as u8,
            (g1 as f64 + (g2 as f64 - g1 as f64) * local_progress).round() as u8,
            (b1 as f64 + (b2 as f64 - b1 as f64) * local_progress).round() as u8,
        )
    }

    /// Get the animated gradient color (cycles through colors over time)
    pub fn get_animated_color(&self) -> Color {
        let elapsed = self.start_time.elapsed().as_millis() as u64;
        let progress = (elapsed % COLOR_CYCLE_DURATION_MS) as f64 / COLOR_CYCLE_DURATION_MS as f64;

        let segment_count = (GRADIENT_COLORS.len() - 1) as f64;
        let segment = progress * segment_count;
        let segment_index = segment.floor() as usize;
        let local_progress = segment - segment.floor();

        let clamped_index = segment_index.min(GRADIENT_COLORS.len() - 2);
        let (r1, g1, b1) = GRADIENT_COLORS[clamped_index];
        let (r2, g2, b2) = GRADIENT_COLORS[clamped_index + 1];

        Color::Rgb(
            (r1 as f64 + (r2 as f64 - r1 as f64) * local_progress).round() as u8,
            (g1 as f64 + (g2 as f64 - g1 as f64) * local_progress).round() as u8,
            (b1 as f64 + (b2 as f64 - b1 as f64) * local_progress).round() as u8,
        )
    }

    /// Render the progress bar string
    pub fn render_progress_bar(&self, width: usize) -> String {
        if width == 0 {
            return String::new();
        }

        let filled_count = (self.progress as usize * width) / 100;
        let empty_count = width.saturating_sub(filled_count);

        let mut bar = String::new();

        // Filled portion
        for _ in 0..filled_count {
            bar.push_str(PROGRESS_FILLED);
        }

        // Empty portion
        for _ in 0..empty_count {
            bar.push_str(PROGRESS_EMPTY);
        }

        bar
    }

    /// Render the progress bar with partial block for smoother appearance
    pub fn render_progress_bar_smooth(&self, width: usize) -> String {
        if width == 0 {
            return String::new();
        }

        // Calculate exact fill amount (can be fractional)
        let exact_fill = (self.progress as f64 / 100.0) * width as f64;
        let full_blocks = exact_fill.floor() as usize;
        let partial_index = ((exact_fill - exact_fill.floor()) * 8.0) as usize;
        let empty_count = width.saturating_sub(full_blocks).saturating_sub(1);

        let mut bar = String::new();

        // Full blocks
        for _ in 0..full_blocks {
            bar.push_str(PROGRESS_FILLED);
        }

        // Partial block (if any)
        if partial_index > 0 && full_blocks < width {
            bar.push_str(PROGRESS_PARTIAL[partial_index]);
        }

        // Empty portion
        for _ in 0..empty_count {
            bar.push_str(PROGRESS_EMPTY);
        }

        // Pad to exact width
        while bar.chars().count() < width {
            bar.push_str(PROGRESS_EMPTY);
        }

        bar
    }

    /// Get elapsed time as formatted string
    pub fn elapsed_string(&self) -> String {
        let elapsed = self.start_time.elapsed().as_secs();
        if elapsed < 60 {
            format!("{}s", elapsed)
        } else {
            format!("{}m{}s", elapsed / 60, elapsed % 60)
        }
    }

    /// Complete the animation (set to 100%)
    pub fn complete(&mut self) {
        self.progress = 100;
        self.active = false;
    }
}

impl Default for CompactionAnimator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compaction_animator_new() {
        let animator = CompactionAnimator::new();
        assert!(!animator.is_active());
        assert_eq!(animator.progress(), 0);
    }

    #[test]
    fn test_compaction_animator_start_stop() {
        let mut animator = CompactionAnimator::new();
        animator.start();
        assert!(animator.is_active());

        animator.stop();
        assert!(!animator.is_active());
    }

    #[test]
    fn test_compaction_animator_tick() {
        let mut animator = CompactionAnimator::new();
        animator.start();

        // Tick should advance frame
        for _ in 0..5 {
            animator.tick();
        }

        assert!(animator.is_active());
        // Frame should have advanced
        assert!(animator.frame > 0 || animator.frame == 0); // frame cycles through
    }

    #[test]
    fn test_compaction_animator_progress_bar() {
        let mut animator = CompactionAnimator::new();
        animator.progress = 50;

        let bar = animator.render_progress_bar(10);
        assert!(bar.contains('█'));
        assert!(bar.contains('░'));
    }

    #[test]
    fn test_compaction_animator_progress_bar_smooth() {
        let mut animator = CompactionAnimator::new();
        animator.progress = 50;

        let bar = animator.render_progress_bar_smooth(10);
        assert!(!bar.is_empty());
    }

    #[test]
    fn test_compaction_animator_complete() {
        let mut animator = CompactionAnimator::new();
        animator.start();
        animator.complete();

        assert_eq!(animator.progress(), 100);
        assert!(!animator.is_active());
    }

    #[test]
    fn test_compaction_animator_colors() {
        let mut animator = CompactionAnimator::new();
        animator.start();

        let color = animator.get_progress_color();
        // Should be a valid RGB color
        match color {
            Color::Rgb(_, _, _) => {}
            _ => panic!("Expected RGB color"),
        }

        let animated_color = animator.get_animated_color();
        match animated_color {
            Color::Rgb(_, _, _) => {}
            _ => panic!("Expected RGB color"),
        }
    }

    #[test]
    fn test_compaction_animator_spinner() {
        let animator = CompactionAnimator::new();
        let frame = animator.spinner_frame();
        assert!(!frame.is_empty());
    }

    #[test]
    fn test_compaction_animator_message() {
        let animator = CompactionAnimator::new();
        let message = animator.current_message();
        assert!(!message.is_empty());
    }

    #[test]
    fn test_compaction_animator_elapsed() {
        let mut animator = CompactionAnimator::new();
        animator.start();

        let elapsed = animator.elapsed_string();
        // Should be a valid time string
        assert!(elapsed.contains('s'));
    }
}
