// @amadeus-header
// summary: Conversation export artifact model, markdown/JSON renderers, and atomic file writer.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::export
// - type: crate::ui::export::ExportArtifact
// - type: crate::ui::export::SessionHeader
// - type: crate::ui::export::ConfigSnapshot
// - type: crate::ui::export::ExportTurn
// - type: crate::ui::export::ExportTurnKind
// - type: crate::ui::export::ExportedTool
// - type: crate::ui::export::ExportStatistics
// - type: crate::ui::export::ExportFormat
// - const: crate::ui::export::EXPORT_SCHEMA_VERSION
// - fn: crate::ui::export::build_export
// - fn: crate::ui::export::to_markdown
// - fn: crate::ui::export::to_json
// - fn: crate::ui::export::write_export
// - fn: crate::ui::export::default_export_path
// - fn: crate::ui::export::detect_format
// uses:
// - type: crate::agent::loop_agent::Agent<C>
// - type: crate::commands::context::ContextReport
// - type: crate::ui::components::messages::MessagesComponent
// - type: crate::ui::components::messages::HistoryItem
// - type: crate::ui::components::messages::CompressionItem
// - type: crate::ui::components::messages::CompressionStatus
// - type: crate::ui::components::tool_group::ToolCall
// - type: crate::ui::components::tool_group::ToolStatus
// - runtime: chrono date and time utilities
// - artifact: conversation export markdown or JSON file
// - protocol: serde serialization
// invariants:
// - Export schema version stays forward-compatible via a top-level schema_version field.
// - Path collisions append a timestamp suffix; the original target path is never overwritten silently.
// - Markdown and JSON exports share the same ExportArtifact, so they cannot drift.
// side_effects:
// - Reads or writes filesystem state when write_export is called.
// tests:
// - cmd: cargo test --features full export
// @end-amadeus-header

//! Conversation export pipeline.
//!
//! Builds a self-contained `ExportArtifact` from a `MessagesComponent` plus the
//! active `Agent`, then renders it to either Markdown or JSON. The same artifact
//! feeds both formats so they cannot drift, and the writer handles parent
//! directory creation, path collision suffixes, and atomic replacement.

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::loop_agent::Agent;
use crate::client::LLMClient;
use crate::commands::context::{
    build_context_report, ContextReport, ContextSection, ContextSectionGroup,
};
use crate::ui::components::{
    CompressionItem, CompressionStatus, HistoryItem, MessagesComponent, ToolCall, ToolStatus,
};

