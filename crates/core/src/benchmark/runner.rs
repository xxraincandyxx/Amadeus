// @amadeus-header
// summary: Benchmark pipeline code for runner.
// layer: benchmark
// status: active
// feature_flags: none
// provides:
// - module: crate::benchmark::runner
// - type: crate::benchmark::runner::BenchmarkRunnerOptions
// - type: crate::benchmark::runner::BenchmarkRunner
// uses:
// - module: crate::agent::config
// - module: crate::agent::events::AgentEvent
// - module: crate::agent::loop_agent::Agent
// - module: crate::agent::messages::Message
// - module: crate::benchmark::case
// - module: crate::benchmark::eval::BenchmarkEvaluation
// - module: crate::benchmark::metrics::BenchmarkMetrics
// - module: crate::benchmark::mock::BenchmarkMockClient
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Reads or writes filesystem state.
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use futures::StreamExt;

use crate::agent::config::{Config, Provider};
use crate::agent::events::AgentEvent;
use crate::agent::loop_agent::Agent;
use crate::agent::messages::Message;
use crate::benchmark::case::{BenchmarkCase, BenchmarkMode};
use crate::benchmark::eval::BenchmarkEvaluation;
use crate::benchmark::metrics::BenchmarkMetrics;
use crate::benchmark::mock::BenchmarkMockClient;
use crate::benchmark::report::{BenchmarkCaseRun, BenchmarkRunSummary, CapturedEvent, RunStatus};
use crate::client::{AnthropicClient, LLMClient, OpenAIClient};
use crate::error::{AgentError, Result};
use crate::policy::{ApprovalMode, Policy};

#[derive(Debug, Clone)]
pub struct BenchmarkRunnerOptions {
    pub fixtures_dir: PathBuf,
    pub output_dir: PathBuf,
    pub suite_filter: Option<String>,
    pub case_filter: Option<String>,
    pub mode_override: Option<BenchmarkMode>,
}

impl Default for BenchmarkRunnerOptions {
    fn default() -> Self {
        Self {
            fixtures_dir: PathBuf::from("tests/fixtures/benchmarks"),
            output_dir: PathBuf::from("benchmark_runs"),
            suite_filter: None,
            case_filter: None,
            mode_override: None,
        }
    }
}

pub struct BenchmarkRunner {
    base_config: Arc<Config>,
    options: BenchmarkRunnerOptions,
}

impl BenchmarkRunner {
    pub fn new(base_config: Arc<Config>, options: BenchmarkRunnerOptions) -> Self {
        Self {
            base_config,
            options,
        }
    }

    pub fn load_cases(&self) -> Result<Vec<BenchmarkCase>> {
        let mut cases = BenchmarkCase::load_dir(&self.options.fixtures_dir)?;

        if let Some(suite_filter) = &self.options.suite_filter {
            cases.retain(|case| case.suite == *suite_filter);
        }

        if let Some(case_filter) = &self.options.case_filter {
            cases.retain(|case| case.id == *case_filter);
        }

        Ok(cases)
    }

    pub async fn run_all(&self) -> Result<BenchmarkRunSummary> {
        let cases = self.load_cases()?;
        if cases.is_empty() {
            return Err(AgentError::Config(
                "No benchmark cases matched the current filters".to_string(),
            ));
        }

        let run_id = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let started_at = Utc::now().to_rfc3339();
        let mut results = Vec::new();

        for case in cases {
            results.push(self.run_case(&case).await?);
        }

        let finished_at = Utc::now().to_rfc3339();
        let passed_cases = results
            .iter()
            .filter(|result| result.status == RunStatus::Passed)
            .count();
        let failed_cases = results
            .iter()
            .filter(|result| result.status == RunStatus::Failed)
            .count();
        let error_cases = results
            .iter()
            .filter(|result| result.status == RunStatus::Error)
            .count();

        let summary = BenchmarkRunSummary {
            run_id,
            started_at,
            finished_at,
            total_cases: results.len(),
            passed_cases,
            failed_cases,
            error_cases,
            results,
        };

        self.persist_summary(&summary)?;
        Ok(summary)
    }

    async fn run_case(&self, case: &BenchmarkCase) -> Result<BenchmarkCaseRun> {
        let mode = self
            .options
            .mode_override
            .clone()
            .unwrap_or_else(|| case.mode.clone());

        match mode {
            BenchmarkMode::Mock => {
                let script = case.mock_script.clone().ok_or_else(|| {
                    AgentError::Config(format!(
                        "Benchmark case '{}' is missing mock_script",
                        case.id
                    ))
                })?;
                let client = BenchmarkMockClient::new(script);
                let config = Arc::new(self.case_config(case)?);
                self.run_case_with_client(case, mode, config, client).await
            }
            BenchmarkMode::Live => {
                let config = Arc::new(self.case_config(case)?);
                match config.provider {
                    Provider::Anthropic => {
                        let client = AnthropicClient::new(
                            config.api_key.clone(),
                            config.base_url.clone(),
                            config.model.clone(),
                        );
                        self.run_case_with_client(case, mode, config, client).await
                    }
                    Provider::OpenAI => {
                        let client = OpenAIClient::new(
                            config.api_key.clone(),
                            config.base_url.clone(),
                            config.model.clone(),
                        );
                        self.run_case_with_client(case, mode, config, client).await
                    }
                }
            }
        }
    }

