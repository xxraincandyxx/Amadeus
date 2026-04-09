// @amadeus-header
// summary: Compatibility layer for the skills subsystem and config-aware loaders.
// layer: infra
// status: active
// feature_flags: none
// provides:
// - module: crate::skills
// - module: crate::skills::registry
// - type: crate::skills::Skill
// - type: crate::skills::SkillError
// - type: crate::skills::registry::SkillRegistry
// - fn: crate::skills::load_for_config
// uses:
// - module: amadeus_skills
// - module: crate::agent::config::Config
// invariants:
// - Skill parsing stays transport-agnostic while runtime config resolution stays in core.
// side_effects:
// - Reads filesystem state when loading config-resolved skill roots.
// tests:
// - tests/mod.rs
// @end-amadeus-header

//! Skills compatibility layer and config-aware loading helpers.

use crate::agent::config::Config;

pub mod registry {
    pub use amadeus_skills::SkillRegistry;
}

pub use amadeus_skills::{Skill, SkillError};

pub fn load_for_config(config: &Config) -> Result<registry::SkillRegistry, SkillError> {
    let global_skills = Config::global_config_root()
        .as_ref()
        .map(|root| root.join("skills"));
    registry::SkillRegistry::load_with_roots(global_skills.as_deref(), &config.skills_dir())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{load_for_config, Skill};

    #[test]
    fn test_skill_parse() {
        let content = r#"---
name: test-skill
description: A test skill
allowed_tools:
  - read_file
  - glob
---

This is the prompt template.
Context: {context}
"#;

        let skill = Skill::parse(content, None).unwrap();
        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.description, "A test skill");
        assert_eq!(
            skill.allowed_tools,
            Some(vec!["read_file".to_string(), "glob".to_string()])
        );
        assert!(skill.prompt_template.contains("Context:"));
    }

    #[test]
    fn test_skill_render() {
        let skill = Skill::new("test", "Test skill", "Hello {context}!");
        let rendered = skill.render("World");
        assert_eq!(rendered, "Hello World!");
    }

    #[test]
    fn test_tool_allowed() {
        let skill =
            Skill::new("test", "Test", "prompt").with_allowed_tools(vec!["read_file".to_string()]);

        assert!(skill.is_tool_allowed("read_file"));
        assert!(!skill.is_tool_allowed("bash"));
    }

    #[test]
    fn test_all_tools_allowed_when_unrestricted() {
        let skill = Skill::new("test", "Test", "prompt");
        assert!(skill.is_tool_allowed("bash"));
        assert!(skill.is_tool_allowed("read_file"));
    }

    #[test]
    fn load_for_config_merges_global_and_workspace_skills() {
        let temp = TempDir::new().unwrap();
        let fake_home = temp.path().join("home");
        let global_skills = fake_home.join(".amadeus/skills");
        let workspace = temp.path().join("workspace");
        let workspace_skills = workspace.join(".amadeus/skills");
        fs::create_dir_all(&global_skills).unwrap();
        fs::create_dir_all(&workspace_skills).unwrap();
        fs::write(
            global_skills.join("global.md"),
            "---\nname: global\ndescription: Global skill\n---\n\nglobal {context}\n",
        )
        .unwrap();
        fs::write(
            workspace_skills.join("workspace.md"),
            "---\nname: workspace\ndescription: Workspace skill\n---\n\nworkspace {context}\n",
        )
        .unwrap();

        let home = std::env::var("HOME").ok();
        std::env::set_var("HOME", &fake_home);

        let mut config = crate::agent::config::Config::default();
        config.workdir = workspace;
        let registry = load_for_config(&config).unwrap();

        match home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }

        assert!(registry.contains("global"));
        assert!(registry.contains("workspace"));
    }
}
