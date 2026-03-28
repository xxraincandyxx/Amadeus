use std::time::Instant;

use super::phrase_cycler::PhraseCycler;
use super::spinner::GeminiSpinner;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreamingState {
    Idle,
    Responding,
    WaitingForConfirmation,
}

pub struct LoadingIndicator {
    spinner: GeminiSpinner,
    phrase_cycler: PhraseCycler,
    scramble: ScrambleTextAnimator,
    streaming_state: StreamingState,
    start_time: Option<Instant>,
    active_tool_count: usize,
}

const SCRAMBLE_CHARS: &str = "!<>-_\\/[]{}=+*^?#________";
const DOT_FRAMES: [&str; 3] = [".", "..", "..."];
const SCRAMBLE_TICK_SLOWDOWN: usize = 6;
/// Spinner frame wraps every 10 ticks; keep this ≤10 so dot phases advance each cycle.
const DOT_TICK_SLOWDOWN: usize = 4;

#[derive(Debug, Clone)]
struct ScrambleCell {
    from: char,
    to: char,
    start: usize,
    end: usize,
}

#[derive(Debug, Default)]
struct ScrambleTextAnimator {
    target: String,
    previous_text: String,
    frame: usize,
    tick_accumulator: usize,
    cells: Vec<ScrambleCell>,
}

impl ScrambleTextAnimator {
    fn set_target(&mut self, new_target: &str) {
        if self.target == new_target && !self.cells.is_empty() {
            return;
        }

        let old_text = if self.target.is_empty() {
            self.previous_text.clone()
        } else {
            self.render_plain_text()
        };
        let old_chars: Vec<char> = old_text.chars().collect();
        let new_chars: Vec<char> = new_target.chars().collect();
        let length = old_chars.len().max(new_chars.len());

        self.target = new_target.to_string();
        self.frame = 0;
        self.tick_accumulator = 0;
        self.cells = (0..length)
            .map(|index| {
                let start = (index * 3) % 12;
                let end = start + 8 + (index % 7);
                ScrambleCell {
                    from: old_chars.get(index).copied().unwrap_or(' '),
                    to: new_chars.get(index).copied().unwrap_or(' '),
                    start,
                    end,
                }
            })
            .collect();
    }

    fn clear(&mut self) {
        self.previous_text.clear();
        self.target.clear();
        self.frame = 0;
        self.tick_accumulator = 0;
        self.cells.clear();
    }

    fn tick(&mut self) {
        if !self.cells.is_empty() {
            self.tick_accumulator += 1;
            if self.tick_accumulator < SCRAMBLE_TICK_SLOWDOWN {
                return;
            }

            self.tick_accumulator = 0;
            self.frame += 1;
            if self.frame > self.max_end_frame() {
                self.previous_text = self.target.clone();
            }
        }
    }

