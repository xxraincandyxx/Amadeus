// @amadeus-header
// summary: Read-only assessment runner that drives agent-based feature audits and report generation.
// layer: core
// status: active
// feature_flags:
// - test-utils
// provides:
// - module: crate::assessment
// - type: crate::assessment::AssessmentConfig
// - type: crate::assessment::AssessmentResult
// - type: crate::assessment::AssessmentRunner
// uses:
// - module: crate::agent::config::Config
// - module: crate::agent::loop_agent
// - module: crate::client::LLMClient
// - module: crate::hooks
// - module: crate::permissions
// - module: crate::skills::registry
// - module: crate::error
// invariants:
// - Assessment runs stay in read-only permission mode and persist reports host-side.
// side_effects:
// - Reads or writes filesystem state.
// - Spawns asynchronous tasks.
// tests:
// - cmd: cargo test -p core assessment_runner_writes_report --features full
// @end-amadeus-header

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use futures::stream;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;

use crate::agent::config::Config;
use crate::agent::loop_agent::Agent;
use crate::agent::messages::{ContentBlock, Message};
use crate::client::{LLMClient, StreamEvent};
use crate::error::{AgentError, Result};
use crate::hooks::{Hook, HookAction, HookRegistry};
use crate::permissions::PermissionMode;
use crate::skills::registry::SkillRegistry;

const DEFAULT_SKILL_NAME: &str = "feature-assessment-loop";

#[derive(Debug, Clone)]
pub struct AssessmentConfig {
    pub report_dir: PathBuf,
    pub skill_name: String,
    pub prompt: String,
}