pub const EXPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportArtifact {
    pub schema_version: u32,
    pub exported_at: String,
    pub session: SessionHeader,
    pub config: ConfigSnapshot,
    pub context: ContextReport,
    pub turns: Vec<ExportTurn>,
    pub statistics: ExportStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHeader {
    pub session_id: String,
    pub label: String,
    pub parent_session_id: Option<String>,
    pub workdir: String,
    pub model: String,
    pub subagent_depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub provider: String,
    pub model: String,
    pub permission_mode: String,
    pub workdir: String,
    pub config_roots: Vec<String>,
    pub hook_paths: Vec<String>,
    pub agent_roots: Vec<String>,
    pub skill_roots: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportTurn {
    pub index: usize,
    pub turn: Option<usize>,
    pub kind: ExportTurnKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExportTurnKind {
    User {
        content: String,
    },
    Assistant {
        content: String,
    },
    Thinking {
        content: String,
    },
    ToolGroup {
        tools: Vec<ExportedTool>,
    },
    LocalCommand {
        content: String,
    },
    SubAgentPrompt {
        content: String,
        depth: usize,
    },
    Compression {
        original_tokens: Option<usize>,
        new_tokens: Option<usize>,
        status: String,
        error: Option<String>,
    },
    ContextReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedTool {
    pub id: String,
    pub name: String,
    pub command: Option<String>,
    pub output: String,
    pub status: String,
    pub progress_message: Option<String>,
    pub progress_percent: Option<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExportStatistics {
    pub total_turns: usize,
    pub user_messages: usize,
    pub assistant_messages: usize,
    pub tool_calls: usize,
    pub tool_errors: usize,
    pub compactions: usize,
    pub subagent_prompts: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
    Json,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Markdown => "md",
            ExportFormat::Json => "json",
        }
    }
}

pub fn build_export<C: LLMClient + Clone + 'static>(
    messages: &MessagesComponent,
    agent: &Agent<C>,
    session: &SessionHeader,
) -> ExportArtifact {
    let config = build_config_snapshot(agent);
    let context = build_context_report(agent);
    let (turns, statistics) = build_turns(messages);

    ExportArtifact {
        schema_version: EXPORT_SCHEMA_VERSION,
        exported_at: Utc::now().to_rfc3339(),
        session: session.clone(),
        config,
        context,
        turns,
        statistics,
    }
}

fn build_config_snapshot<C: LLMClient + Clone + 'static>(agent: &Agent<C>) -> ConfigSnapshot {
    let config = agent.config();
    let provider = match config.provider {
        crate::agent::config::Provider::Anthropic => "anthropic",
        crate::agent::config::Provider::OpenAI => "openai",
    };
    ConfigSnapshot {
        provider: provider.to_string(),
        model: config.model.clone(),
        permission_mode: config.permission_mode.as_str().to_string(),
        workdir: config.workdir.to_string_lossy().to_string(),
        config_roots: config
            .config_roots()
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        hook_paths: config
            .hook_paths()
            .into_iter()
            .map(|(p, _)| p.to_string_lossy().to_string())
            .collect(),
        agent_roots: config
            .agent_roots()
            .into_iter()
            .map(|(_, p)| p.to_string_lossy().to_string())
            .collect(),
        skill_roots: config
            .skill_roots()
            .into_iter()
            .map(|(_, p)| p.to_string_lossy().to_string())
            .collect(),
    }
}

fn build_turns(messages: &MessagesComponent) -> (Vec<ExportTurn>, ExportStatistics) {
    let mut turns = Vec::with_capacity(messages.len());
    let mut stats = ExportStatistics::default();
    let mut last_turn: Option<usize> = None;

    for (index, item) in messages.iter_items().enumerate() {
        let (kind, turn) = classify_item(item, &mut last_turn);
        update_statistics(&kind, &mut stats);
        turns.push(ExportTurn { index, turn, kind });
    }

    stats.total_turns = stats.user_messages.max(stats.assistant_messages);
    (turns, stats)
}

fn classify_item(
    item: &HistoryItem,
    last_turn: &mut Option<usize>,
) -> (ExportTurnKind, Option<usize>) {
    match item {
        HistoryItem::User { content, turn, .. } => {
            *last_turn = Some(*turn);
            (
                ExportTurnKind::User {
                    content: content.clone(),
                },
                Some(*turn),
            )
        }
        HistoryItem::Assistant { content, turn, .. } => (
            ExportTurnKind::Assistant {
                content: content.clone(),
            },
            Some(*turn),
        ),
        HistoryItem::LocalCommand { content, .. } => (
            ExportTurnKind::LocalCommand {
                content: content.clone(),
            },
            None,
        ),
        HistoryItem::SubAgentPrompt {
            content,
            depth,
            turn,
        } => (
            ExportTurnKind::SubAgentPrompt {
                content: content.clone(),
                depth: *depth,
            },
            Some(*turn),
        ),
        HistoryItem::Thinking { content, turn, .. } => (
            ExportTurnKind::Thinking {
                content: content.clone(),
            },
            Some(*turn),
        ),
        HistoryItem::ToolGroup { group, turn } => {
            let tools = group.tools.iter().map(exported_tool).collect();
            (ExportTurnKind::ToolGroup { tools }, Some(*turn))
        }
        HistoryItem::Compression { compression } => (
            ExportTurnKind::Compression {
                original_tokens: compression.original_token_count,
                new_tokens: compression.new_token_count,
                status: compression_status_label(compression).to_string(),
                error: compression.error_message.clone(),
            },
            None,
        ),
        HistoryItem::ContextReport { turn, .. } => (ExportTurnKind::ContextReport, Some(*turn)),
    }
}

fn exported_tool(tool: &ToolCall) -> ExportedTool {
    ExportedTool {
        id: tool.id.clone(),
        name: tool.name.clone(),
        command: tool.command.clone(),
        output: tool.output.clone(),
        status: tool_status_label(tool.status).to_string(),
        progress_message: tool.progress_message.clone(),
        progress_percent: tool.progress_percent,
    }
}

fn tool_status_label(status: ToolStatus) -> &'static str {
    match status {
        ToolStatus::Pending => "pending",
        ToolStatus::Success => "success",
        ToolStatus::Error => "error",
    }
}

