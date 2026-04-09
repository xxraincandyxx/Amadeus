// @amadeus-header
// summary: Structured configuration models and hierarchical settings loading for core runtimes.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - type: crate::Provider
// - type: crate::Config
// - type: crate::ConfigError
// uses:
// - module: amadeus_prompts
// - module: amadeus_context
// - module: amadeus_compaction
// - module: amadeus_permissions
// - artifact: filesystem paths and files
// invariants:
// - Workspace settings override global settings and process env overrides both.
// side_effects:
// - Reads filesystem state and process environment variables.
// tests:
// - cmd: cargo test -p config
// @end-amadeus-header

//! Structured configuration loading for Amadeus runtimes.

use std::env;
use std::path::{Path, PathBuf};

use amadeus_compaction::CompactionConfig;
use amadeus_context::ProjectContext;
use amadeus_hooks::HookSource;
use amadeus_permissions::PermissionMode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

const DEFAULT_MODEL: &str = "claude-sonnet-4-5-20250929";
const DEFAULT_TIMEOUT_SECONDS: u64 = 300;
const DEFAULT_MAX_OUTPUT_BYTES: usize = 50_000;
const DEFAULT_CONTEXT_WINDOW_SIZE: u32 = 200_000;
const DEFAULT_COMPACT_THRESHOLD_PERCENT: u8 = 75;
const DEFAULT_COMPACT_PRESERVE_RECENT: usize = 6;
const DEFAULT_MAX_SUBAGENT_DEPTH: usize = 2;