impl AssessmentConfig {
    pub fn new(report_dir: PathBuf, prompt: impl Into<String>) -> Self {
        Self {
            report_dir,
            skill_name: DEFAULT_SKILL_NAME.to_string(),
            prompt: prompt.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssessmentResult {
    pub report_path: PathBuf,
    pub skill_name: String,
    pub permission_mode: String,
    pub tool_log: Vec<ToolLogEntry>,
    pub report_markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolLogEntry {
    pub phase: String,
    pub tool_name: String,
    pub duration_ms: Option<u64>,
    pub is_error: Option<bool>,
}

#[derive(Clone, Default)]
struct ReportHook {
    entries: Arc<Mutex<Vec<ToolLogEntry>>>,
}

impl ReportHook {
    fn new(entries: Arc<Mutex<Vec<ToolLogEntry>>>) -> Self {
        Self { entries }
    }

    async fn snapshot(&self) -> Vec<ToolLogEntry> {
        self.entries.lock().await.clone()
    }
}

#[async_trait]
impl Hook for ReportHook {
    fn name(&self) -> &str {
        "assessment-report"
    }

    async fn on_tool_start(&self, name: &str, _input: &serde_json::Value) -> Result<HookAction> {
        self.entries.lock().await.push(ToolLogEntry {
            phase: "start".to_string(),
            tool_name: name.to_string(),
            duration_ms: None,
            is_error: None,
        });
        Ok(HookAction::Continue)
    }

    async fn on_tool_complete(
        &self,
        name: &str,
        _input: &serde_json::Value,
        _output: &str,
        is_error: bool,
        duration_ms: u64,
    ) -> Result<()> {
        self.entries.lock().await.push(ToolLogEntry {
            phase: "complete".to_string(),
            tool_name: name.to_string(),
            duration_ms: Some(duration_ms),
            is_error: Some(is_error),
        });
        Ok(())
    }
}

pub struct AssessmentRunner<C: LLMClient> {
    client: C,
    config: Arc<Config>,
}

impl<C: LLMClient + Clone + 'static> AssessmentRunner<C> {
    pub fn new(client: C, config: Arc<Config>) -> Self {
        Self { client, config }
    }

    pub async fn run(&self, assessment: AssessmentConfig) -> Result<AssessmentResult> {
        let registry = SkillRegistry::load_for_config(self.config.as_ref())
            .map_err(|error| AgentError::Config(error.to_string()))?;
        let skill = registry
            .get(&assessment.skill_name)
            .cloned()
            .ok_or_else(|| {
                AgentError::Config(format!("Skill '{}' not found", assessment.skill_name))
            })?;

        let report_hook = ReportHook::new(Arc::new(Mutex::new(Vec::new())));
        let mut assessment_config = (*self.config).clone();
        assessment_config.permission_mode = PermissionMode::ReadOnly;
        let assessment_config = Arc::new(assessment_config);

        let mut hooks =
            HookRegistry::load_for_config(assessment_config.as_ref()).unwrap_or_default();
        hooks.register_arc(Arc::new(report_hook.clone()));

        let agent = Agent::builder(self.client.clone(), assessment_config.clone())
            .with_default_tools()
            .with_hooks(hooks)
            .build();
        let result = agent.run_with_skill(&skill, &assessment.prompt).await?;
        let tool_log = report_hook.snapshot().await;
        let report_markdown = render_report(
            &assessment.skill_name,
            assessment_config.permission_mode,
            &result.text,
            &tool_log,
        );

        std::fs::create_dir_all(&assessment.report_dir)?;
        let report_path = assessment.report_dir.join(format!(
            "feature_assessment_{}.md",
            Utc::now().format("%Y%m%d_%H%M%S")
        ));
        std::fs::write(&report_path, &report_markdown)?;

        Ok(AssessmentResult {
            report_path,
            skill_name: assessment.skill_name,
            permission_mode: assessment_config.permission_mode.as_str().to_string(),
            tool_log,
            report_markdown,
        })
    }
}

fn render_report(
    skill_name: &str,
    permission_mode: PermissionMode,
    summary: &str,
    tool_log: &[ToolLogEntry],
) -> String {
    let mut report = String::new();
    report.push_str("# Feature Assessment Report\n\n");
    report.push_str(&format!("Generated: {}\n\n", Utc::now().to_rfc3339()));
    report.push_str(&format!("Skill: `{skill_name}`\n\n"));
    report.push_str(&format!(
        "Permission mode: `{}`\n\n",
        permission_mode.as_str()
    ));
    report.push_str("## Assessment\n\n");
    report.push_str(summary.trim());
    report.push_str("\n\n## Tool Activity\n\n");
    if tool_log.is_empty() {
        report.push_str("- no tool activity recorded\n");
    } else {
        for entry in tool_log {
            report.push_str(&format!(
                "- {} `{}` duration_ms={:?} is_error={:?}\n",
                entry.phase, entry.tool_name, entry.duration_ms, entry.is_error
            ));
        }
    }
    report
}

pub fn default_prompt(workdir: &Path) -> String {
    format!(
        "Assess the workspace at {}. Base coverage on docs/TMUX_TEST_FLOW.md, docs/TUI_GUIDE.md, relevant tests, and runtime flows. Stay in read-only mode, use tmux-cli and targeted test commands when useful, split work with subagents when helpful, and report only confirmed bugs with reproduction details. If no confirmed bug remains, say so explicitly.",
        workdir.display()
    )
}

#[derive(Clone)]
pub struct ScriptedAssessmentClient {
    state: Arc<Mutex<ScriptedAssessmentState>>,
}

#[derive(Debug, Clone)]
struct ScriptedAssessmentState {
    workdir: PathBuf,
    step: usize,
    pane_id: Option<String>,
    findings: Vec<String>,
    clean_areas: Vec<String>,
    unconfirmed: Vec<String>,
    processed_tool_results: usize,
}

impl ScriptedAssessmentClient {
    pub fn new(workdir: PathBuf) -> Self {
        Self {
            state: Arc::new(Mutex::new(ScriptedAssessmentState {
                workdir,
                step: 0,
                pane_id: None,
                findings: Vec::new(),
                clean_areas: Vec::new(),
                unconfirmed: Vec::new(),
                processed_tool_results: 0,
            })),
        }
    }
}

fn newest_tool_result<'a>(
    messages: &'a [Message],
    processed_count: usize,
) -> (Option<&'a str>, usize) {
    let mut seen = 0;
    let mut newest = None;

    for message in messages {
        for content in &message.content {
            if let ContentBlock::ToolResult { content, .. } = content {
                seen += 1;
                if seen > processed_count {
                    newest = Some(content.as_str());
                }
            }
        }
    }

    (newest, seen)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn contains_all(output: &str, expected: &[&str]) -> bool {
    expected.iter().all(|needle| output.contains(needle))
}

fn pause_command(seconds: f32) -> String {
    format!("python3 -c \"import time; time.sleep({seconds})\"")
}

#[async_trait]
impl LLMClient for ScriptedAssessmentClient {
    async fn create_message(
        &self,
        _system: &str,
        messages: &[Message],
        _tools: &[serde_json::Value],
        _max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)> {
        let mut state = self.state.lock().await;
        let (latest_tool_result, seen_count) =
            newest_tool_result(messages, state.processed_tool_results);
        if let Some(output) = latest_tool_result {
            match state.step {
                1 => {
                    if output.contains("__AMADEUS_EXIT_CODE__=0") {
                        state
                            .clean_areas
                            .push("`cargo check --features full` passed.".to_string());
                    } else {
                        state.findings.push(format!(
                            "`cargo check --features full` failed.\n\n```text\n{}\n```",
                            output.trim()
                        ));
                    }
                }
                2 => {
                    if output.contains("__AMADEUS_EXIT_CODE__=0") {
                        state
                            .clean_areas
                            .push("`cargo test --features full` passed.".to_string());
                    } else {
                        state.findings.push(format!(
                            "`cargo test --features full` failed.\n\n```text\n{}\n```",
                            output.trim()
                        ));
                    }
                }
                3 => {
                    let pane_id = output
                        .lines()
                        .rev()
                        .find(|line| line.contains(':'))
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .map(ToOwned::to_owned);
                    if pane_id.is_none() {
                        state.findings.push(format!(
                            "Failed to parse tmux pane id from launch output.\n\n```text\n{}\n```",
                            output.trim()
                        ));
                    }
                    state.pane_id = pane_id;
                }
                4 => {
                    if contains_all(output, &["Amadeus v0.1.0", "? for shortcuts", "[root]"]) {
                        state.clean_areas.push(
                            "TUI startup smoke anchors are visible in tmux capture.".to_string(),
                        );
                    } else {
                        state.unconfirmed.push(format!(
                            "Startup smoke capture is missing expected anchors.\n\n```text\n{}\n```",
                            output.trim()
                        ));
                    }
                }
                5 => {
                    if output.contains("Next session") || output.contains("To parent") {
                        state
                            .clean_areas
                            .push("Help overlay rendered in tmux capture.".to_string());
                    } else {
                        state.unconfirmed.push(format!(
                            "Help overlay capture did not show the expected shortcut labels.\n\n```text\n{}\n```",
                            output.trim()
                        ));
                    }
                }
                6 => {
                    if output.contains("/new-agent") || output.contains("/help") {
                        state
                            .clean_areas
                            .push("Slash completion rendered in tmux capture.".to_string());
                    } else {
                        state.unconfirmed.push(format!(
                            "Slash completion capture did not show expected command suggestions.\n\n```text\n{}\n```",
                            output.trim()
                        ));
                    }
                }
                7 => {
                    if output.contains("//new-agent") {
                        state.unconfirmed.push(
                            "Session creation scenario reused an existing `/` input buffer, so the capture shows `//new-agent` instead of a clean command.".to_string(),
                        );
                    } else if output.contains("session1") {
                        state
                            .clean_areas
                            .push("Session creation rendered a second session tab.".to_string());
                    } else {
                        state.unconfirmed.push(format!(
                            "Creating a new session did not expose the expected `session1` tab.\n\n```text\n{}\n```",
                            output.trim()
                        ));
                    }
                }
                8 => {
                    if output.trim().is_empty() {
                        state.unconfirmed.push(
                            "Session switch capture returned a blank frame immediately after `Tab`."
                                .to_string(),
                        );
                    } else if output.contains("session1") || output.contains("[root]") {
                        state.clean_areas.push(
                            "Session switching preserved a non-blank tmux capture.".to_string(),
                        );
                    } else {
                        state.unconfirmed.push(format!(
                            "Session switch capture stayed non-blank but did not include an expected tab anchor.\n\n```text\n{}\n```",
                            output.trim()
                        ));
                    }
                }
                _ => {}
            }
        }
        state.processed_tool_results = seen_count;

        let response = match state.step {
            0 => Some(ContentBlock::ToolUse {
                id: "assessment_step_1".to_string(),
                name: "bash".to_string(),
                input: json!({
                    "command": "cargo check --features full; printf '\\n__AMADEUS_EXIT_CODE__=%s\\n' $?"
                }),
            }),
            1 => Some(ContentBlock::ToolUse {
                id: "assessment_step_2".to_string(),
                name: "bash".to_string(),
                input: json!({
                    "command": "cargo test --features full; printf '\\n__AMADEUS_EXIT_CODE__=%s\\n' $?"
                }),
            }),
            2 => Some(ContentBlock::ToolUse {
                id: "assessment_step_3".to_string(),
                name: "bash".to_string(),
                input: json!({"command": "tmux-cli launch \"zsh\""}),
            }),
            3 => {
                let pane = state.pane_id.clone().ok_or_else(|| {
                    AgentError::InvalidResponse(
                        "Assessment runner could not determine tmux pane id".to_string(),
                    )
                })?;
                let startup = format!(
                    "tmux-cli send {} --pane={} && {} && tmux-cli capture --pane={}",
                    shell_quote(&format!(
                        "cd {} && env ANTHROPIC_API_KEY=dummy cargo run --features full",
                        state.workdir.display()
                    )),
                    pane,
                    pause_command(3.0),
                    pane
                );
                Some(ContentBlock::ToolUse {
                    id: "assessment_step_4".to_string(),
                    name: "bash".to_string(),
                    input: json!({ "command": startup }),
                })
            }
            4 => {
                let pane = state.pane_id.clone().ok_or_else(|| {
                    AgentError::InvalidResponse(
                        "Assessment runner could not determine tmux pane id".to_string(),
                    )
                })?;
                Some(ContentBlock::ToolUse {
                    id: "assessment_step_5".to_string(),
                    name: "bash".to_string(),
                    input: json!({
                        "command": format!(
                            "tmux-cli send \"?\" --pane={} --enter=False && {} && tmux-cli capture --pane={}",
                            pane,
                            pause_command(1.0),
                            pane
                        )
                    }),
                })
            }
            5 => {
                let pane = state.pane_id.clone().ok_or_else(|| {
                    AgentError::InvalidResponse(
                        "Assessment runner could not determine tmux pane id".to_string(),
                    )
                })?;
                Some(ContentBlock::ToolUse {
                    id: "assessment_step_6".to_string(),
                    name: "bash".to_string(),
                    input: json!({
                        "command": format!(
                            "tmux-cli escape --pane={} && tmux-cli send \"/\" --pane={} --enter=False && {} && tmux-cli capture --pane={}",
                            pane,
                            pane,
                            pause_command(1.0),
                            pane
                        )
                    }),
                })
            }
            6 => {
                let pane = state.pane_id.clone().ok_or_else(|| {
                    AgentError::InvalidResponse(
                        "Assessment runner could not determine tmux pane id".to_string(),
                    )
                })?;
                Some(ContentBlock::ToolUse {
                    id: "assessment_step_7".to_string(),
                    name: "bash".to_string(),
                    input: json!({
                        "command": format!(
                            "tmux-cli escape --pane={} && tmux-cli send \"/new-agent\" --pane={} && {} && tmux-cli capture --pane={}",
                            pane,
                            pane,
                            pause_command(1.5),
                            pane
                        )
                    }),
                })
            }
            7 => {
                let pane = state.pane_id.clone().ok_or_else(|| {
                    AgentError::InvalidResponse(
                        "Assessment runner could not determine tmux pane id".to_string(),
                    )
                })?;
                Some(ContentBlock::ToolUse {
                    id: "assessment_step_8".to_string(),
                    name: "bash".to_string(),
                    input: json!({
                        "command": format!(
                            "tmux-cli send \"\\t\" --pane={} --enter=False && {} && tmux-cli capture --pane={}",
                            pane,
                            pause_command(1.0),
                            pane
                        )
                    }),
                })
            }
            8 => {
                if let Some(pane) = &state.pane_id {
                    Some(ContentBlock::ToolUse {
                        id: "assessment_step_9".to_string(),
                        name: "bash".to_string(),
                        input: json!({
                            "command": format!(
                                "tmux-cli interrupt --pane={} && tmux-cli kill --pane={}",
                                pane, pane
                            )
                        }),
                    })
                } else {
                    None
                }
            }
            _ => None,
        };

        state.step += 1;

        if let Some(block) = response {
            Ok(("tool_use".to_string(), vec![block]))
        } else {
            let findings = if state.findings.is_empty() {
                "- none".to_string()
            } else {
                state
                    .findings
                    .iter()
                    .map(|item| format!("- {}", item.replace('\n', "\n  ")))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            let clean = if state.clean_areas.is_empty() {
                "- none".to_string()
            } else {
                state
                    .clean_areas
                    .iter()
                    .map(|item| format!("- {}", item))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            let unconfirmed = if state.unconfirmed.is_empty() {
                "- none".to_string()
            } else {
                state
                    .unconfirmed
                    .iter()
                    .map(|item| format!("- {}", item.replace('\n', "\n  ")))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            let summary = if state.findings.is_empty() {
                "No confirmed bugs found."
            } else {
                "Confirmed bugs were found during the read-only automation pass."
            };
            Ok((
                "end_turn".to_string(),
                vec![ContentBlock::Text {
                    text: format!(
                        "## Assessment Summary\n\n{}\n\n## Confirmed Bugs\n\n{}\n\n## Unconfirmed Findings\n\n{}\n\n## Clean Areas\n\n{}",
                        summary, findings, unconfirmed, clean
                    ),
                }],
            ))
        }
    }

    async fn create_message_stream(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        max_tokens: u32,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<StreamEvent>> + Send>>> {
        let (stop_reason, blocks) = self
            .create_message(system, messages, tools, max_tokens)
            .await?;
        let mut events = Vec::new();

        for block in blocks {
            match block {
                ContentBlock::Text { text } => {
                    events.push(Ok(StreamEvent::TextDelta(text)));
                }
                ContentBlock::ToolUse { id, name, input } => {
                    let tool_id = id.clone();
                    events.push(Ok(StreamEvent::ToolCallStart { id, name }));
                    events.push(Ok(StreamEvent::ToolCallDelta {
                        arguments: input.to_string(),
                    }));
                    events.push(Ok(StreamEvent::ToolCallDone(tool_id)));
                }
                _ => {}
            }
        }

        events.push(Ok(StreamEvent::StopReason(stop_reason)));
        Ok(Box::pin(stream::iter(events)))
    }
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;

    use futures::stream;
    use tempfile::tempdir;

    use super::*;
    use crate::agent::messages::{ContentBlock, Message};
    use crate::client::{LLMClient, StreamEvent};

    #[derive(Clone, Default)]
    struct StubClient;

    #[async_trait]
    impl LLMClient for StubClient {
        async fn create_message(
            &self,
            _system: &str,
            _messages: &[Message],
            _tools: &[serde_json::Value],
            _max_tokens: u32,
        ) -> Result<(String, Vec<ContentBlock>)> {
            Ok((
                "end_turn".to_string(),
                vec![ContentBlock::Text {
                    text: "No confirmed bugs found.".to_string(),
                }],
            ))
        }

        async fn create_message_stream(
            &self,
            _system: &str,
            _messages: &[Message],
            _tools: &[serde_json::Value],
            _max_tokens: u32,
        ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<StreamEvent>> + Send>>> {
            Ok(Box::pin(stream::iter(vec![
                Ok(StreamEvent::TextDelta(
                    "No confirmed bugs found.".to_string(),
                )),
                Ok(StreamEvent::StopReason("end_turn".to_string())),
            ])))
        }
    }

    #[tokio::test]
    async fn assessment_runner_writes_report() {
        let temp = tempdir().unwrap();
        let skill_dir = temp.path().join(".amadeus/skills/feature-assessment-loop");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: feature-assessment-loop
description: Assess the project in read-only mode
allowed_tools:
  - read_file
---

Produce a short report. Context: {context}
"#,
        )
        .unwrap();

        let config = Arc::new(Config {
            api_key: "test".to_string(),
            workdir: temp.path().to_path_buf(),
            ..Config::default()
        });
        let runner = AssessmentRunner::new(StubClient, config);
        let result = runner
            .run(AssessmentConfig::new(
                temp.path().join("reports"),
                default_prompt(temp.path()),
            ))
            .await
            .unwrap();

        assert!(result.report_path.exists());
        assert!(result.report_markdown.contains("Feature Assessment Report"));
        assert!(result.report_markdown.contains("No confirmed bugs found."));
        assert_eq!(result.permission_mode, "read-only");
    }
}
