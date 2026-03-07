use serde::{Deserialize, Serialize};

use crate::agent::events::{AgentEvent, RunResult};

use super::case::BenchmarkMode;
use super::eval::BenchmarkEvaluation;
use super::metrics::BenchmarkMetrics;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedEvent {
    pub index: usize,
    pub elapsed_ms: u64,
    pub event: AgentEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Passed,
    Failed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkCaseRun {
    pub id: String,
    pub suite: String,
    pub description: String,
    pub mode: BenchmarkMode,
    pub prompt: String,
    pub status: RunStatus,
    pub started_at: String,
    pub finished_at: String,
    pub final_text: String,
    pub thinking_text: String,
    pub session_log_path: Option<String>,
    pub terminal_error: Option<String>,
    pub final_result: Option<RunResult>,
    pub events: Vec<CapturedEvent>,
    pub metrics: BenchmarkMetrics,
    pub evaluation: BenchmarkEvaluation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRunSummary {
    pub run_id: String,
    pub started_at: String,
    pub finished_at: String,
    pub total_cases: usize,
    pub passed_cases: usize,
    pub failed_cases: usize,
    pub error_cases: usize,
    pub results: Vec<BenchmarkCaseRun>,
}
