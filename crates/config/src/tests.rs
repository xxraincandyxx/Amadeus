// @amadeus-header
// summary: Unit tests for configuration defaults, parsing, merging, paths, prompts, and layered loading.
// layer: test
// status: test-only
// feature_flags: none
// provides:
// - module: crate::config::tests
// uses:
// - type: crate::Config
// - artifact: filesystem paths and files
// invariants:
// - Tests preserve the documented global, workspace, and local precedence order.
// side_effects:
// - Mutates process environment variables under a shared test lock.
// - Reads and writes temporary filesystem state.
// tests:
// - cmd: cargo test -p config
// @end-amadeus-header

use std::sync::{Mutex, OnceLock};

use super::*;
use tempfile::tempdir;

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env test lock poisoned")
}

#[test]
fn compaction_prompt_settings_support_inline_and_relative_file_values() {
    let temp = tempdir().unwrap();
    let settings_dir = temp.path().join(".amadeus");
    std::fs::create_dir_all(&settings_dir).unwrap();
    let path = settings_dir.join("settings.json");
    std::fs::write(
        &path,
        r#"{"compact_prompt":"inline prompt","compact_prompt_file":"prompts/compact.md"}"#,
    )
    .unwrap();

    let config = Config::load_from_file(&path).unwrap();

    assert_eq!(config.compact_prompt.as_deref(), Some("inline prompt"));
    assert_eq!(
        config.compact_prompt_file,
        Some(settings_dir.join("prompts/compact.md"))
    );
    let compaction = config.to_compaction_config();
    assert_eq!(compaction.prompt.as_deref(), Some("inline prompt"));
    assert_eq!(
        compaction.prompt_file,
        Some(settings_dir.join("prompts/compact.md"))
    );
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

    let home = env::var("HOME").ok();

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
    assert_eq!(
        config.agent_roots(),
        vec![
            ("User".to_string(), global_root.join("agents")),
            ("Project".to_string(), workspace_root.join("agents"))
        ]
    );
    assert_eq!(
        config.skill_roots(),
        vec![
            ("User".to_string(), global_root.join("skills")),
            ("Project".to_string(), workspace_root.join("skills"))
        ]
    );

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
    std::fs::write(
        workspace_root.join("settings.json"),
        r#"{"provider":"anthropic","api_key":"settings-key"}"#,
    )
    .unwrap();

    let config = Config::load_with_hierarchy(&workdir).unwrap();

    assert_eq!(config.model, DEFAULT_MODEL);
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
fn load_from_file_reads_tool_profiles() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("settings.json");
    std::fs::write(
        &path,
        r#"{
            "api_key":"x",
            "tools":{
                "default_profile":"planner",
                "profiles":{
                    "planner":{
                        "enabled_packs":["filesystem","planning"],
                        "disabled_tools":["write_file"],
                        "allow_aliases":false,
                        "include_mcp":false,
                        "include_control_plane":true,
                        "model_permission_mode":"read-only"
                    }
                }
            }
        }"#,
    )
    .unwrap();

    let config = Config::load_from_file(&path).unwrap();

    assert_eq!(config.tools.default_profile, "planner");
    let planner = config
        .tools
        .profiles
        .get("planner")
        .expect("planner profile should exist");
    assert_eq!(
        planner.enabled_packs,
        vec!["filesystem".to_string(), "planning".to_string()]
    );
    assert_eq!(planner.disabled_tools, vec!["write_file".to_string()]);
    assert!(!planner.allow_aliases);
    assert!(!planner.include_mcp);
    assert_eq!(
        planner.model_permission_mode,
        Some(PermissionMode::ReadOnly)
    );
}