    fn render(&self) -> String {
        if self.cells.is_empty() {
            return self.target.clone();
        }

        let scramble_chars: Vec<char> = SCRAMBLE_CHARS.chars().collect();
        self.cells
            .iter()
            .enumerate()
            .map(|(index, cell)| {
                if self.frame >= cell.end {
                    cell.to
                } else if self.frame >= cell.start {
                    let scramble_index =
                        (self.frame + index.saturating_mul(7)) % scramble_chars.len().max(1);
                    scramble_chars
                        .get(scramble_index)
                        .copied()
                        .unwrap_or(cell.to)
                } else {
                    cell.from
                }
            })
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    fn render_plain_text(&self) -> String {
        self.cells
            .iter()
            .map(|cell| cell.to)
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    fn max_end_frame(&self) -> usize {
        self.cells.iter().map(|cell| cell.end).max().unwrap_or(0)
    }
}

impl LoadingIndicator {
    pub fn new() -> Self {
        Self {
            spinner: GeminiSpinner::new(),
            phrase_cycler: PhraseCycler::default(),
            scramble: ScrambleTextAnimator::default(),
            streaming_state: StreamingState::Idle,
            start_time: None,
            active_tool_count: 0,
        }
    }

    pub fn set_streaming_state(&mut self, state: StreamingState) {
        let was_idle = self.streaming_state == StreamingState::Idle;
        let now_idle = state == StreamingState::Idle;

        if state == StreamingState::Responding && was_idle {
            self.start_time = Some(Instant::now());
            self.spinner.start();
        } else if state == StreamingState::Idle && !now_idle {
            self.start_time = None;
            self.spinner.stop();
            self.phrase_cycler.reset();
            self.clear_activity_context();
            self.scramble.clear();
        }

        if state == StreamingState::WaitingForConfirmation {
            self.phrase_cycler.set_waiting_phrase(true);
        }

        self.streaming_state = state;
        self.sync_scramble_target();
    }

    pub fn streaming_state(&self) -> StreamingState {
        self.streaming_state
    }

    pub fn is_active(&self) -> bool {
        self.streaming_state != StreamingState::Idle
    }

    pub fn tick(&mut self) {
        self.spinner.tick();
        self.scramble.tick();

        let is_responding = self.streaming_state == StreamingState::Responding;
        self.phrase_cycler.tick(is_responding);
    }

    pub fn set_tool_activity_phrase(&mut self, tool_name: &str) {
        self.phrase_cycler.set_tool_activity_phrase(tool_name);
    }

    pub fn set_activity_context(
        &mut self,
        _label: Option<String>,
        _progress_message: Option<String>,
        _progress: Option<u8>,
        count: usize,
    ) {
        self.active_tool_count = count;
        self.sync_scramble_target();
    }

    pub fn clear_activity_context(&mut self) {
        self.active_tool_count = 0;
        self.sync_scramble_target();
    }

    fn dot_frame(&self) -> &'static str {
        DOT_FRAMES[(self.spinner.frame_index() / DOT_TICK_SLOWDOWN) % DOT_FRAMES.len()]
    }

    fn current_status_label(&self) -> Option<&'static str> {
        match self.streaming_state {
            StreamingState::Idle => None,
            StreamingState::WaitingForConfirmation => None,
            StreamingState::Responding => Some(if self.active_tool_count > 0 {
                "working"
            } else {
                "responding"
            }),
        }
    }

    fn sync_scramble_target(&mut self) {
        match self.streaming_state {
            StreamingState::Idle => {
                self.scramble.clear();
            }
            StreamingState::WaitingForConfirmation => {
                self.scramble.set_target("awaiting approval");
            }
            StreamingState::Responding => {
                if let Some(label) = self.current_status_label() {
                    self.scramble.set_target(label);
                }
            }
        }
    }

    /// Scramble-only text for the **input** status row (above the composer). No dot suffix.
    pub fn input_chrome_hint(&self) -> Option<String> {
        match self.streaming_state {
            StreamingState::Idle => None,
            StreamingState::WaitingForConfirmation | StreamingState::Responding => {
                let s = self.scramble.render();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            }
        }
    }

    /// Live viewport / monitor: **plain** status label + `.` / `..` / `...` only (no scramble).
    pub fn viewport_loading_line(&self) -> Option<String> {
        match self.streaming_state {
            StreamingState::Idle => None,
            StreamingState::WaitingForConfirmation => {
                Some(format!("awaiting approval {}", self.dot_frame()))
            }
            StreamingState::Responding => {
                let label = self.current_status_label().unwrap_or("responding");
                Some(format!("{label} {}", self.dot_frame()))
            }
        }
    }

    /// Animated `.` / `..` / `...` suffix (for fallbacks when state is idle but UI still wants dots).
    pub fn loading_dot_suffix(&self) -> &'static str {
        self.dot_frame()
    }

    pub fn get_elapsed_secs(&self) -> u64 {
        self.start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0)
    }
}

