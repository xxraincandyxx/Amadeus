// @amadeus-header
// summary: Module root for the benchmark subsystem and its exports.
// layer: benchmark
// status: active
// feature_flags: none
// provides:
// - module: crate::benchmark
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - tests/mod.rs
// @end-amadeus-header

pub mod case;
pub mod eval;
pub mod metrics;
pub mod mock;
pub mod report;
pub mod runner;

pub use case::{
    BenchmarkCase, BenchmarkCaseConfig, BenchmarkExpectations, BenchmarkMode, BenchmarkThresholds,
};
pub use eval::{BenchmarkCheckResult, BenchmarkEvaluation};
pub use metrics::BenchmarkMetrics;
pub use report::{BenchmarkCaseRun, BenchmarkRunSummary, CapturedEvent, RunStatus};
pub use runner::{BenchmarkRunner, BenchmarkRunnerOptions};