    async fn run_case_with_client<C: LLMClient + Clone + 'static>(
        &self,
        case: &BenchmarkCase,
        mode: BenchmarkMode,
        config: Arc<Config>,
        client: C,
    ) -> Result<BenchmarkCaseRun> {
        let started_at = Utc::now().to_rfc3339();
        let policy = self.case_policy(case)?;
        let agent = Agent::builder(client, config)
            .with_default_tools()
            .with_policy(policy)
            .build();

        {
            let history = agent.history();
            let mut history_guard = history.write().await;
            history_guard.push(Message::user(&case.prompt));
        }

        let start = Instant::now();
        let mut stream = agent.run_stream();
        let mut events = Vec::new();
        let mut final_text = String::new();
        let mut thinking_text = String::new();
        let mut final_result = None;
        let mut terminal_error = None;
        let mut session_log_path = None;

        while let Some(item) = stream.next().await {
            let elapsed_ms = start.elapsed().as_millis() as u64;
            match item {
                Ok(event) => {
                    let index = events.len();

                    match &event {
                        AgentEvent::TextDelta { delta } => final_text.push_str(delta),
                        AgentEvent::ThinkingDelta { delta } => thinking_text.push_str(delta),
                        AgentEvent::Done { result } => {
                            final_result = Some(result.clone());
                            if final_text.is_empty() {
                                final_text = result.text.clone();
                            }
                        }
                        AgentEvent::Error { message } => {
                            terminal_error = Some(message.clone());
                        }
                        AgentEvent::SessionSaved { path } => {
                            session_log_path = Some(path.clone());
                        }
                        _ => {}
                    }

                    let is_terminal =
                        matches!(event, AgentEvent::Done { .. } | AgentEvent::Error { .. });
                    events.push(CapturedEvent {
                        index,
                        elapsed_ms,
                        event,
                    });

                    if is_terminal {
                        break;
                    }
                }
                Err(error) => {
                    let message = error.to_string();
                    terminal_error = Some(message.clone());
                    let index = events.len();
                    events.push(CapturedEvent {
                        index,
                        elapsed_ms,
                        event: AgentEvent::Error { message },
                    });
                    break;
                }
            }
        }

        let finished_at = Utc::now().to_rfc3339();
        let metrics = BenchmarkMetrics::from_events(&events, &final_text);
        let mut run = BenchmarkCaseRun {
            id: case.id.clone(),
            suite: case.suite.clone(),
            description: case.description.clone(),
            mode,
            prompt: case.prompt.clone(),
            status: RunStatus::Passed,
            started_at,
            finished_at,
            final_text,
            thinking_text,
            session_log_path,
            terminal_error,
            final_result,
            events,
            metrics,
            evaluation: BenchmarkEvaluation::default(),
        };

        let evaluation = BenchmarkEvaluation::from_run(case, &run, &run.metrics);
        run.status = if evaluation.passed {
            RunStatus::Passed
        } else if run.terminal_error.is_some() {
            RunStatus::Error
        } else {
            RunStatus::Failed
        };
        run.evaluation = evaluation;
        Ok(run)
    }

    fn case_config(&self, case: &BenchmarkCase) -> Result<Config> {
        let mut config = (*self.base_config).clone();

        if let Some(timeout_seconds) = case.config.timeout_seconds {
            config.timeout_seconds = timeout_seconds;
        }

        if let Some(session_log_dir) = &case.config.session_log_dir {
            config.session_log_dir = Some(PathBuf::from(session_log_dir));
        }

        if case.mode == BenchmarkMode::Live && config.api_key.is_empty() {
            return Err(AgentError::MissingEnvVar(match config.provider {
                Provider::Anthropic => "ANTHROPIC_API_KEY".to_string(),
                Provider::OpenAI => "OPENAI_API_KEY".to_string(),
            }));
        }

        Ok(config)
    }

    fn case_policy(&self, case: &BenchmarkCase) -> Result<Policy> {
        let mode = case
            .config
            .approval_mode
            .clone()
            .unwrap_or_else(|| "ask".to_string());

        let approval_mode = match mode.as_str() {
            "auto" => ApprovalMode::Auto,
            "ask" => ApprovalMode::Ask,
            "strict" => ApprovalMode::Strict,
            other => {
                return Err(AgentError::Config(format!(
                    "Unsupported approval mode '{other}' in case '{}'",
                    case.id
                )))
            }
        };

        let mut value = serde_json::to_value(Policy::default()).map_err(AgentError::Serde)?;
        value["mode"] = serde_json::Value::String(mode);
        let mut policy = Policy::from_json(&value).map_err(AgentError::Serde)?;
        policy.set_mode(approval_mode);
        Ok(policy)
    }

    fn persist_summary(&self, summary: &BenchmarkRunSummary) -> Result<()> {
        let run_dir = self.options.output_dir.join(&summary.run_id);
        fs::create_dir_all(run_dir.join("cases"))?;

        self.write_json(run_dir.join("summary.json"), summary)?;

        let mut jsonl = String::new();
        for result in &summary.results {
            let case_dir = run_dir.join("cases").join(safe_path_component(&result.id));
            fs::create_dir_all(&case_dir)?;
            self.write_json(case_dir.join("trace.json"), &result.events)?;
            self.write_json(case_dir.join("metrics.json"), &result.metrics)?;
            self.write_json(case_dir.join("evaluation.json"), &result.evaluation)?;
            self.write_json(case_dir.join("result.json"), result)?;
            fs::write(case_dir.join("transcript.txt"), &result.final_text)?;

            jsonl.push_str(&serde_json::to_string(result).map_err(AgentError::Serde)?);
            jsonl.push('\n');
        }

        fs::write(run_dir.join("results.jsonl"), jsonl)?;
        Ok(())
    }

    fn write_json<T: serde::Serialize, P: AsRef<Path>>(&self, path: P, value: &T) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(value).map_err(AgentError::Serde)?;
        fs::write(path, bytes)?;
        Ok(())
    }
}

fn safe_path_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