#[test]
fn load_from_file_reads_prompt_profiles_and_tool_overrides() {
    let temp = tempdir().unwrap();
    let config_dir = temp.path().join(".amadeus");
    std::fs::create_dir_all(config_dir.join("prompts")).unwrap();
    std::fs::write(config_dir.join("prompts/team.md"), "Prefer short replies.").unwrap();
    let path = config_dir.join("settings.json");
    std::fs::write(
        &path,
        r#"{
            "api_key":"x",
            "prompts":{
                "active_profile":"team",
                "profiles":{
                    "team":{
                        "mode":"prepend",
                        "include_project_context":false,
                        "sections":[{"id":"style","title":"Style","content":"Be concise."}],
                        "files":["prompts/team.md"]
                    }
                }
            },
            "tools":{
                "overrides":{
                    "bash":{
                        "description":"Run approved workspace shell commands.",
                        "aliases":["shell"],
                        "tags":["custom"],
                        "prompt_approval":true,
                        "visible_in_modes":["workspace-write"],
                        "required_permission":"workspace-write"
                    }
                }
            }
        }"#,
    )
    .unwrap();

    let config = Config::load_from_file(&path).unwrap();

    assert_eq!(config.prompts.active_profile, "team");
    let profile = config.prompts.profiles.get("team").unwrap();
    assert_eq!(profile.mode, PromptMergeMode::Prepend);
    assert!(!profile.include_project_context);
    assert_eq!(profile.sections[0].id, "style");
    assert_eq!(profile.files, vec![config_dir.join("prompts/team.md")]);

    let bash = config.tools.overrides.get("bash").unwrap();
    assert_eq!(
        bash.description.as_deref(),
        Some("Run approved workspace shell commands.")
    );
    assert_eq!(bash.aliases, vec!["shell".to_string()]);
    assert_eq!(bash.tags, vec!["custom".to_string()]);
    assert_eq!(bash.prompt_approval, Some(true));
    assert_eq!(bash.visible_in_modes, vec![PermissionMode::WorkspaceWrite]);
    assert_eq!(
        bash.required_permission,
        Some(PermissionMode::WorkspaceWrite)
    );
}

#[test]
fn configured_system_prompt_preserves_default_and_supports_append() {
    let temp = tempdir().unwrap();
    let mut config = Config {
        workdir: temp.path().to_path_buf(),
        ..Config::default()
    };
    let default_prompt = config.system_prompt(false);

    config.prompts.profiles.insert(
        "default".to_string(),
        PromptProfileConfig {
            sections: vec![PromptSectionConfig {
                id: "style".to_string(),
                title: Some("Style".to_string()),
                content: "Prefer compact explanations.".to_string(),
            }],
            ..PromptProfileConfig::default()
        },
    );

    let configured_prompt = config.system_prompt(false);
    assert!(configured_prompt.starts_with(&default_prompt));
    assert!(configured_prompt.contains("## Style"));
    assert!(configured_prompt.contains("Prefer compact explanations."));
}

#[test]
fn prompt_profile_can_replace_and_remove_builtin_sections() {
    let mut config = Config {
        workdir: PathBuf::from("/tmp/amadeus-prompt-profile"),
        ..Config::default()
    };
    config.prompts.active_profile = "focused".to_string();
    config.prompts.profiles.insert(
        "focused".to_string(),
        PromptProfileConfig {
            builtin_sections: HashMap::from([
                (
                    "core_loop".to_string(),
                    Some("Custom agent identity.".to_string()),
                ),
                ("task_management".to_string(), None),
            ]),
            ..PromptProfileConfig::default()
        },
    );

    let prompt = config.system_prompt(false);

    assert!(prompt.contains("Custom agent identity."));
    assert!(!prompt.contains("You are a CLI agent"));
    assert!(!prompt.contains("Use todo to track multi-step tasks"));
    assert!(prompt.contains("Never commit secrets"));
}

#[test]
fn builtin_section_settings_load_from_json() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("settings.json");
    std::fs::write(
        &path,
        r#"{
            "prompts": {
                "active_profile": "focused",
                "profiles": {
                    "focused": {
                        "builtin_sections": {
                            "core_loop": "Custom identity.",
                            "task_management": null
                        }
                    }
                }
            }
        }"#,
    )
    .unwrap();

    let config = Config::load_from_file(&path).unwrap();
    let profile = config.prompt_profile().unwrap();

    assert_eq!(
        profile
            .builtin_sections
            .get("core_loop")
            .and_then(Option::as_deref),
        Some("Custom identity.")
    );
    assert_eq!(profile.builtin_sections.get("task_management"), Some(&None));
}

