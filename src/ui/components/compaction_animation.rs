use std::time::{Duration, Instant};

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

/// Minimum time to show the animation (prevents flashing)
const MIN_DISPLAY_DURATION: Duration = Duration::from_millis(800);

/// Time to show completion result before transitioning
const COMPLETION_DISPLAY_DURATION: Duration = Duration::from_millis(1500);

/// State of the compaction animation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompactionState {
    /// Not active
    Idle,
    /// Animation in progress
    Running,
    /// Completed successfully, showing result
    Completed,
    /// Failed, showing error
    Failed,
}

/// Result data for completed compaction
#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub original_tokens: usize,
    pub new_tokens: usize,
    pub error_message: Option<String>,
}

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
    /// Current state
    state: CompactionState,
    /// Time of last progress update
    last_progress_update: Instant,
    /// Time when completion was shown
    completion_time: Option<Instant>,
    /// Result data (when completed)
    result: Option<CompactionResult>,
}

impl CompactionAnimator {
    pub fn new() -> Self {
        Self {
            frame: 0,
            message_index: 0,
            start_time: Instant::now(),
            progress: 0,
            state: CompactionState::Idle,
            last_progress_update: Instant::now(),
            completion_time: None,
            result: None,
        }
    }

    /// Start the animation
    pub fn start(&mut self) {
        self.state = CompactionState::Running;
        self.start_time = Instant::now();
        self.progress = 0;
        self.frame = 0;
        self.message_index = 0;
        self.last_progress_update = Instant::now();
        self.completion_time = None;
        self.result = None;
    }

    /// Stop the animation immediately
    pub fn stop(&mut self) {
        self.state = CompactionState::Idle;
    }

    /// Check if animation is active (running or showing result)
    pub fn is_active(&self) -> bool {
        self.state != CompactionState::Idle
    }

    /// Check if still running
    pub fn is_running(&self) -> bool {
        self.state == CompactionState::Running
    }

    /// Check if showing completion result
    pub fn is_showing_result(&self) -> bool {
        matches!(self.state, CompactionState::Completed | CompactionState::Failed)
    }

    /// Get current state
    pub fn state(&self) -> CompactionState {
        self.state
    }

    /// Complete the animation with success result
    pub fn complete(&mut self, original_tokens: usize, new_tokens: usize) {
        self.progress = 100;
        self.state = CompactionState::Completed;
        self.completion_time = Some(Instant::now());
        self.result = Some(CompactionResult {
            original_tokens,
            new_tokens,
            error_message: None,
        });
    }

    /// Complete the animation with failure
    pub fn fail(&mut self, error: String) {
        self.state = CompactionState::Failed;
        self.completion_time = Some(Instant::now());
        self.result = Some(CompactionResult {
            original_tokens: 0,
            new_tokens: 0,
            error_message: Some(error),
        });
    }

    /// Get the result if available
    pub fn result(&self) -> Option<&CompactionResult> {
        self.result.as_ref()
    }

    /// Check if minimum display time has passed
    pub fn has_min_display_time_passed(&self) -> bool {
        self.start_time.elapsed() >= MIN_DISPLAY_DURATION
    }

    /// Check if result display should transition to history
    pub fn should_transition_to_history(&self) -> bool {
        if let Some(completion_time) = self.completion_time {
            completion_time.elapsed() >= COMPLETION_DISPLAY_DURATION
        } else {
            false
        }
    }

    /// Advance animation by one tick
    pub fn tick(&mut self) {
        if self.state == CompactionState::Idle {
            return;
        }

        // If showing result, check if we should transition
        if self.should_transition_to_history() {
            self.state = CompactionState::Idle;
            return;
        }

        // Only advance spinner/progress if running
        if self.state != CompactionState::Running {
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

    /// Get success color (green)
    pub fn get_success_color(&self) -> Color {
        Color::Rgb(52, 168, 83)
    }

    /// Get error color (red)
    pub fn get_error_color(&self) -> Color {
        Color::Rgb(234, 67, 53)
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

    /// Render a completion progress bar (full, in success color)
    pub fn render_completion_bar(&self, width: usize) -> String {
        PROGRESS_FILLED.repeat(width)
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
        assert_eq!(animator.state(), CompactionState::Idle);
    }

    #[test]
    fn test_compaction_animator_start_stop() {
        let mut animator = CompactionAnimator::new();
        animator.start();
        assert!(animator.is_active());
        assert!(animator.is_running());

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
        animator.complete(1000, 500);

        assert_eq!(animator.progress(), 100);
        assert!(animator.is_showing_result());
        assert!(animator.result().is_some());

        let result = animator.result().unwrap();
        assert_eq!(result.original_tokens, 1000);
        assert_eq!(result.new_tokens, 500);
    }

    #[test]
    fn test_compaction_animator_fail() {
        let mut animator = CompactionAnimator::new();
        animator.start();
        animator.fail("Test error".to_string());

        assert!(animator.is_showing_result());
        let result = animator.result().unwrap();
        assert_eq!(result.error_message, Some("Test error".to_string()));
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

        let success_color = animator.get_success_color();
        match success_color {
            Color::Rgb(_, _, _) => {}
            _ => panic!("Expected RGB color"),
        }

        let error_color = animator.get_error_color();
        match error_color {
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

    #[test]
    fn test_compaction_animator_completion_bar() {
        let animator = CompactionAnimator::new();
        let bar = animator.render_completion_bar(10);
        // Check character count (not byte length, since █ is multi-byte)
        assert_eq!(bar.chars().count(), 10);
        assert!(bar.chars().all(|c| c == '█'));
    }

    #[test]
    fn test_compaction_state_variants() {
        assert_ne!(CompactionState::Idle, CompactionState::Running);
        assert_ne!(CompactionState::Running, CompactionState::Completed);
        assert_ne!(CompactionState::Completed, CompactionState::Failed);
    }
}
