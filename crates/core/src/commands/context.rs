// @amadeus-header
// summary: Core context report model and builder shared by slash-command frontends.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::commands::context
// - type: crate::commands::context::ContextEntry
// - type: crate::commands::context::ContextReport
// - type: crate::commands::context::ContextSection
// - type: crate::commands::context::ContextSectionGroup
// - fn: crate::commands::context::build_context_report
// uses:
// - module: crate::agent::config::Config
// - module: crate::agent::loop_agent::Agent<C>
// - module: crate::client::LLMClient
// - module: crate::context::ProjectContext
// - module: crate::prompts
// - module: crate::skills::registry::SkillRegistry
// - module: crate::tools::registry::ToolRegistry
// - artifact: filesystem paths and files
// invariants:
// - Reported token totals reflect the current live agent payload plus clearly separated on-disk inventory.
// - Tool classification keeps built-in tools separate from additional registered tools.
// side_effects:
// - Reads context, skill, and custom-agent files from disk.
// tests:
// - cmd: cargo test --features full build_context_report
// @end-amadeus-header

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::agent::{Agent, Message};
use crate::client::LLMClient;
use crate::context::ProjectContext;
use crate::prompts;
use crate::skills::registry::SkillRegistry;

const BUILTIN_TOOL_NAMES: &[&str] = &[
    "bash",
    "read_file",
    "write_file",
    "edit_file",
    "glob",
    "grep",
    "todo",
    "web_fetch",
    "sub_agent",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEntry {
    pub label: String,
    pub tokens: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSectionGroup {
    pub title: Option<String>,
    pub entries: Vec<ContextEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSection {
    pub title: String,
    pub command_hint: Option<String>,
    pub groups: Vec<ContextSectionGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextReport {
    pub model_name: String,
    pub context_window_size: u32,
    pub system_prompt_tokens: usize,
    pub system_tools_tokens: usize,
    pub additional_tools_tokens: usize,
    pub memory_files_tokens: usize,
    pub conversation_tokens: usize,
    pub sections: Vec<ContextSection>,
    pub suggestions: Vec<String>,
}

impl ContextReport {
    pub fn used_tokens(&self) -> usize {
        self.system_prompt_tokens
            + self.system_tools_tokens
            + self.additional_tools_tokens
            + self.memory_files_tokens
            + self.conversation_tokens
    }

    pub fn free_tokens(&self) -> usize {
        (self.context_window_size as usize).saturating_sub(self.used_tokens())
    }

    pub fn usage_percent(&self) -> u8 {
        if self.context_window_size == 0 {
            return 0;
        }
        let pct = (self.used_tokens() as f64 / self.context_window_size as f64 * 100.0) as u8;
        pct.min(100)
    }

    pub fn pct_of(&self, tokens: usize) -> f64 {
        if self.context_window_size == 0 {
            return 0.0;
        }
        tokens as f64 / self.context_window_size as f64 * 100.0
    }

    pub fn fmt_tokens(n: usize) -> String {
        if n >= 1_000_000 {
            format!("{:.1}M", n as f64 / 1_000_000.0)
        } else if n >= 1_000 {
            format!("{:.1}k", n as f64 / 1_000.0)
        } else {
            n.to_string()
        }
    }
}

pub fn build_context_report<C: LLMClient + Clone + 'static>(agent: &Agent<C>) -> ContextReport {
    let config = agent.config();
    let include_sub_agent_tool = agent.subagent_depth() < config.max_subagent_depth;
    let system_prompt = prompts::render_system_prompt(
        &config.workdir.display().to_string(),
        include_sub_agent_tool,
    );
    let system_prompt_tokens = estimate_tokens(&system_prompt);

    let project_context = ProjectContext::load(&config.workdir);
    let memory_entries = project_context
        .iter()
        .map(|ctx| ContextEntry {
            label: display_path(&ctx.source, &config.workdir),
            tokens: estimate_tokens(&ctx.content),
        })
        .collect::<Vec<_>>();
    let memory_files_tokens = memory_entries.iter().map(|entry| entry.tokens).sum();

    let registry = agent.registry();
    let builtin_tool_names: HashSet<&str> = BUILTIN_TOOL_NAMES.iter().copied().collect();
    let mut builtin_tools = Vec::new();
    let mut additional_tools = Vec::new();
    for name in registry.names() {
        let tokens = registry
            .get(name)
            .map(|tool| estimate_tokens(&serde_json::to_string(tool.schema()).unwrap_or_default()))
            .unwrap_or(0);
        let entry = ContextEntry {
            label: name.to_string(),
            tokens,
        };
        if builtin_tool_names.contains(name) {
            builtin_tools.push(entry);
        } else {
            additional_tools.push(entry);
        }
    }
    builtin_tools.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.label.cmp(&b.label)));
    additional_tools.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.label.cmp(&b.label)));
    let system_tools_tokens = builtin_tools.iter().map(|entry| entry.tokens).sum();
    let additional_tools_tokens = additional_tools.iter().map(|entry| entry.tokens).sum();

    let history = agent.history();
    let message_entries = history
        .try_read()
        .map(|guard| build_message_entries(&guard))
        .unwrap_or_default();
    let conversation_tokens = message_entries.iter().map(|entry| entry.tokens).sum();

    let skill_groups = build_skill_groups(&config);
    let custom_agent_groups = build_custom_agent_groups(&config);
    let suggestions = build_suggestions(
        &config,
        conversation_tokens,
        system_prompt_tokens,
        system_tools_tokens + additional_tools_tokens,
        &message_entries,
    );

    let mut sections = Vec::new();
    sections.push(ContextSection {
        title: "Tools".to_string(),
        command_hint: Some("/help".to_string()),
        groups: vec![
            ContextSectionGroup {
                title: Some("Core".to_string()),
                entries: builtin_tools,
            },
            ContextSectionGroup {
                title: Some("Additional".to_string()),
                entries: additional_tools,
            },
        ],
    });
    sections.push(ContextSection {
        title: "Memory Files".to_string(),
        command_hint: Some("/memory".to_string()),
        groups: vec![ContextSectionGroup {
            title: None,
            entries: memory_entries,
        }],
    });
    sections.push(ContextSection {
        title: "Skills Inventory".to_string(),
        command_hint: Some("/skills".to_string()),
        groups: skill_groups,
    });
    sections.push(ContextSection {
        title: "Custom Agents".to_string(),
        command_hint: None,
        groups: custom_agent_groups,
    });
    sections.push(ContextSection {
        title: "Message Breakdown".to_string(),
        command_hint: None,
        groups: vec![ContextSectionGroup {
            title: Some("Largest live messages".to_string()),
            entries: message_entries.into_iter().take(12).collect(),
        }],
    });

    ContextReport {
        model_name: config.model.clone(),
        context_window_size: config.context_window_size,
        system_prompt_tokens,
        system_tools_tokens,
        additional_tools_tokens,
        memory_files_tokens,
        conversation_tokens,
        sections,
        suggestions,
    }
}