pub type Result<T> = std::result::Result<T, ConfigError>;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Environment variable '{0}' not set")]
    MissingEnvVar(String),

    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provider {
    Anthropic,
    OpenAI,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookSettings {
    #[serde(default)]
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetrySettings {
    pub enabled: bool,
    pub jsonl_path: Option<PathBuf>,
}

impl Default for TelemetrySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            jsonl_path: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionSettings {
    pub mode: PermissionMode,
    #[serde(default)]
    pub rules: Vec<String>,
}

impl Default for PermissionSettings {
    fn default() -> Self {
        Self {
            mode: PermissionMode::WorkspaceWrite,
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub provider: Provider,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: String,
    pub workdir: PathBuf,
    pub timeout_seconds: u64,
    pub max_output_bytes: usize,
    pub blocked_commands: Vec<String>,
    pub session_log_dir: Option<PathBuf>,
    pub session_log_compress: bool,
    pub context_window_size: u32,
    pub auto_compact: bool,
    pub compact_threshold_percent: u8,
    pub compact_preserve_recent: usize,
    pub max_subagent_depth: usize,
    pub permission_mode: PermissionMode,
    #[serde(default)]
    pub hooks: HookSettings,
    #[serde(default)]
    pub telemetry: TelemetrySettings,
    #[serde(default)]
    pub permissions: PermissionSettings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: Provider::Anthropic,
            api_key: String::new(),
            base_url: None,
            model: DEFAULT_MODEL.to_string(),
            workdir: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            timeout_seconds: DEFAULT_TIMEOUT_SECONDS,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
            blocked_commands: vec!["rm -rf /".to_string()],
            session_log_dir: None,
            session_log_compress: false,
            context_window_size: DEFAULT_CONTEXT_WINDOW_SIZE,
            auto_compact: true,
            compact_threshold_percent: DEFAULT_COMPACT_THRESHOLD_PERCENT,
            compact_preserve_recent: DEFAULT_COMPACT_PRESERVE_RECENT,
            max_subagent_depth: DEFAULT_MAX_SUBAGENT_DEPTH,
            permission_mode: PermissionMode::WorkspaceWrite,
            hooks: HookSettings::default(),
            telemetry: TelemetrySettings::default(),
            permissions: PermissionSettings::default(),
        }
    }
}

impl Config {
    pub fn workspace_config_root(&self) -> PathBuf {
        self.workdir.join(".amadeus")
    }

    pub fn global_config_root() -> Option<PathBuf> {
        dirs::home_dir().map(|home| home.join(".amadeus"))
    }

    pub fn config_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if let Some(global_root) = Self::global_config_root() {
            roots.push(global_root);
        }
        roots.push(self.workspace_config_root());
        roots
    }

    pub fn workspace_settings_path(&self) -> PathBuf {
        self.workspace_config_root().join("settings.json")
    }

    pub fn global_settings_path() -> Option<PathBuf> {
        Self::global_config_root().map(|root| root.join("settings.json"))
    }

    pub fn workspace_local_settings_path(&self) -> PathBuf {
        self.workspace_config_root().join("settings.local.json")
    }

    pub fn workspace_hooks_path(&self) -> PathBuf {
        self.workspace_config_root().join("hook.json")
    }

    pub fn workspace_local_hooks_path(&self) -> PathBuf {
        self.workspace_config_root().join("hook.local.json")
    }

    pub fn global_hooks_path() -> Option<PathBuf> {
        Self::global_config_root().map(|root| root.join("hook.json"))
    }

    pub fn agents_dir(&self) -> PathBuf {
        self.workspace_config_root().join("agents")
    }

    pub fn skills_dir(&self) -> PathBuf {
        self.workspace_config_root().join("skills")
    }

    pub fn settings_layers(workdir: &Path) -> Vec<(PathBuf, HookSource)> {
        let mut layers = Vec::new();
        if let Some(global_root) = Self::global_config_root() {
            layers.push((global_root.join("settings.json"), HookSource::Global));
        }

        let workspace_root = workdir.join(".amadeus");
        layers.push((workspace_root.join("settings.json"), HookSource::Workspace));
        layers.push((
            workspace_root.join("settings.local.json"),
            HookSource::Local,
        ));
        layers
    }

    pub fn hook_paths(&self) -> Vec<(PathBuf, HookSource)> {
        let mut paths = Vec::new();

        if let Some(global_path) = Self::global_hooks_path() {
            paths.push((global_path, HookSource::Global));
        }

        paths.push((self.workspace_hooks_path(), HookSource::Workspace));
        paths.push((self.workspace_local_hooks_path(), HookSource::Local));

        for file in &self.hooks.files {
            let resolved = if file.is_absolute() {
                file.clone()
            } else {
                self.workspace_config_root().join(file)
            };
            paths.push((resolved, HookSource::Local));
        }

        let mut seen = std::collections::HashSet::new();
        paths
            .into_iter()
            .filter(|(path, _)| seen.insert(path.clone()))
            .collect()
    }

    pub fn load() -> Result<Self> {
        let workdir = env::current_dir()?;
        Self::load_with_hierarchy(&workdir)
    }

    pub fn load_for_assessment() -> Result<Self> {
        let workdir = env::current_dir()?;
        Self::load_with_hierarchy_internal(&workdir, false)
    }

    pub fn system_prompt(&self, include_sub_agent_tool: bool) -> String {
        let mut prompt = amadeus_prompts::render_system_prompt(
            &self.workdir.display().to_string(),
            include_sub_agent_tool,
        );

        if let Some(ctx) = ProjectContext::load(&self.workdir) {
            prompt.push_str(&ctx.to_prompt_section());
        }

        prompt
    }

    pub fn load_from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(ConfigError::Config(format!(
                "Config file not found: {}",
                path.display()
            )));
        }

        let content = std::fs::read_to_string(path).map_err(|e| {
            ConfigError::Config(format!(
                "Failed to read config file {}: {}",
                path.display(),
                e
            ))
        })?;

        let json: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
            ConfigError::Config(format!(
                "Invalid JSON in config file {}: {}",
                path.display(),
                e
            ))
        })?;

        let mut config = Config::default();
        config.apply_json(&json, path.parent());
        Ok(config)
    }

    pub fn load_with_hierarchy(workdir: &Path) -> Result<Self> {
        Self::load_with_hierarchy_internal(workdir, true)
    }

    pub fn merge(self, other: Self) -> Self {
        Self {
            provider: if other.provider != Provider::Anthropic || other.api_key != self.api_key {
                other.provider
            } else {
                self.provider
            },
            api_key: if !other.api_key.is_empty() {
                other.api_key
            } else {
                self.api_key
            },
            base_url: other.base_url.or(self.base_url),
            model: if other.model != DEFAULT_MODEL || self.model.is_empty() {
                other.model
            } else {
                self.model
            },
            workdir: if other.workdir != Path::new(".")
                && other.workdir != env::current_dir().unwrap_or_default()
            {
                other.workdir
            } else {
                self.workdir
            },
            timeout_seconds: if other.timeout_seconds != DEFAULT_TIMEOUT_SECONDS {
                other.timeout_seconds
            } else {
                self.timeout_seconds
            },
            max_output_bytes: if other.max_output_bytes != DEFAULT_MAX_OUTPUT_BYTES {
                other.max_output_bytes
            } else {
                self.max_output_bytes
            },
            blocked_commands: if !other.blocked_commands.is_empty()
                && other.blocked_commands != vec!["rm -rf /".to_string()]
            {
                other.blocked_commands
            } else {
                self.blocked_commands
            },
            session_log_dir: other.session_log_dir.or(self.session_log_dir),
            session_log_compress: other.session_log_compress || self.session_log_compress,
            context_window_size: if other.context_window_size != DEFAULT_CONTEXT_WINDOW_SIZE {
                other.context_window_size
            } else {
                self.context_window_size
            },
            auto_compact: if !other.auto_compact {
                other.auto_compact
            } else {
                self.auto_compact
            },
            compact_threshold_percent: if other.compact_threshold_percent
                != DEFAULT_COMPACT_THRESHOLD_PERCENT
            {
                other.compact_threshold_percent
            } else {
                self.compact_threshold_percent
            },
            compact_preserve_recent: if other.compact_preserve_recent
                != DEFAULT_COMPACT_PRESERVE_RECENT
            {
                other.compact_preserve_recent
            } else {
                self.compact_preserve_recent
            },
            max_subagent_depth: if other.max_subagent_depth != DEFAULT_MAX_SUBAGENT_DEPTH {
                other.max_subagent_depth
            } else {
                self.max_subagent_depth
            },
            permission_mode: if other.permission_mode != PermissionMode::WorkspaceWrite {
                other.permission_mode
            } else {
                self.permission_mode
            },
            hooks: if !other.hooks.files.is_empty() {
                other.hooks
            } else {
                self.hooks
            },
            telemetry: if other.telemetry != TelemetrySettings::default() {
                other.telemetry
            } else {
                self.telemetry
            },
            permissions: if other.permissions != PermissionSettings::default() {
                other.permissions
            } else {
                self.permissions
            },
        }
    }

    pub fn merge_env(mut self) -> Self {
        if let Ok(provider) = env::var("PROVIDER") {
            self.provider = parse_provider(&provider);
        }

        match &self.provider {
            Provider::Anthropic => {
                if let Ok(key) = env::var("ANTHROPIC_API_KEY") {
                    self.api_key = key;
                }
                if let Ok(url) = env::var("ANTHROPIC_BASE_URL") {
                    self.base_url = Some(url);
                }
            }
            Provider::OpenAI => {
                if let Ok(key) = env::var("OPENAI_API_KEY") {
                    self.api_key = key;
                }
                if let Ok(url) = env::var("OPENAI_BASE_URL") {
                    self.base_url = Some(url);
                }
            }
        }

        if let Ok(model) = env::var("MODEL_ID") {
            self.model = model;
        }

        if let Ok(max_bytes) = env::var("MAX_OUTPUT_BYTES") {
            if let Ok(bytes) = max_bytes.parse::<usize>() {
                self.max_output_bytes = bytes;
            }
        }

        if let Ok(blocked) = env::var("BLOCKED_COMMANDS") {
            self.blocked_commands = blocked
                .split(',')
                .map(|cmd| cmd.trim().to_string())
                .filter(|cmd| !cmd.is_empty())
                .collect();
        }

        if let Ok(log_dir) = env::var("SESSION_LOG_DIR") {
            self.session_log_dir = Some(PathBuf::from(log_dir));
        }

        if let Ok(compress) = env::var("SESSION_LOG_COMPRESS") {
            if let Ok(value) = compress.parse::<bool>() {
                self.session_log_compress = value;
            }
        }

        if let Ok(timeout) = env::var("TIMEOUT_SECONDS") {
            if let Ok(seconds) = timeout.parse::<u64>() {
                self.timeout_seconds = seconds;
            }
        }

        if let Ok(auto_compact) = env::var("AUTO_COMPACT") {
            if let Ok(value) = auto_compact.parse::<bool>() {
                self.auto_compact = value;
            }
        }

        if let Ok(threshold) = env::var("COMPACT_THRESHOLD_PERCENT") {
            if let Ok(value) = threshold.parse::<u8>() {
                self.compact_threshold_percent = value;
            }
        }

        if let Ok(preserve) = env::var("COMPACT_PRESERVE_RECENT") {
            if let Ok(value) = preserve.parse::<usize>() {
                self.compact_preserve_recent = value;
            }
        }

        if let Ok(max_depth) = env::var("MAX_SUBAGENT_DEPTH") {
            if let Ok(value) = max_depth.parse::<usize>() {
                self.max_subagent_depth = value;
            }
        }

        if let Ok(permission_mode) = env::var("PERMISSION_MODE") {
            if let Some(mode) = PermissionMode::parse(&permission_mode) {
                self.permission_mode = mode;
            }
        }

        self
    }

    pub fn to_compaction_config(&self) -> CompactionConfig {
        CompactionConfig {
            threshold_percent: self.compact_threshold_percent,
            target_percent: 40,
            preserve_recent: self.compact_preserve_recent,
            use_llm_summary: true,
            max_summary_chars: 2000,
            min_messages: 10,
            max_tool_result_chars: 5000,
        }
    }

    fn load_with_hierarchy_internal(workdir: &Path, validate_credentials: bool) -> Result<Self> {
        let mut config = Config {
            workdir: workdir.to_path_buf(),
            ..Config::default()
        };

        for (path, _) in Self::settings_layers(workdir) {
            if path.exists() {
                let content = std::fs::read_to_string(&path).map_err(|e| {
                    ConfigError::Config(format!(
                        "Failed to read config file {}: {}",
                        path.display(),
                        e
                    ))
                })?;
                let json: Value = serde_json::from_str(&content).map_err(|e| {
                    ConfigError::Config(format!(
                        "Invalid JSON in config file {}: {}",
                        path.display(),
                        e
                    ))
                })?;
                config.apply_json(&json, path.parent());
            }
        }

        config = config.merge_env();
        config.workdir = workdir.to_path_buf();
        if validate_credentials {
            config.validate_credentials()?;
        }
        Ok(config)
    }

    fn validate_credentials(&self) -> Result<()> {
        match self.provider {
            Provider::Anthropic if self.api_key.is_empty() => {
                Err(ConfigError::MissingEnvVar("ANTHROPIC_API_KEY".into()))
            }
            Provider::OpenAI if self.api_key.is_empty() => {
                Err(ConfigError::MissingEnvVar("OPENAI_API_KEY".into()))
            }
            _ => Ok(()),
        }
    }

    fn apply_json(&mut self, json: &Value, base_dir: Option<&Path>) {
        if let Some(provider) = json.get("provider").and_then(|v| v.as_str()) {
            self.provider = parse_provider(provider);
        }

        if let Some(api_key) = json.get("api_key").and_then(|v| v.as_str()) {
            self.api_key = api_key.to_string();
        }

        if let Some(base_url) = json.get("base_url").and_then(|v| v.as_str()) {
            self.base_url = Some(base_url.to_string());
        }

        if let Some(model) = json.get("model").and_then(|v| v.as_str()) {
            self.model = model.to_string();
        }

        if let Some(workdir) = json.get("workdir").and_then(|v| v.as_str()) {
            self.workdir = resolve_path(base_dir, workdir);
        }

        if let Some(timeout) = json.get("timeout_seconds").and_then(|v| v.as_u64()) {
            self.timeout_seconds = timeout;
        }

        if let Some(max_bytes) = json.get("max_output_bytes").and_then(|v| v.as_u64()) {
            self.max_output_bytes = max_bytes as usize;
        }

        if let Some(blocked) = json.get("blocked_commands").and_then(|v| v.as_array()) {
            self.blocked_commands = parse_string_list(blocked);
        }

        if let Some(log_dir) = json.get("session_log_dir").and_then(|v| v.as_str()) {
            self.session_log_dir = Some(resolve_path(base_dir, log_dir));
        }

        if matches!(json.get("session_log_dir"), Some(Value::Null)) {
            self.session_log_dir = None;
        }

        if let Some(compress) = json.get("session_log_compress").and_then(|v| v.as_bool()) {
            self.session_log_compress = compress;
        }

        if let Some(context_window_size) = json.get("context_window_size").and_then(|v| v.as_u64())
        {
            self.context_window_size = context_window_size as u32;
        }

        if let Some(auto_compact) = json.get("auto_compact").and_then(|v| v.as_bool()) {
            self.auto_compact = auto_compact;
        }

        if let Some(threshold) = json
            .get("compact_threshold_percent")
            .and_then(|v| v.as_u64())
        {
            self.compact_threshold_percent = threshold as u8;
        }

        if let Some(preserve_recent) = json.get("compact_preserve_recent").and_then(|v| v.as_u64())
        {
            self.compact_preserve_recent = preserve_recent as usize;
        }

        if let Some(max_subagent_depth) = json.get("max_subagent_depth").and_then(|v| v.as_u64()) {
            self.max_subagent_depth = max_subagent_depth as usize;
        }

        if let Some(permission_mode) = json.get("permission_mode").and_then(|v| v.as_str()) {
            if let Some(mode) = PermissionMode::parse(permission_mode) {
                self.permission_mode = mode;
                self.permissions.mode = mode;
            }
        }

        if let Some(hooks) = json.get("hooks").and_then(|v| v.as_object()) {
            if let Some(files) = hooks.get("files").and_then(|v| v.as_array()) {
                self.hooks.files = files
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|path| resolve_path(base_dir, path))
                    .collect();
            }
        }

        if let Some(telemetry) = json.get("telemetry").and_then(|v| v.as_object()) {
            if let Some(enabled) = telemetry.get("enabled").and_then(|v| v.as_bool()) {
                self.telemetry.enabled = enabled;
            }
            if telemetry.contains_key("jsonl_path") {
                self.telemetry.jsonl_path = telemetry
                    .get("jsonl_path")
                    .and_then(|v| v.as_str())
                    .map(|path| resolve_path(base_dir, path));
            }
        }

        if let Some(permissions) = json.get("permissions").and_then(|v| v.as_object()) {
            if let Some(mode) = permissions
                .get("mode")
                .and_then(|v| v.as_str())
                .and_then(PermissionMode::parse)
            {
                self.permission_mode = mode;
                self.permissions.mode = mode;
            }
            if let Some(rules) = permissions.get("rules").and_then(|v| v.as_array()) {
                self.permissions.rules = parse_string_list(rules);
            }
        }
    }
}

