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
    streaming_state: StreamingState,
    start_time: Option<Instant>,
    active_tool_count: usize,
}

const SLIDING_BLOCK_FRAMES: &[&str] = &[
    "[=     ]", "[ =    ]", "[  =   ]", "[   =  ]", "[    = ]", "[     =]", "[    = ]", "[   =  ]",
    "[  =   ]", "[ =    ]",
];
const ANIMATION_SLOWDOWN_FACTOR: usize = 3;

impl LoadingIndicator {
    pub fn new() -> Self {
        Self {
            spinner: GeminiSpinner::new(),
            phrase_cycler: PhraseCycler::default(),
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
        }

        if state == StreamingState::WaitingForConfirmation {
            self.phrase_cycler.set_waiting_phrase(true);
        }

        self.streaming_state = state;
    }

    pub fn streaming_state(&self) -> StreamingState {
        self.streaming_state
    }

    pub fn is_active(&self) -> bool {
        self.streaming_state != StreamingState::Idle
    }

    pub fn tick(&mut self) {
        self.spinner.tick();

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
    }

    pub fn clear_activity_context(&mut self) {
        self.active_tool_count = 0;
    }

    fn sliding_block_frame(&self) -> &'static str {
        let idx =
            (self.spinner.frame_index() / ANIMATION_SLOWDOWN_FACTOR) % SLIDING_BLOCK_FRAMES.len();
        SLIDING_BLOCK_FRAMES[idx]
    }

    pub fn prompt_hint(&self) -> Option<String> {
        match self.streaming_state {
            StreamingState::Idle => None,
            StreamingState::WaitingForConfirmation => Some("awaiting approval".to_string()),
            StreamingState::Responding => {
                let status = if self.active_tool_count > 0 {
                    "working"
                } else {
                    "responding"
                };
                Some(format!("{status} {}", self.sliding_block_frame()))
            }
        }
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

    #[test]
    fn prompt_hint_uses_slower_sliding_animation() {
        let mut indicator = LoadingIndicator::new();
        indicator.set_streaming_state(StreamingState::Responding);
        indicator.set_activity_context(Some("bash".to_string()), None, None, 1);

        let first = indicator.prompt_hint();
        indicator.tick();
        let second = indicator.prompt_hint();
        indicator.tick();
        let third = indicator.prompt_hint();
        indicator.tick();
        let fourth = indicator.prompt_hint();

        assert_eq!(first, second);
        assert_eq!(second, third);
        assert_ne!(third, fourth);
    }
}
