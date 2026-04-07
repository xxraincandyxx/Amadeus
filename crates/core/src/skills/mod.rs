// @amadeus-header
// summary: Module root for the skills subsystem and its exports.
// layer: infra
// status: active
// feature_flags: none
// provides:
// - module: crate::skills
// - type: crate::skills::Skill
// - type: crate::skills::SkillError
// uses:
// - protocol: serde serialization
// - artifact: filesystem paths and files
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects:
// - Reads or writes filesystem state.
// tests:
// - tests/mod.rs
// @end-amadeus-header

//! # Skills System
//!
//! Skills are reusable prompt templates that can be loaded from files.
//!
//! ## Skill File Format
//!
//! Skills are stored as markdown files with YAML frontmatter:
//!
//! ```markdown
//! ---
//! name: code-review
//! description: Review code for quality and issues
//! allowed_tools:
//!   - read_file
//!   - glob
//!   - grep
//! ---
//!
//! You are reviewing code. Focus on:
//! - Code quality
//! - Potential bugs
//! - Security issues
//! - Performance concerns
//!
//! Context: {context}
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use amadeus::skills::{Skill, SkillRegistry};
//!
//! let registry = SkillRegistry::load_from_dir(Path::new(".amadeus/skills"))?;
//! let skill = registry.get("code-review").unwrap();
//! let prompt = skill.render("Review the main.rs file");
//! ```

pub mod registry;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A skill is a reusable prompt template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Unique name for the skill.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// The prompt template with {context} placeholder.
    pub prompt_template: String,
    /// Optional list of allowed tools for this skill.
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,
    /// Source file path (if loaded from file).
    #[serde(skip)]
    pub source: Option<PathBuf>,
}

impl Skill {
    /// Create a new skill.
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

    /// Set allowed tools for this skill.
    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = Some(tools);
        self
    }

    /// Load a skill from a markdown file with YAML frontmatter.
    ///
    /// The file format is:
    /// ```markdown
    /// ---
    /// name: skill-name
    /// description: Skill description
    /// allowed_tools:
    ///   - tool1
    ///   - tool2
    /// ---
    ///
    /// Prompt template content here.
    /// Use {context} for variable substitution.
    /// ```
    pub fn load(path: &Path) -> Result<Self, SkillError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| SkillError::LoadFailed(path.display().to_string(), e.to_string()))?;

        Self::parse(&content, Some(path.to_path_buf()))
    }

    /// Parse a skill from content string.
    pub fn parse(content: &str, source: Option<PathBuf>) -> Result<Self, SkillError> {
        // Check for YAML frontmatter
        if !content.starts_with("---") {
            return Err(SkillError::MissingFrontmatter);
        }

        // Find the end of frontmatter
        let end_idx = content[3..]
            .find("\n---")
            .ok_or(SkillError::MissingFrontmatter)?
            + 3;

        let frontmatter = &content[4..end_idx];
        let prompt_template = content[end_idx + 4..].trim().to_string();

        // Parse frontmatter as YAML (simplified parsing for basic cases)
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
                // Parse YAML list
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

    /// Render the prompt template with context.
    ///
    /// Replaces `{context}` with the provided context string.
    pub fn render(&self, context: &str) -> String {
        self.prompt_template.replace("{context}", context)
    }

    /// Check if a tool is allowed for this skill.
    pub fn is_tool_allowed(&self, tool: &str) -> bool {
        match &self.allowed_tools {
            Some(tools) => tools.iter().any(|t| t == tool),
            None => true, // If no restriction, all tools are allowed
        }
    }
}

/// Errors that can occur when working with skills.
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
