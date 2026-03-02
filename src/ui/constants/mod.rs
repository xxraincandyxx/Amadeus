pub mod tips;
pub mod witty_phrases;

pub use tips::INFORMATIVE_TIPS;
pub use witty_phrases::WITTY_LOADING_PHRASES;

pub const PHRASE_CHANGE_INTERVAL_MS: u64 = 15000;
pub const COLOR_CYCLE_DURATION_MS: u64 = 4000;
pub const SPINNER_FRAME_INTERVAL_MS: u64 = 80;