fn build_message_entries(messages: &[Message]) -> Vec<ContextEntry> {
    let mut role_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut entries = Vec::new();
    for message in messages {
        let count = role_counts.entry(message.role.clone()).or_default();
        *count += 1;
        let label = format!("{} #{}", message.role, *count);
        entries.push(ContextEntry {
            label,
            tokens: estimate_message_tokens(message),
        });
    }
    entries.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.label.cmp(&b.label)));
    entries
}

fn estimate_message_tokens(message: &Message) -> usize {
    let chars: usize = message
        .content
        .iter()
        .map(|block| match block {
            crate::agent::ContentBlock::Text { text } => text.len(),
            crate::agent::ContentBlock::ToolUse { name, input, .. } => {
                name.len() + input.to_string().len()
            }
            crate::agent::ContentBlock::ToolResult { content, .. } => content.len(),
        })
        .sum();
    chars.div_ceil(4)
}

fn build_skill_groups(config: &crate::agent::Config) -> Vec<ContextSectionGroup> {
    let mut groups = Vec::new();
    if let Some(global_root) = crate::agent::Config::global_config_root() {
        groups.push(ContextSectionGroup {
            title: Some("User".to_string()),
            entries: load_skill_entries(&global_root.join("skills"), &global_root),
        });
    }
    groups.push(ContextSectionGroup {
        title: Some("Project".to_string()),
        entries: load_skill_entries(&config.skills_dir(), &config.workdir),
    });
    groups
}