fn parse_provider(input: &str) -> Provider {
    match input.trim().to_ascii_lowercase().as_str() {
        "openai" => Provider::OpenAI,
        _ => Provider::Anthropic,
    }
}

fn resolve_path(base_dir: Option<&Path>, input: &str) -> PathBuf {
    let path = PathBuf::from(input);
    if path.is_absolute() {
        path
    } else if let Some(base_dir) = base_dir {
        base_dir.join(path)
    } else {
        path
    }
}

fn parse_string_list(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(|v| v.as_str().map(ToString::to_string))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::*;
    use tempfile::tempdir;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env test lock poisoned")
    }

    fn restore_env(key: &str, value: Option<String>) {
        match value {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
    }

    #[test]
    fn load_with_hierarchy_prefers_workspace_settings() {
        let _guard = env_lock();
        let temp = tempdir().unwrap();
        let workdir = temp.path().join("workspace");
        let workspace_root = workdir.join(".amadeus");
        std::fs::create_dir_all(&workspace_root).unwrap();

        let provider = env::var("PROVIDER").ok();
        let home = env::var("HOME").ok();

        env::remove_var("PROVIDER");

        let fake_home = temp.path().join("home");
        let global_root = fake_home.join(".amadeus");
        std::fs::create_dir_all(&global_root).unwrap();
        std::fs::write(
            global_root.join("settings.json"),
            r#"{"model":"global-model","timeout_seconds":120,"api_key":"global-key"}"#,
        )
        .unwrap();
        std::fs::write(
            workspace_root.join("settings.json"),
            r#"{"model":"workspace-model","timeout_seconds":45,"api_key":"workspace-key"}"#,
        )
        .unwrap();
        env::set_var("HOME", &fake_home);

        let config = Config::load_with_hierarchy(&workdir).unwrap();

        assert_eq!(config.model, "workspace-model");
        assert_eq!(config.timeout_seconds, 45);
        assert_eq!(
            config.workspace_settings_path(),
            workspace_root.join("settings.json")
        );
        assert_eq!(
            config.workspace_hooks_path(),
            workspace_root.join("hook.json")
        );
        assert_eq!(config.agents_dir(), workspace_root.join("agents"));
        assert_eq!(config.skills_dir(), workspace_root.join("skills"));

        restore_env("PROVIDER", provider);
        restore_env("HOME", home);
    }

    #[test]
    fn load_with_hierarchy_ignores_workspace_env_file() {
        let _guard = env_lock();
        let temp = tempdir().unwrap();
        let workdir = temp.path().join("workspace");
        let workspace_root = workdir.join(".amadeus");
        std::fs::create_dir_all(&workspace_root).unwrap();
        std::fs::write(workspace_root.join("env"), "MODEL_ID=env-model\n").unwrap();

        let provider = env::var("PROVIDER").ok();
        let anthropic_key = env::var("ANTHROPIC_API_KEY").ok();
        let model = env::var("MODEL_ID").ok();

        env::remove_var("PROVIDER");
        env::set_var("ANTHROPIC_API_KEY", "env-test-key");
        env::remove_var("MODEL_ID");

        let config = Config::load_with_hierarchy(&workdir).unwrap();

        assert_eq!(config.model, DEFAULT_MODEL);

        restore_env("PROVIDER", provider);
        restore_env("ANTHROPIC_API_KEY", anthropic_key);
        restore_env("MODEL_ID", model);
    }

    #[test]
    fn load_from_file_reads_permission_mode() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("settings.json");
        std::fs::write(&path, r#"{"api_key":"x","permission_mode":"read-only"}"#).unwrap();

        let config = Config::load_from_file(&path).unwrap();

        assert_eq!(config.permission_mode, PermissionMode::ReadOnly);
    }

    #[test]
    fn load_with_hierarchy_prefers_local_settings_layer() {
        let _guard = env_lock();
        let temp = tempdir().unwrap();
        let workdir = temp.path().join("workspace");
        let workspace_root = workdir.join(".amadeus");
        std::fs::create_dir_all(&workspace_root).unwrap();

        let provider = env::var("PROVIDER").ok();
        let anthropic_key = env::var("ANTHROPIC_API_KEY").ok();
        let home = env::var("HOME").ok();

        env::remove_var("PROVIDER");
        env::set_var("ANTHROPIC_API_KEY", "layer-key");

        let fake_home = temp.path().join("home");
        std::fs::create_dir_all(fake_home.join(".amadeus")).unwrap();
        env::set_var("HOME", &fake_home);

        std::fs::write(
            workspace_root.join("settings.json"),
            r#"{"model":"workspace-model","timeout_seconds":45}"#,
        )
        .unwrap();
        std::fs::write(
            workspace_root.join("settings.local.json"),
            r#"{"model":"local-model","timeout_seconds":12}"#,
        )
        .unwrap();

        let config = Config::load_with_hierarchy(&workdir).unwrap();
        assert_eq!(config.model, "local-model");
        assert_eq!(config.timeout_seconds, 12);
        assert_eq!(
            config.workspace_local_settings_path(),
            workspace_root.join("settings.local.json")
        );

        restore_env("PROVIDER", provider);
        restore_env("ANTHROPIC_API_KEY", anthropic_key);
        restore_env("HOME", home);
    }

    #[test]
    fn load_from_file_reads_typed_sections() {
        let temp = tempdir().unwrap();
        let config_dir = temp.path().join(".amadeus");
        std::fs::create_dir_all(&config_dir).unwrap();
        let path = config_dir.join("settings.json");
        std::fs::write(
            &path,
            r#"{
                "api_key":"x",
                "permissions":{"mode":"prompt","rules":["bash(git:*)"]},
                "telemetry":{"enabled":true,"jsonl_path":"logs/telemetry.jsonl"},
                "hooks":{"files":["custom-hooks.json"]}
            }"#,
        )
        .unwrap();

        let config = Config::load_from_file(&path).unwrap();

        assert_eq!(config.permission_mode, PermissionMode::Prompt);
        assert_eq!(config.permissions.mode, PermissionMode::Prompt);
        assert_eq!(config.permissions.rules, vec!["bash(git:*)".to_string()]);
        assert!(config.telemetry.enabled);
        assert_eq!(
            config.telemetry.jsonl_path,
            Some(config_dir.join("logs/telemetry.jsonl"))
        );
        assert_eq!(
            config.hooks.files,
            vec![config_dir.join("custom-hooks.json")]
        );
    }

    #[test]
    fn hook_paths_include_layered_defaults_and_custom_files() {
        let temp = tempdir().unwrap();
        let workdir = temp.path().join("workspace");
        let mut config = Config {
            workdir: workdir.clone(),
            ..Config::default()
        };
        config.hooks.files = vec![PathBuf::from("custom.json")];

        let paths = config.hook_paths();
        assert!(paths
            .iter()
            .any(|(path, source)| *source == HookSource::Workspace
                && *path == workdir.join(".amadeus/hook.json")));
        assert!(paths
            .iter()
            .any(|(path, source)| *source == HookSource::Local
                && *path == workdir.join(".amadeus/hook.local.json")));
        assert!(paths
            .iter()
            .any(|(path, source)| *source == HookSource::Local
                && *path == workdir.join(".amadeus/custom.json")));
    }
}