fn compression_status_label(item: &CompressionItem) -> &'static str {
    match item.status {
        CompressionStatus::Compressed => "compressed",
        CompressionStatus::NotBeneficial => "not_beneficial",
        CompressionStatus::Failed => "failed",
        CompressionStatus::Noop => "noop",
        CompressionStatus::Pending => "pending",
    }
}

fn update_statistics(kind: &ExportTurnKind, stats: &mut ExportStatistics) {
    match kind {
        ExportTurnKind::User { .. } => stats.user_messages += 1,
        ExportTurnKind::Assistant { .. } => stats.assistant_messages += 1,
        ExportTurnKind::ToolGroup { tools } => {
            stats.tool_calls += tools.len();
            stats.tool_errors += tools.iter().filter(|t| t.status == "error").count();
        }
        ExportTurnKind::SubAgentPrompt { .. } => stats.subagent_prompts += 1,
        ExportTurnKind::Compression { .. } => stats.compactions += 1,
        ExportTurnKind::Thinking { .. }
        | ExportTurnKind::LocalCommand { .. }
        | ExportTurnKind::ContextReport => {}
    }
}

pub fn to_json(artifact: &ExportArtifact) -> Value {
    serde_json::to_value(artifact).unwrap_or(Value::Null)
}

pub fn to_markdown(artifact: &ExportArtifact) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push("# Amadeus Conversation Export".to_string());
    lines.push(String::new());
    lines.push(format!("- **Exported at:** {}", artifact.exported_at));
    lines.push(format!("- **Schema version:** {}", artifact.schema_version));
    lines.push(format!(
        "- **Session:** {} (id={})",
        artifact.session.label, artifact.session.session_id
    ));
    if let Some(parent) = &artifact.session.parent_session_id {
        lines.push(format!("- **Parent session:** {parent}"));
    }
    lines.push(format!("- **Model:** {}", artifact.session.model));
    lines.push(format!("- **Workdir:** {}", artifact.session.workdir));
    if artifact.session.subagent_depth > 0 {
        lines.push(format!(
            "- **Sub-agent depth:** {}",
            artifact.session.subagent_depth
        ));
    }
    lines.push(String::new());

    lines.push("## Configuration".to_string());
    lines.push(String::new());
    lines.push("| Key | Value |".to_string());
    lines.push("| --- | --- |".to_string());
    lines.push(format!("| provider | `{}` |", artifact.config.provider));
    lines.push(format!("| model | `{}` |", artifact.config.model));
    lines.push(format!(
        "| permission_mode | `{}` |",
        artifact.config.permission_mode
    ));
    lines.push(format!("| workdir | `{}` |", artifact.config.workdir));
    lines.push(format!(
        "| config_roots | {} |",
        render_string_list(&artifact.config.config_roots)
    ));
    lines.push(format!(
        "| hook_paths | {} |",
        render_string_list(&artifact.config.hook_paths)
    ));
    lines.push(format!(
        "| agent_roots | {} |",
        render_string_list(&artifact.config.agent_roots)
    ));
    lines.push(format!(
        "| skill_roots | {} |",
        render_string_list(&artifact.config.skill_roots)
    ));
    lines.push(String::new());

    lines.push("## Context".to_string());
    lines.push(String::new());
    let ctx = &artifact.context;
    let used = ctx.used_tokens();
    let window = ctx.context_window_size as usize;
    let pct = ctx.usage_percent();
    lines.push(format!(
        "Model `{}`, window {} tokens, used {} (~{}%).",
        ctx.model_name,
        ContextReport::fmt_tokens(window),
        ContextReport::fmt_tokens(used),
        pct
    ));
    lines.push(String::new());
    lines.push("| Bucket | Tokens |".to_string());
    lines.push("| --- | --- |".to_string());
    lines.push(format!(
        "| system_prompt | {} |",
        ContextReport::fmt_tokens(ctx.system_prompt_tokens)
    ));
    lines.push(format!(
        "| system_tools | {} |",
        ContextReport::fmt_tokens(ctx.system_tools_tokens)
    ));
    lines.push(format!(
        "| additional_tools | {} |",
        ContextReport::fmt_tokens(ctx.additional_tools_tokens)
    ));
    lines.push(format!(
        "| memory_files | {} |",
        ContextReport::fmt_tokens(ctx.memory_files_tokens)
    ));
    lines.push(format!(
        "| conversation | {} |",
        ContextReport::fmt_tokens(ctx.conversation_tokens)
    ));
    lines.push(String::new());
    for section in &ctx.sections {
        render_context_section(&mut lines, section);
    }
    if !ctx.suggestions.is_empty() {
        lines.push("### Suggestions".to_string());
        lines.push(String::new());
        for suggestion in &ctx.suggestions {
            lines.push(format!("- {suggestion}"));
        }
        lines.push(String::new());
    }

    lines.push("## Conversation".to_string());
    lines.push(String::new());
    for turn in &artifact.turns {
        render_turn(&mut lines, turn);
    }

    lines.push("## Statistics".to_string());
    lines.push(String::new());
    let stats = &artifact.statistics;
    lines.push(format!("- Total turns: {}", stats.total_turns));
    lines.push(format!("- User messages: {}", stats.user_messages));
    lines.push(format!(
        "- Assistant messages: {}",
        stats.assistant_messages
    ));
    lines.push(format!("- Tool calls: {}", stats.tool_calls));
    lines.push(format!("- Tool errors: {}", stats.tool_errors));
    lines.push(format!("- Compactions: {}", stats.compactions));
    lines.push(format!("- Sub-agent prompts: {}", stats.subagent_prompts));
    lines.push(String::new());

    lines.join("\n")
}