impl Default for LoadingIndicator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Last whitespace-delimited token (the dot suffix in `responding ..`).
    fn dot_suffix(line: &str) -> &str {
        line.rsplit_once(' ').map(|(_, d)| d).unwrap_or(line)
    }

    #[test]
    fn viewport_loading_line_is_plain_label_plus_dots_only() {
        let mut indicator = LoadingIndicator::new();
        indicator.set_streaming_state(StreamingState::Responding);
        indicator.set_activity_context(Some("bash".to_string()), None, None, 1);

        let first = indicator.viewport_loading_line().expect("line");
        assert!(first.starts_with("working "));
        assert!([".", "..", "..."].contains(&dot_suffix(&first)));

        for _ in 0..(DOT_TICK_SLOWDOWN - 1) {
            indicator.tick();
        }
        assert_eq!(
            dot_suffix(&first),
            dot_suffix(&indicator.viewport_loading_line().expect("line"))
        );
        indicator.tick();
        let two_dots = indicator.viewport_loading_line().expect("line");
        assert_eq!(dot_suffix(&two_dots), "..");

        for _ in 0..(DOT_TICK_SLOWDOWN - 1) {
            indicator.tick();
        }
        assert_eq!(
            dot_suffix(&two_dots),
            dot_suffix(&indicator.viewport_loading_line().expect("line"))
        );
        indicator.tick();
        let three_dots = indicator.viewport_loading_line().expect("line");
        assert_eq!(dot_suffix(&three_dots), "...");

        assert!(!first.contains('['));
    }

    #[test]
    fn input_chrome_hint_scrambles_to_working_without_dot_suffix() {
        let mut indicator = LoadingIndicator::new();
        indicator.set_streaming_state(StreamingState::Responding);
        indicator.set_activity_context(None, None, None, 1);

        for _ in 0..800 {
            indicator.tick();
        }

        let hint = indicator.input_chrome_hint().expect("input chrome");
        assert_eq!(hint, "working");
        assert!(!hint.ends_with('.'));
    }

    #[test]
    fn viewport_relabels_when_tool_activity_changes() {
        let mut indicator = LoadingIndicator::new();
        indicator.set_streaming_state(StreamingState::Responding);

        for _ in 0..800 {
            indicator.tick();
        }
        let before = indicator.viewport_loading_line().expect("line");
        assert!(before.starts_with("responding "));

        indicator.set_activity_context(None, None, None, 1);
        for _ in 0..800 {
            indicator.tick();
        }

        let after = indicator.viewport_loading_line().expect("line");
        assert!(after.starts_with("working "));
    }

    #[test]
    fn waiting_confirmation_input_scrambles_viewport_adds_dots() {
        let mut indicator = LoadingIndicator::new();
        indicator.set_streaming_state(StreamingState::WaitingForConfirmation);

        let v = indicator.viewport_loading_line().expect("viewport");
        assert!(v.starts_with("awaiting approval "));
        assert!([".", "..", "..."].contains(&dot_suffix(&v)));

        for _ in 0..800 {
            indicator.tick();
        }
        assert_eq!(
            indicator.input_chrome_hint().as_deref(),
            Some("awaiting approval")
        );
    }

    #[test]
    fn scramble_animator_advances_frames_more_slowly() {
        let mut animator = ScrambleTextAnimator::default();
        animator.set_target("working");

        for _ in 0..5 {
            animator.tick();
        }
        assert_eq!(animator.frame, 0);

        animator.tick();
        assert_eq!(animator.frame, 1);
    }

    #[test]
    fn dot_animation_advances_more_slowly_on_viewport_line() {
        let mut indicator = LoadingIndicator::new();
        indicator.set_streaming_state(StreamingState::Responding);

        let first = indicator.viewport_loading_line().expect("line");
        for _ in 0..(DOT_TICK_SLOWDOWN - 1) {
            indicator.tick();
        }
        assert_eq!(
            dot_suffix(&first),
            dot_suffix(&indicator.viewport_loading_line().expect("line"))
        );
        indicator.tick();
        let with_two_dots = indicator.viewport_loading_line().expect("line");
        assert_eq!(dot_suffix(&first), ".");
        assert_eq!(dot_suffix(&with_two_dots), "..");
    }
}