fn load_skill_entries(path: &Path, base: &Path) -> Vec<ContextEntry> {
    let Ok(registry) = SkillRegistry::load_from_dir(path) else {
        return Vec::new();
    };
    let mut entries = registry
        .all()
        .into_iter()
        .map(|skill| ContextEntry {
            label: skill
                .source
                .as_ref()
                .map(|source| display_path(source, base))
                .unwrap_or_else(|| skill.name.clone()),
            tokens: estimate_tokens(&skill.prompt_template),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.label.cmp(&b.label)));
    entries
}

fn build_custom_agent_groups(config: &crate::agent::Config) -> Vec<ContextSectionGroup> {
    let mut groups = Vec::new();
    if let Some(global_root) = crate::agent::Config::global_config_root() {
        groups.push(ContextSectionGroup {
            title: Some("User".to_string()),
            entries: load_markdown_inventory(&global_root.join("agents"), &global_root),
        });
    }
    groups.push(ContextSectionGroup {
        title: Some("Project".to_string()),
        entries: load_markdown_inventory(&config.agents_dir(), &config.workdir),
    });
    groups
}

fn load_markdown_inventory(path: &Path, base: &Path) -> Vec<ContextEntry> {
    let Ok(mut entries) = collect_markdown_files(path) else {
        return Vec::new();
    };
    let mut inventory = entries
        .drain(..)
        .filter_map(|entry| {
            let content = fs::read_to_string(&entry).ok()?;
            Some(ContextEntry {
                label: display_path(&entry, base),
                tokens: estimate_tokens(&content),
            })
        })
        .collect::<Vec<_>>();
    inventory.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.label.cmp(&b.label)));
    inventory
}

fn collect_markdown_files(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            let nested = entry_path.join("AGENT.md");
            if nested.exists() {
                files.push(nested);
            }
            let nested = entry_path.join("agent.md");
            if nested.exists() {
                files.push(nested);
            }
        } else if entry_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("md"))
            .unwrap_or(false)
        {
            files.push(entry_path);
        }
    }
    files.sort();
    Ok(files)
}

fn build_suggestions(
    config: &crate::agent::Config,
    conversation_tokens: usize,
    system_prompt_tokens: usize,
    tool_tokens: usize,
    message_entries: &[ContextEntry],
) -> Vec<String> {
    let mut suggestions = Vec::new();
    if conversation_tokens > system_prompt_tokens + tool_tokens {
        suggestions.push(
            "Messages dominate the live window. `/compact` will recover the most space once older turns stop mattering."
                .to_string(),
        );
    }
    if let Some(largest_message) = message_entries.first() {
        let threshold = (config.context_window_size as usize).saturating_div(10);
        if largest_message.tokens >= threshold && threshold > 0 {
            suggestions.push(format!(
                "{} is unusually large. Summarizing or trimming that turn would save context fastest.",
                largest_message.label
            ));
        }
    }
    suggestions.push(
        "Skills and custom agents below are inventory only. They do not consume live context until you invoke them."
            .to_string(),
    );
    suggestions
}

