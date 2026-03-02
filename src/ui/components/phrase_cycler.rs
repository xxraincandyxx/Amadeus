use std::time::{Duration, Instant};

use crate::ui::constants::{INFORMATIVE_TIPS, PHRASE_CHANGE_INTERVAL_MS, WITTY_LOADING_PHRASES};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhraseMode {
    Tips,
    Witty,
    All,
    Off,
}

impl Default for PhraseMode {
    fn default() -> Self {
        Self::Tips
    }
}

pub struct PhraseCycler {
    mode: PhraseMode,
    current_phrase: Option<String>,
    last_change: Instant,
    custom_phrases: Vec<String>,
    has_shown_first_tip: bool,
}

impl PhraseCycler {
    pub fn new(mode: PhraseMode) -> Self {
        Self {
            mode,
            current_phrase: None,
            last_change: Instant::now(),
            custom_phrases: Vec::new(),
            has_shown_first_tip: false,
        }
    }

    pub fn with_custom_phrases(mut self, phrases: Vec<String>) -> Self {
        self.custom_phrases = phrases;
        self
    }

    pub fn set_mode(&mut self, mode: PhraseMode) {
        self.mode = mode;
    }

    pub fn mode(&self) -> PhraseMode {
        self.mode
    }

    pub fn tick(&mut self, is_active: bool) {
        if self.mode == PhraseMode::Off || !is_active {
            self.current_phrase = None;
            return;
        }

        let should_change = self.current_phrase.is_none()
            || self.last_change.elapsed() >= Duration::from_millis(PHRASE_CHANGE_INTERVAL_MS);

        if should_change {
            self.select_random_phrase();
            self.last_change = Instant::now();
        }
    }

    fn select_random_phrase(&mut self) {
        use rand::seq::SliceRandom;

        let phrase_list: Vec<&str> = match self.mode {
            PhraseMode::Tips => INFORMATIVE_TIPS.to_vec(),
            PhraseMode::Witty => {
                if !self.custom_phrases.is_empty() {
                    self.custom_phrases.iter().map(|s| s.as_str()).collect()
                } else {
                    WITTY_LOADING_PHRASES.to_vec()
                }
            }
            PhraseMode::All => {
                if !self.has_shown_first_tip {
                    self.has_shown_first_tip = true;
                    INFORMATIVE_TIPS.to_vec()
                } else {
                    let show_tip = rand::random::<f64>() < (1.0 / 6.0);
                    if show_tip {
                        INFORMATIVE_TIPS.to_vec()
                    } else if !self.custom_phrases.is_empty() {
                        self.custom_phrases.iter().map(|s| s.as_str()).collect()
                    } else {
                        WITTY_LOADING_PHRASES.to_vec()
                    }
                }
            }
            PhraseMode::Off => return,
        };

        let mut rng = rand::thread_rng();
        self.current_phrase = phrase_list.choose(&mut rng).map(|s| s.to_string());
    }

    pub fn get_phrase(&self) -> Option<&str> {
        self.current_phrase.as_deref()
    }

    pub fn set_waiting_phrase(&mut self, waiting: bool) {
        if waiting {
            self.current_phrase = Some("Waiting for user confirmation...".to_string());
        }
    }

    pub fn reset(&mut self) {
        self.current_phrase = None;
        self.last_change = Instant::now();
    }
}

impl Default for PhraseCycler {
    fn default() -> Self {
        Self::new(PhraseMode::default())
    }
}