fn render_string_list(values: &[String]) -> String {
    if values.is_empty() {
        "`-`".to_string()
    } else {
        values
            .iter()
            .map(|v| format!("`{v}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn render_context_section(lines: &mut Vec<String>, section: &ContextSection) {
    lines.push(format!("### {}", section.title));
    if let Some(hint) = &section.command_hint {
        lines.push(format!("_Hint: `{hint}`_"));
    }
    lines.push(String::new());
    for group in &section.groups {
        render_context_group(lines, group);
    }
}

fn render_context_group(lines: &mut Vec<String>, group: &ContextSectionGroup) {
    if let Some(title) = &group.title {
        lines.push(format!("- **{title}**"));
    }
    if group.entries.is_empty() {
        lines.push("  - (none)".to_string());
    } else {
        for entry in &group.entries {
            lines.push(format!(
                "  - `{}` — {} tokens",
                entry.label,
                ContextReport::fmt_tokens(entry.tokens)
            ));
        }
    }
}

fn render_turn(lines: &mut Vec<String>, turn: &ExportTurn) {
    let badge = turn
        .turn
        .map(|t| format!(" [turn {t}]"))
        .unwrap_or_default();
    match &turn.kind {
        ExportTurnKind::User { content } => {
            lines.push(format!("### User{badge}"));
            lines.push(String::new());
            for line in content.lines() {
                lines.push(format!("> {line}"));
            }
            lines.push(String::new());
        }
        ExportTurnKind::Assistant { content } => {
            lines.push(format!("### Assistant{badge}"));
            lines.push(String::new());
            lines.push(content.clone());
            lines.push(String::new());
        }
        ExportTurnKind::Thinking { content } => {
            lines.push(format!("### Thinking{badge}"));
            lines.push(String::new());
            lines.push("<details><summary>Thinking</summary>".to_string());
            lines.push(String::new());
            for line in content.lines() {
                lines.push(format!("> {line}"));
            }
            lines.push(String::new());
            lines.push("</details>".to_string());
            lines.push(String::new());
        }
        ExportTurnKind::ToolGroup { tools } => {
            lines.push(format!("### Tool group{badge}"));
            lines.push(String::new());
            if tools.is_empty() {
                lines.push("_(no tool calls)_".to_string());
                lines.push(String::new());
            }
            for tool in tools {
                lines.push(format!("#### `{}` — {}", tool.name, tool.status));
                if let Some(progress) = &tool.progress_message {
                    let pct = tool
                        .progress_percent
                        .map(|p| format!(" ({p}%)"))
                        .unwrap_or_default();
                    lines.push(format!("> progress{pct}: {progress}"));
                }
                lines.push(String::new());
                if let Some(cmd) = &tool.command {
                    lines.push("```bash".to_string());
                    lines.push(cmd.clone());
                    lines.push("```".to_string());
                    lines.push(String::new());
                }
                if !tool.output.is_empty() {
                    lines.push("```text".to_string());
                    for line in tool.output.lines() {
                        lines.push(line.to_string());
                    }
                    lines.push("```".to_string());
                    lines.push(String::new());
                }
            }
        }
        ExportTurnKind::LocalCommand { content } => {
            lines.push("### Local command".to_string());
            lines.push(String::new());
            lines.push("```text".to_string());
            for line in content.lines() {
                lines.push(line.to_string());
            }
            lines.push("```".to_string());
            lines.push(String::new());
        }
        ExportTurnKind::SubAgentPrompt { content, depth } => {
            lines.push(format!("### Sub-agent (depth {depth}){badge}"));
            lines.push(String::new());
            for line in content.lines() {
                lines.push(format!("> {line}"));
            }
            lines.push(String::new());
        }
        ExportTurnKind::Compression {
            original_tokens,
            new_tokens,
            status,
            error,
        } => {
            let detail = match (original_tokens, new_tokens) {
                (Some(o), Some(n)) => format!("{o} → {n} tokens"),
                _ => "incomplete telemetry".to_string(),
            };
            lines.push(format!("### Compaction ({status}): {detail}"));
            if let Some(err) = error {
                lines.push(format!("_Error: {err}_"));
            }
            lines.push(String::new());
        }
        ExportTurnKind::ContextReport => {
            lines.push(format!("### Context report{badge}"));
            lines.push(String::new());
            lines.push(
                "_Full context report embedded in the JSON export and rendered in the `## Context` section above._"
                    .to_string(),
            );
            lines.push(String::new());
        }
    }
}

pub fn detect_format(path: &Path) -> ExportFormat {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("json") => ExportFormat::Json,
        _ => ExportFormat::Markdown,
    }
}

pub fn default_export_path(workdir: &Path, session_label: &str, session_id: usize) -> PathBuf {
    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let safe_label = sanitize_for_filename(session_label);
    let exports_dir = workdir.join(".amadeus").join("exports");
    exports_dir.join(format!("conversation-{safe_label}-{session_id}-{stamp}.md"))
}

fn sanitize_for_filename(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn write_export(artifact: &ExportArtifact, path: &Path) -> std::io::Result<PathBuf> {
    let format = detect_format(path);
    write_export_with_format(artifact, path, format)
}

pub fn write_export_with_format(
    artifact: &ExportArtifact,
    path: &Path,
    format: ExportFormat,
) -> std::io::Result<PathBuf> {
    let target = resolve_target_path(path, format)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let body = match format {
        ExportFormat::Json => {
            let value = to_json(artifact);
            serde_json::to_string_pretty(&value).map_err(std::io::Error::other)?
        }
        ExportFormat::Markdown => to_markdown(artifact),
    };

    let tmp = target.with_extension(format!(
        "{}.tmp",
        target.extension().and_then(|e| e.to_str()).unwrap_or("out")
    ));
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, &target)?;
    Ok(target)
}

fn resolve_target_path(path: &Path, format: ExportFormat) -> std::io::Result<PathBuf> {
    if !path.exists() {
        return Ok(path.to_path_buf());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("export");
    let stamp = Utc::now().format("%Y%m%dT%H%M%S%3fZ").to_string();
    let mut candidate = parent.join(format!("{stem}-{stamp}.{}", format.extension()));
    let mut counter = 1u32;
    while candidate.exists() {
        candidate = parent.join(format!("{stem}-{stamp}-{counter}.{}", format.extension()));
        counter += 1;
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::components::{CompressionItem, MessagesComponent, ToolGroup};

    fn fixture_messages() -> MessagesComponent {
        let mut m = MessagesComponent::new();
        m.add_user("Hello there".to_string());
        m.add_assistant("General Kenobi!".to_string());
        m.add_local_command_result("Local: /help shown".to_string());
        m.add_subagent_prompt("Investigate the bug".to_string(), 1);
        let tool = ToolCall::new("tool_1".to_string(), "bash".to_string())
            .with_command("ls -la".to_string())
            .complete("file1\nfile2".to_string(), false);
        m.add_user("Run ls".to_string());
        let mut group = ToolGroup::new();
        group.add_tool(tool);
        m.push_tool_group(group, 2);
        let failed = CompressionItem::failed("network error".to_string());
        m.push_compression(failed);
        m
    }

    #[test]
    fn build_export_emits_one_entry_per_history_item() {
        let messages = fixture_messages();
        let turns = build_turns(&messages);
        let (items, _) = turns;
        assert_eq!(items.len(), messages.len());
        for (index, turn) in items.iter().enumerate() {
            assert_eq!(turn.index, index);
        }
    }

    #[test]
    fn build_export_counts_tool_calls_and_errors() {
        let messages = fixture_messages();
        let (_, stats) = build_turns(&messages);
        assert_eq!(stats.user_messages, 2);
        assert_eq!(stats.assistant_messages, 1);
        assert_eq!(stats.tool_calls, 1);
        assert_eq!(stats.tool_errors, 0);
        assert_eq!(stats.compactions, 1);
        assert_eq!(stats.subagent_prompts, 1);
    }

    #[test]
    fn markdown_export_includes_expected_sections() {
        let messages = fixture_messages();
        let session = SessionHeader {
            session_id: "0".to_string(),
            label: "root".to_string(),
            parent_session_id: None,
            workdir: "/tmp".to_string(),
            model: "test-model".to_string(),
            subagent_depth: 0,
        };
        let json = serde_json::json!({
            "model_name": "test-model",
            "context_window_size": 200_000_u32,
            "system_prompt_tokens": 100_usize,
            "system_tools_tokens": 50_usize,
            "additional_tools_tokens": 0_usize,
            "memory_files_tokens": 0_usize,
            "conversation_tokens": 25_usize,
            "sections": [],
            "suggestions": ["Run /compact soon".to_string()],
        });
        let context: ContextReport = serde_json::from_value(json).unwrap();
        let mut artifact = ExportArtifact {
            schema_version: EXPORT_SCHEMA_VERSION,
            exported_at: "2026-06-02T15:30:12Z".to_string(),
            session,
            config: ConfigSnapshot {
                provider: "anthropic".to_string(),
                model: "test-model".to_string(),
                permission_mode: "prompt".to_string(),
                workdir: "/tmp".to_string(),
                config_roots: vec!["/etc/amadeus".to_string()],
                hook_paths: vec![],
                agent_roots: vec![],
                skill_roots: vec![],
            },
            context,
            turns: build_turns(&messages).0,
            statistics: build_turns(&messages).1,
        };
        let markdown = to_markdown(&artifact);
        assert!(markdown.contains("# Amadeus Conversation Export"));
        assert!(markdown.contains("## Configuration"));
        assert!(markdown.contains("## Context"));
        assert!(markdown.contains("## Conversation"));
        assert!(markdown.contains("## Statistics"));
        assert!(markdown.contains("Hello there"));
        assert!(markdown.contains("General Kenobi!"));
        assert!(markdown.contains("Investigate the bug"));
        assert!(markdown.contains("ls -la"));
        assert!(markdown.contains("Run /compact soon"));
        // Stats
        assert!(markdown.contains("- Tool calls: 1"));
        artifact.context.suggestions.clear();
    }

    #[test]
    fn json_export_round_trips() {
        let messages = fixture_messages();
        let session = SessionHeader {
            session_id: "0".to_string(),
            label: "root".to_string(),
            parent_session_id: None,
            workdir: "/tmp".to_string(),
            model: "test-model".to_string(),
            subagent_depth: 0,
        };
        let artifact = ExportArtifact {
            schema_version: EXPORT_SCHEMA_VERSION,
            exported_at: "2026-06-02T15:30:12Z".to_string(),
            session,
            config: ConfigSnapshot {
                provider: "anthropic".to_string(),
                model: "test-model".to_string(),
                permission_mode: "prompt".to_string(),
                workdir: "/tmp".to_string(),
                config_roots: vec![],
                hook_paths: vec![],
                agent_roots: vec![],
                skill_roots: vec![],
            },
            context: ContextReport {
                model_name: "test-model".to_string(),
                context_window_size: 200_000,
                system_prompt_tokens: 0,
                system_tools_tokens: 0,
                additional_tools_tokens: 0,
                memory_files_tokens: 0,
                conversation_tokens: 0,
                sections: vec![],
                suggestions: vec![],
            },
            turns: build_turns(&messages).0,
            statistics: build_turns(&messages).1,
        };
        let value = to_json(&artifact);
        let parsed: ExportArtifact = serde_json::from_value(value).unwrap();
        assert_eq!(parsed.schema_version, EXPORT_SCHEMA_VERSION);
        assert_eq!(parsed.turns.len(), artifact.turns.len());
    }

    #[test]
    fn detect_format_picks_extension() {
        assert_eq!(
            detect_format(Path::new("/tmp/foo.json")),
            ExportFormat::Json
        );
        assert_eq!(
            detect_format(Path::new("/tmp/foo.md")),
            ExportFormat::Markdown
        );
        assert_eq!(
            detect_format(Path::new("/tmp/foo.MARKDOWN")),
            ExportFormat::Markdown
        );
    }

    #[test]
    fn default_export_path_uses_amadeus_exports() {
        let path = default_export_path(Path::new("/work"), "root", 0);
        assert!(path.starts_with("/work/.amadeus/exports/"));
        assert!(path.to_string_lossy().ends_with(".md"));
    }

    #[test]
    fn write_export_creates_parent_dirs_and_collision_suffix() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("export.md");
        let session = SessionHeader {
            session_id: "0".to_string(),
            label: "root".to_string(),
            parent_session_id: None,
            workdir: dir.path().to_string_lossy().to_string(),
            model: "test-model".to_string(),
            subagent_depth: 0,
        };
        let artifact = ExportArtifact {
            schema_version: EXPORT_SCHEMA_VERSION,
            exported_at: "2026-06-02T15:30:12Z".to_string(),
            session,
            config: ConfigSnapshot {
                provider: "anthropic".to_string(),
                model: "test-model".to_string(),
                permission_mode: "prompt".to_string(),
                workdir: dir.path().to_string_lossy().to_string(),
                config_roots: vec![],
                hook_paths: vec![],
                agent_roots: vec![],
                skill_roots: vec![],
            },
            context: ContextReport {
                model_name: "test-model".to_string(),
                context_window_size: 200_000,
                system_prompt_tokens: 0,
                system_tools_tokens: 0,
                additional_tools_tokens: 0,
                memory_files_tokens: 0,
                conversation_tokens: 0,
                sections: vec![],
                suggestions: vec![],
            },
            turns: vec![],
            statistics: ExportStatistics::default(),
        };
        let written = write_export(&artifact, &target).unwrap();
        assert_eq!(written, target);
        assert!(target.exists());

        let second = write_export(&artifact, &target).unwrap();
        assert_ne!(second, target);
        assert!(second.exists());
        let body = std::fs::read_to_string(&second).unwrap();
        assert!(body.contains("# Amadeus Conversation Export"));
    }
}