fn display_path(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn estimate_tokens(content: &str) -> usize {
    content.len().div_ceil(4)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use tempfile::TempDir;
    use tokio::sync::RwLock;

    use super::{build_context_report, ContextSectionGroup};
    use crate::agent::{Agent, Config, ContentBlock, Message};
    use crate::benchmark::case::MockScript;
    use crate::benchmark::mock::BenchmarkMockClient;
    use crate::tools::tool_trait::Tool;

    struct ExtraTool;

    #[async_trait::async_trait]
    impl Tool for ExtraTool {
        fn name(&self) -> &'static str {
            "search_docs"
        }

        fn schema(&self) -> &'static serde_json::Value {
            Box::leak(Box::new(serde_json::json!({
                "name": "search_docs",
                "description": "Search docs",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" }
                    }
                }
            })))
        }

        async fn execute(&self, _input: serde_json::Value) -> crate::Result<String> {
            Ok("ok".to_string())
        }
    }

    fn group<'a>(groups: &'a [ContextSectionGroup], title: &str) -> &'a ContextSectionGroup {
        groups
            .iter()
            .find(|group| group.title.as_deref() == Some(title))
            .expect("missing section group")
    }

    #[test]
    fn build_context_report_collects_live_and_inventory_sections() {
        let temp = TempDir::new().expect("tempdir");
        let workdir = temp.path();
        fs::create_dir_all(workdir.join(".amadeus/skills/review")).expect("skills dir");
        fs::create_dir_all(workdir.join(".amadeus/agents/helper")).expect("agents dir");
        fs::create_dir_all(workdir.join(".amadeus")).expect("config dir");
        fs::write(
            workdir.join(".amadeus/context.md"),
            "Project guidance that should count as memory.",
        )
        .expect("context file");
        fs::write(
            workdir.join(".amadeus/skills/review/SKILL.md"),
            "---\nname: review\ndescription: Review code\n---\n\nUse {context} carefully.",
        )
        .expect("skill file");
        fs::write(
            workdir.join(".amadeus/agents/helper/AGENT.md"),
            "# Helper Agent\n\nFocused instructions.",
        )
        .expect("agent file");

        let history = Arc::new(RwLock::new(vec![
            Message::user("hello world"),
            Message::assistant(vec![
                ContentBlock::Text {
                    text: "assistant response".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "tool_1".to_string(),
                    name: "bash".to_string(),
                    input: serde_json::json!({"cmd": "pwd"}),
                },
            ]),
        ]));

        let client = BenchmarkMockClient::new(MockScript::default());
        let mut config = Config::default();
        config.workdir = workdir.to_path_buf();
        let agent = Agent::builder(client, Arc::new(config))
            .with_default_tools()
            .register_tool(Box::new(ExtraTool))
            .with_history(history)
            .build();

        let report = build_context_report(&agent);

        assert!(report.system_prompt_tokens > 0);
        assert!(report.system_tools_tokens > 0);
        assert!(report.additional_tools_tokens > 0);
        assert!(report.memory_files_tokens > 0);
        assert!(report.conversation_tokens > 0);
        assert!(report.used_tokens() > 0);

        let tools = report
            .sections
            .iter()
            .find(|section| section.title == "Tools")
            .expect("tools section");
        assert!(group(&tools.groups, "Core")
            .entries
            .iter()
            .any(|entry| entry.label == "bash"));
        assert!(group(&tools.groups, "Additional")
            .entries
            .iter()
            .any(|entry| entry.label == "search_docs"));

        let memory = report
            .sections
            .iter()
            .find(|section| section.title == "Memory Files")
            .expect("memory section");
        assert!(memory.groups[0]
            .entries
            .iter()
            .any(|entry| entry.label.contains(".amadeus/context.md")));

        let skills = report
            .sections
            .iter()
            .find(|section| section.title == "Skills Inventory")
            .expect("skills section");
        assert!(group(&skills.groups, "Project")
            .entries
            .iter()
            .any(|entry| entry.label.contains("skills/review/SKILL.md")));

        let agents = report
            .sections
            .iter()
            .find(|section| section.title == "Custom Agents")
            .expect("agents section");
        assert!(group(&agents.groups, "Project")
            .entries
            .iter()
            .any(|entry| entry.label.contains("agents/helper/AGENT.md")));

        assert!(report
            .suggestions
            .iter()
            .any(|line| line.contains("inventory only")));
    }
}
