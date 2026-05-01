// @amadeus-header
// summary: Pluggable compaction trigger trait and built-in implementations.
// layer: core
// status: active
// feature_flags: none
// provides:
// - trait: crate::trigger::CompactionTrigger
// - type: crate::trigger::CompactionInput
// - type: crate::trigger::ThresholdCompactionTrigger
// - type: crate::trigger::CompositeCompactionTrigger
// uses: none
// invariants:
// - Trigger implementations must be deterministic for the same input.
// side_effects: none
// tests:
// - cmd: cargo test -p compaction
// @end-amadeus-header

//! Pluggable compaction triggers.
//!
//! The `CompactionTrigger` trait allows swapping how the agent decides
//! _when_ to compact. Built-in implementations cover the classic
//! threshold-based check and composite (multi-trigger) combinations.

use std::fmt;

/// Input signals available to a compaction trigger.
#[derive(Debug, Clone, Default)]
pub struct CompactionInput {
    pub message_count: usize,
    pub estimated_tokens: usize,
    pub turn_count: usize,
}

/// Decides whether compaction should fire for the current history state.
///
/// Implementations should be cheap (no I/O) — the caller is responsible
/// for token estimation and gathering metadata into `CompactionInput`.
pub trait CompactionTrigger: Send + Sync + fmt::Debug {
    /// Return `true` if compaction should run.
    fn should_compact(&self, input: &CompactionInput, context_window_size: u32) -> bool;

    /// Human-readable name for logging and observability.
    fn name(&self) -> &'static str;

    /// Current context usage as a percentage (0–100).
    fn context_usage_percent(&self, input: &CompactionInput, context_window_size: u32) -> u8 {
        if context_window_size == 0 {
            return 100;
        }
        let pct = (input.estimated_tokens as f64 / context_window_size as f64 * 100.0) as u8;
        pct.min(100)
    }
}

// ---------------------------------------------------------------------------
// Threshold trigger (the classic behaviour)
// ---------------------------------------------------------------------------

/// Fires when estimated tokens exceed a percentage of the context window.
///
/// This is the default trigger — it preserves the existing behaviour.
#[derive(Debug, Clone)]
pub struct ThresholdCompactionTrigger {
    pub threshold_percent: u8,
    pub min_messages: usize,
}

impl ThresholdCompactionTrigger {
    pub fn new(threshold_percent: u8, min_messages: usize) -> Self {
        Self {
            threshold_percent,
            min_messages,
        }
    }
}

impl Default for ThresholdCompactionTrigger {
    fn default() -> Self {
        Self {
            threshold_percent: 75,
            min_messages: 10,
        }
    }
}

impl CompactionTrigger for ThresholdCompactionTrigger {
    fn should_compact(&self, input: &CompactionInput, context_window_size: u32) -> bool {
        if input.message_count < self.min_messages {
            return false;
        }
        let threshold =
            (context_window_size as f64 * self.threshold_percent as f64 / 100.0) as usize;
        input.estimated_tokens > threshold
    }

    fn name(&self) -> &'static str {
        "threshold"
    }

    fn context_usage_percent(&self, input: &CompactionInput, context_window_size: u32) -> u8 {
        if context_window_size == 0 {
            return 100;
        }
        let pct = (input.estimated_tokens as f64 / context_window_size as f64 * 100.0) as u8;
        pct.min(100)
    }
}

// ---------------------------------------------------------------------------
// Composite trigger (combine multiple triggers)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum CompositeMode {
    /// Fire if **any** sub-trigger says yes.
    Any,
    /// Fire only if **all** sub-triggers say yes.
    All,
}

/// Combines multiple triggers with a logical combinator.
#[derive(Debug)]
pub struct CompositeCompactionTrigger {
    pub triggers: Vec<Box<dyn CompactionTrigger>>,
    pub mode: CompositeMode,
}

impl CompositeCompactionTrigger {
    pub fn new(triggers: Vec<Box<dyn CompactionTrigger>>, mode: CompositeMode) -> Self {
        Self { triggers, mode }
    }

    pub fn any(triggers: Vec<Box<dyn CompactionTrigger>>) -> Self {
        Self::new(triggers, CompositeMode::Any)
    }

    pub fn all(triggers: Vec<Box<dyn CompactionTrigger>>) -> Self {
        Self::new(triggers, CompositeMode::All)
    }
}

impl CompactionTrigger for CompositeCompactionTrigger {
    fn should_compact(&self, input: &CompactionInput, context_window_size: u32) -> bool {
        match self.mode {
            CompositeMode::Any => self
                .triggers
                .iter()
                .any(|t| t.should_compact(input, context_window_size)),
            CompositeMode::All => self
                .triggers
                .iter()
                .all(|t| t.should_compact(input, context_window_size)),
        }
    }

    fn name(&self) -> &'static str {
        "composite"
    }

    fn context_usage_percent(&self, input: &CompactionInput, context_window_size: u32) -> u8 {
        // Return the maximum across sub-triggers
        self.triggers
            .iter()
            .map(|t| t.context_usage_percent(input, context_window_size))
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_trigger_below_min_messages() {
        let trigger = ThresholdCompactionTrigger::new(75, 10);
        let input = CompactionInput {
            message_count: 5,
            estimated_tokens: 200_000,
            turn_count: 1,
        };
        assert!(!trigger.should_compact(&input, 200_000));
    }

    #[test]
    fn threshold_trigger_below_threshold() {
        let trigger = ThresholdCompactionTrigger::new(75, 5);
        let input = CompactionInput {
            message_count: 10,
            estimated_tokens: 50_000,
            turn_count: 3,
        };
        // 75% of 200_000 = 150_000 — 50k is below
        assert!(!trigger.should_compact(&input, 200_000));
    }

    #[test]
    fn threshold_trigger_above_threshold() {
        let trigger = ThresholdCompactionTrigger::new(75, 5);
        let input = CompactionInput {
            message_count: 20,
            estimated_tokens: 160_000,
            turn_count: 5,
        };
        // 75% of 200_000 = 150_000 — 160k exceeds
        assert!(trigger.should_compact(&input, 200_000));
    }

    #[test]
    fn composite_any_fires_when_one_does() {
        let always = ThresholdCompactionTrigger::new(0, 0); // fires at >0 tokens
        let never = ThresholdCompactionTrigger::new(100, 999); // never fires
        let composite = CompositeCompactionTrigger::any(vec![
            Box::new(never),
            Box::new(always),
        ]);
        let input = CompactionInput {
            message_count: 10,
            estimated_tokens: 100,
            turn_count: 1,
        };
        assert!(composite.should_compact(&input, 200_000));
    }

    #[test]
    fn composite_all_requires_both() {
        let always = ThresholdCompactionTrigger::new(0, 0);
        let never = ThresholdCompactionTrigger::new(100, 999);
        let composite = CompositeCompactionTrigger::all(vec![
            Box::new(never),
            Box::new(always),
        ]);
        let input = CompactionInput {
            message_count: 10,
            estimated_tokens: 100,
            turn_count: 1,
        };
        assert!(!composite.should_compact(&input, 200_000));
    }
}