#[test]
fn load_with_hierarchy_prefers_local_settings_layer() {
    let _guard = env_lock();
    let temp = tempdir().unwrap();
    let workdir = temp.path().join("workspace");
    let workspace_root = workdir.join(".amadeus");
    std::fs::create_dir_all(&workspace_root).unwrap();

    let home = env::var("HOME").ok();

    let fake_home = temp.path().join("home");
    std::fs::create_dir_all(fake_home.join(".amadeus")).unwrap();
    env::set_var("HOME", &fake_home);

    std::fs::write(
        workspace_root.join("settings.json"),
        r#"{"api_key":"workspace-key","model":"workspace-model","timeout_seconds":45}"#,
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
            "permissions":{
                "mode":"prompt",
                "rules":["allow:bash(git:*)"],
                "allow":["tool(read_file)"],
                "additionalDirectories":["../shared"]
            },
            "telemetry":{"enabled":true,"jsonl_path":"logs/telemetry.jsonl"},
            "hooks":{"files":["custom-hooks.json"]}
        }"#,
    )
    .unwrap();

    let config = Config::load_from_file(&path).unwrap();

    assert_eq!(config.permission_mode, PermissionMode::Prompt);
    assert_eq!(config.permissions.mode, PermissionMode::Prompt);
    assert_eq!(
        config.permissions.rules,
        vec!["allow:bash(git:*)".to_string()]
    );
    assert_eq!(
        config.permissions.allow,
        vec!["tool(read_file)".to_string()]
    );
    assert_eq!(
        config.permissions.additional_directories,
        vec![temp.path().join("shared")]
    );
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

#[test]
fn layered_settings_merge_arrays_across_scopes() {
    let _guard = env_lock();
    let temp = tempdir().unwrap();
    let workdir = temp.path().join("workspace");
    let workspace_root = workdir.join(".amadeus");
    std::fs::create_dir_all(&workspace_root).unwrap();

    let home = env::var("HOME").ok();

    let fake_home = temp.path().join("home");
    let global_root = fake_home.join(".amadeus");
    std::fs::create_dir_all(&global_root).unwrap();
    env::set_var("HOME", &fake_home);

    std::fs::write(
        global_root.join("settings.json"),
        r#"{
            "api_key":"global-key",
            "hooks":{"files":["global-hooks.json"]},
            "permissions":{"allow":["tool(read_file)"]}
        }"#,
    )
    .unwrap();
    std::fs::write(
        workspace_root.join("settings.json"),
        r#"{
            "hooks":{"files":["project-hooks.json"]},
            "permissions":{"ask":["bash(git:*)"]}
        }"#,
    )
    .unwrap();
    std::fs::write(
        workspace_root.join("settings.local.json"),
        r#"{
            "hooks":{"files":["local-hooks.json"]},
            "permissions":{"deny":["tool(write_file)"]}
        }"#,
    )
    .unwrap();

    let config = Config::load_with_hierarchy(&workdir).unwrap();
    assert_eq!(
        config.hooks.files,
        vec![
            global_root.join("global-hooks.json"),
            workspace_root.join("project-hooks.json"),
            workspace_root.join("local-hooks.json")
        ]
    );
    assert_eq!(
        config.permissions.allow,
        vec!["tool(read_file)".to_string()]
    );
    assert_eq!(config.permissions.ask, vec!["bash(git:*)".to_string()]);
    assert_eq!(
        config.permissions.deny,
        vec!["tool(write_file)".to_string()]
    );

    restore_env("HOME", home);
}

#[test]
fn language_parses_supported_names_and_defaults_to_english() {
    assert_eq!(Config::default().tui.language, Language::English);
    assert_eq!(Language::parse_str("en"), Some(Language::English));
    assert_eq!(
        Language::parse_str("zh-CN"),
        Some(Language::ChineseSimplified)
    );
    assert_eq!(
        Language::parse_str("simplified-chinese"),
        Some(Language::ChineseSimplified)
    );
    assert!(Language::parse_str("fr").is_none());
}

#[test]
fn hierarchy_loads_tui_language() {
    let _env = env_lock();
    let temp = tempfile::tempdir().expect("tempdir");
    let config_root = temp.path().join(".amadeus");
    std::fs::create_dir_all(&config_root).expect("create config root");
    std::fs::write(
        config_root.join("settings.json"),
        r#"{"tui":{"language":"zh-CN"}}"#,
    )
    .expect("write settings");

    let home = env::var("HOME").ok();
    env::set_var("HOME", temp.path());

    let config = Config::load_with_hierarchy_internal(temp.path(), false).expect("load config");
    assert_eq!(config.tui.language, Language::ChineseSimplified);

    restore_env("HOME", home);
}
