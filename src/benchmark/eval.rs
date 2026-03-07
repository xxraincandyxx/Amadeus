use regex::Regex;
use serde::{Deserialize, Serialize};

use super::case::BenchmarkCase;
use super::metrics::BenchmarkMetrics;
use super::report::BenchmarkCaseRun;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkCheckResult {
    pub name: String,
    pub passed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BenchmarkEvaluation {
    pub passed: bool,
    pub score: usize,
    pub total_checks: usize,
    pub checks: Vec<BenchmarkCheckResult>,
}

impl BenchmarkEvaluation {
    pub fn from_run(
        case: &BenchmarkCase,
        run: &BenchmarkCaseRun,
        metrics: &BenchmarkMetrics,
    ) -> Self {
        let mut checks = Vec::new();
        let final_text = &run.final_text;

        for needle in &case.expectations.required_substrings {
            checks.push(BenchmarkCheckResult {
                name: format!("required_substring:{needle}"),
                passed: final_text.contains(needle),
                message: format!("Expected output to contain '{needle}'"),
            });
        }

        for needle in &case.expectations.forbidden_substrings {
            checks.push(BenchmarkCheckResult {
                name: format!("forbidden_substring:{needle}"),
                passed: !final_text.contains(needle),
                message: format!("Expected output to avoid '{needle}'"),
            });
        }

        for pattern in &case.expectations.required_regex {
            let regex = Regex::new(pattern).expect("validated regex");
            checks.push(BenchmarkCheckResult {
                name: format!("required_regex:{pattern}"),
                passed: regex.is_match(final_text),
                message: format!("Expected output to match regex '{pattern}'"),
            });
        }

        for pattern in &case.expectations.forbidden_regex {
            let regex = Regex::new(pattern).expect("validated regex");
            checks.push(BenchmarkCheckResult {
                name: format!("forbidden_regex:{pattern}"),
                passed: !regex.is_match(final_text),
                message: format!("Expected output to avoid regex '{pattern}'"),
            });
        }

        for tool in &case.expectations.required_tools {
            checks.push(BenchmarkCheckResult {
                name: format!("required_tool:{tool}"),
                passed: metrics.tools_used.iter().any(|used| used == tool),
                message: format!("Expected tool '{tool}' to be used"),
            });
        }

        for tool in &case.expectations.forbidden_tools {
            checks.push(BenchmarkCheckResult {
                name: format!("forbidden_tool:{tool}"),
                passed: !metrics.tools_used.iter().any(|used| used == tool),
                message: format!("Expected tool '{tool}' not to be used"),
            });
        }

        if let Some(expect_error) = case.expectations.expect_error {
            checks.push(BenchmarkCheckResult {
                name: "expect_error".to_string(),
                passed: (metrics.error_count > 0) == expect_error,
                message: format!("Expected error presence to be {expect_error}"),
            });
        }

        if let Some(expect_approval) = case.expectations.expect_approval {
            checks.push(BenchmarkCheckResult {
                name: "expect_approval".to_string(),
                passed: (metrics.approval_count > 0) == expect_approval,
                message: format!("Expected approval presence to be {expect_approval}"),
            });
        }

        if let Some(expect_compaction) = case.expectations.expect_compaction {
            checks.push(BenchmarkCheckResult {
                name: "expect_compaction".to_string(),
                passed: (metrics.compaction_count > 0) == expect_compaction,
                message: format!("Expected compaction presence to be {expect_compaction}"),
            });
        }

        if let Some(min_output_len) = case.expectations.min_output_len {
            checks.push(BenchmarkCheckResult {
                name: "min_output_len".to_string(),
                passed: final_text.len() >= min_output_len,
                message: format!("Expected output length >= {min_output_len}"),
            });
        }

        if let Some(max_output_len) = case.expectations.max_output_len {
            checks.push(BenchmarkCheckResult {
                name: "max_output_len".to_string(),
                passed: final_text.len() <= max_output_len,
                message: format!("Expected output length <= {max_output_len}"),
            });
        }

        if let Some(max_duration_ms) = case.thresholds.max_duration_ms {
            checks.push(BenchmarkCheckResult {
                name: "max_duration_ms".to_string(),
                passed: metrics.total_duration_ms <= max_duration_ms,
                message: format!("Expected duration <= {max_duration_ms}ms"),
            });
        }

        if let Some(max_tool_calls) = case.thresholds.max_tool_calls {
            checks.push(BenchmarkCheckResult {
                name: "max_tool_calls".to_string(),
                passed: metrics.tool_call_count <= max_tool_calls,
                message: format!("Expected tool calls <= {max_tool_calls}"),
            });
        }

        if let Some(max_errors) = case.thresholds.max_errors {
            checks.push(BenchmarkCheckResult {
                name: "max_errors".to_string(),
                passed: metrics.error_count <= max_errors,
                message: format!("Expected errors <= {max_errors}"),
            });
        }

        let total_checks = checks.len();
        let score = checks.iter().filter(|check| check.passed).count();
        let passed = checks.iter().all(|check| check.passed);

        Self {
            passed,
            score,
            total_checks,
            checks,
        }
    }
}
