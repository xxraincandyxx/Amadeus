// @amadeus-header
// summary: Skills subsystem code for registry.
// layer: infra
// status: active
// feature_flags: none
// provides:
// - module: crate::skills::registry
// - type: crate::skills::registry::SkillRegistry
// uses:
// - artifact: filesystem paths and files
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Reads or writes filesystem state.
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

//! # Skill Registry
//!
//! Registry for loading and managing skills.

use std::collections::HashMap;
use std::path::Path;

use super::{Skill, SkillError};

/// Registry for managing skills.
#[derive(Debug, Clone, Default)]
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    /// Create a new empty skill registry.
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// Load skills from a directory.
    ///
    /// Searches for `.md` files and attempts to load them as skills.
    pub fn load_from_dir(path: &Path) -> Result<Self, SkillError> {
        let mut registry = Self::new();

        if !path.exists() {
            return Ok(registry);
        }

        let entries = std::fs::read_dir(path)
            .map_err(|e| SkillError::LoadFailed(path.display().to_string(), e.to_string()))?;

        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Ok(skill) = Skill::load(&entry_path) {
                    tracing::info!(
                        skill = %skill.name,
                        path = %entry_path.display(),
                        "Loaded skill"
                    );
                    registry.skills.insert(skill.name.clone(), skill);
                } else {
                    tracing::warn!(
                        path = %entry_path.display(),
                        "Failed to load skill file"
                    );
                }
            }
        }

        Ok(registry)
    }

    /// Register a skill.
    pub fn register(&mut self, skill: Skill) {
        self.skills.insert(skill.name.clone(), skill);
    }

    /// Get a skill by name.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// List all skill names.
    pub fn list(&self) -> Vec<&str> {
        self.skills.keys().map(|s| s.as_str()).collect()
    }

    /// Get all skills.
    pub fn all(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }

    /// Check if a skill exists.
    pub fn contains(&self, name: &str) -> bool {
        self.skills.contains_key(name)
    }

    /// Get the number of registered skills.
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Consume the registry and return all skills.
    pub fn into_skills(self) -> Vec<Skill> {
        self.skills.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_registry() {
        let mut registry = SkillRegistry::new();

        let skill = Skill::new("test", "Test skill", "Template");
        registry.register(skill);

        assert!(registry.contains("test"));
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_load_from_dir() {
        let temp = TempDir::new().unwrap();

        let skill_content = r#"---
name: example
description: An example skill
---

Do something with: {context}
"#;

        fs::write(temp.path().join("example.md"), skill_content).unwrap();

        let registry = SkillRegistry::load_from_dir(temp.path()).unwrap();
        assert!(registry.contains("example"));

        let skill = registry.get("example").unwrap();
        assert_eq!(skill.name, "example");
        assert_eq!(skill.description, "An example skill");
    }

    #[test]
    fn test_load_from_nonexistent_dir() {
        let registry = SkillRegistry::load_from_dir(Path::new("/nonexistent")).unwrap();
        assert!(registry.is_empty());
    }
}
