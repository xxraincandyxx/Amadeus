// @amadeus-header
// summary: Reusable skill templates and registry shared across runtime surfaces.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - type: crate::Skill
// - type: crate::SkillError
// - type: crate::SkillRegistry
// uses:
// - protocol: serde serialization
// - artifact: filesystem paths and files
// invariants:
// - Skill parsing and registry loading stay independent from runtime config resolution.
// side_effects:
// - Reads filesystem state when loading skills.
// tests:
// - cmd: cargo test -p skills
// @end-amadeus-header

//! Reusable skill templates and registry loading.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub prompt_template: String,
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,
    #[serde(skip)]
    pub source: Option<PathBuf>,
}

impl Skill {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        prompt_template: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            prompt_template: prompt_template.into(),
            allowed_tools: None,
            source: None,
        }
    }

    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = Some(tools);
        self
    }

    pub fn load(path: &Path) -> Result<Self, SkillError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| SkillError::LoadFailed(path.display().to_string(), e.to_string()))?;
        Self::parse(&content, Some(path.to_path_buf()))
    }

    pub fn parse(content: &str, source: Option<PathBuf>) -> Result<Self, SkillError> {
        if !content.starts_with("---") {
            return Err(SkillError::MissingFrontmatter);
        }

        let end_idx = content[3..]
            .find("\n---")
            .ok_or(SkillError::MissingFrontmatter)?
            + 3;

        let frontmatter = &content[4..end_idx];
        let prompt_template = content[end_idx + 4..].trim().to_string();

        let mut name = String::new();
        let mut description = String::new();
        let mut allowed_tools: Option<Vec<String>> = None;

        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("name:") {
                name = value.trim().trim_matches('"').to_string();
            } else if let Some(value) = line.strip_prefix("description:") {
                description = value.trim().trim_matches('"').to_string();
            } else if line.starts_with("allowed_tools:") {
                allowed_tools = Some(Vec::new());
            } else if line.starts_with("- ") && allowed_tools.is_some() {
                if let Some(ref mut tools) = allowed_tools {
                    tools.push(line[2..].trim().trim_matches('"').to_string());
                }
            }
        }

        if name.is_empty() {
            return Err(SkillError::MissingField("name".to_string()));
        }

        Ok(Self {
            name,
            description,
            prompt_template,
            allowed_tools,
            source,
        })
    }

    pub fn render(&self, context: &str) -> String {
        self.prompt_template.replace("{context}", context)
    }

    pub fn is_tool_allowed(&self, tool: &str) -> bool {
        match &self.allowed_tools {
            Some(tools) => tools.iter().any(|t| t == tool),
            None => true,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("Failed to load skill from {0}: {1}")]
    LoadFailed(String, String),
    #[error("Missing YAML frontmatter in skill file")]
    MissingFrontmatter,
    #[error("Missing required field: {0}")]
    MissingField(String),
    #[error("Skill not found: {0}")]
    NotFound(String),
    #[error("Invalid skill format: {0}")]
    InvalidFormat(String),
}

#[derive(Debug, Clone, Default)]
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    pub fn load_from_dir(path: &Path) -> Result<Self, SkillError> {
        let mut registry = Self::new();

        if !path.exists() {
            return Ok(registry);
        }

        for skill_path in collect_skill_paths(path)? {
            if let Ok(skill) = Skill::load(&skill_path) {
                tracing::info!(
                    skill = %skill.name,
                    path = %skill_path.display(),
                    "Loaded skill"
                );
                registry.skills.insert(skill.name.clone(), skill);
            } else {
                tracing::warn!(
                    path = %skill_path.display(),
                    "Failed to load skill file"
                );
            }
        }

        Ok(registry)
    }

    pub fn load_with_roots(
        global_root: Option<&Path>,
        workspace_root: &Path,
    ) -> Result<Self, SkillError> {
        let mut registry = Self::new();
        if let Some(global_root) = global_root {
            registry.merge(Self::load_from_dir(global_root)?);
        }
        registry.merge(Self::load_from_dir(workspace_root)?);
        Ok(registry)
    }

    pub fn register(&mut self, skill: Skill) {
        self.skills.insert(skill.name.clone(), skill);
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    pub fn list(&self) -> Vec<&str> {
        self.skills.keys().map(|s| s.as_str()).collect()
    }

    pub fn all(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.skills.contains_key(name)
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn into_skills(self) -> Vec<Skill> {
        self.skills.into_values().collect()
    }

    pub fn merge(&mut self, other: Self) {
        self.skills.extend(other.skills);
    }
}

fn collect_skill_paths(path: &Path) -> Result<Vec<PathBuf>, SkillError> {
    let entries = std::fs::read_dir(path)
        .map_err(|e| SkillError::LoadFailed(path.display().to_string(), e.to_string()))?;
    let mut paths = Vec::new();

    for entry in entries.flatten() {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            let nested = entry_path.join("SKILL.md");
            if nested.exists() {
                paths.push(nested);
            }
        } else if entry_path.extension().map(|e| e == "md").unwrap_or(false) {
            paths.push(entry_path);
        }
    }

    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

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
    fn test_registry_load_from_nested_skill_dir() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("nested");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: nested-example\ndescription: Nested skill\n---\n\nDo something with: {context}\n",
        )
        .unwrap();

        let registry = SkillRegistry::load_from_dir(temp.path()).unwrap();
        assert!(registry.contains("nested-example"));
    }

    #[test]
    fn test_registry_load_with_roots_merges_global_and_workspace() {
        let temp = TempDir::new().unwrap();
        let global_dir = temp.path().join("global");
        let workspace_dir = temp.path().join("workspace");
        fs::create_dir_all(&global_dir).unwrap();
        fs::create_dir_all(&workspace_dir).unwrap();
        fs::write(
            global_dir.join("global.md"),
            "---\nname: global\ndescription: Global skill\n---\n\nglobal {context}\n",
        )
        .unwrap();
        fs::write(
            workspace_dir.join("workspace.md"),
            "---\nname: workspace\ndescription: Workspace skill\n---\n\nworkspace {context}\n",
        )
        .unwrap();

        let registry = SkillRegistry::load_with_roots(Some(&global_dir), &workspace_dir).unwrap();
        assert!(registry.contains("global"));
        assert!(registry.contains("workspace"));
    }
}
