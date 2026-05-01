// @amadeus-header
// summary: Composable system prompt builder with prioritised sections.
// layer: core
// status: active
// feature_flags: none
// provides:
// - type: crate::builder::PromptSection
// - type: crate::builder::SystemPromptBuilder
// - const: crate::builder::DYNAMIC_BOUNDARY_MARKER
// uses: none
// invariants:
// - Sections are joined in priority order with double-newline separators.
// - The DYNAMIC_BOUNDARY_MARKER separates static from runtime-injected content.
// side_effects: none
// tests:
// - cmd: cargo test -p prompts
// @end-amadeus-header

//! Composable system prompt builder.
//!
//! The builder replaces the old monolithic `SYSTEM_PROMPT` const with
//! prioritised, replaceable sections that can be extended at runtime.

/// Marker inserted between static and dynamic prompt content.
pub const DYNAMIC_BOUNDARY_MARKER: &str = "__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__";

/// A single named section of the system prompt.
#[derive(Debug, Clone)]
pub struct PromptSection {
    /// Unique identifier (e.g. "core_loop", "security", "memory").
    pub id: String,
    /// Human-readable title for display.
    pub title: String,
    /// The prompt text for this section.
    pub content: String,
    /// Ordering priority — lower numbers appear earlier.
    pub priority: i32,
    /// Whether this section can be updated at runtime.
    pub dynamic: bool,
}

impl PromptSection {
    pub fn new(id: impl Into<String>, title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            content: content.into(),
            priority: 50,
            dynamic: false,
        }
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_dynamic(mut self, dynamic: bool) -> Self {
        self.dynamic = dynamic;
        self
    }
}

/// Summary of a section for API responses (no full content).
#[derive(Debug, Clone)]
pub struct PromptSectionSummary {
    pub id: String,
    pub title: String,
    pub priority: i32,
    pub dynamic: bool,
    pub content_preview: String,
}

/// Builds a system prompt from prioritised, composable sections.
#[derive(Debug, Clone, Default)]
pub struct SystemPromptBuilder {
    sections: Vec<PromptSection>,
}

impl SystemPromptBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start with predefined sections.
    pub fn with_sections(sections: Vec<PromptSection>) -> Self {
        Self { sections }
    }

    /// Add a section. If one with the same id exists, it is replaced.
    pub fn add_section(mut self, section: PromptSection) -> Self {
        self.sections.retain(|s| s.id != section.id);
        self.sections.push(section);
        self
    }

    /// Replace a section by id. No-op if the id is not found.
    pub fn replace_section(&mut self, section: PromptSection) {
        self.sections.retain(|s| s.id != section.id);
        self.sections.push(section);
    }

    /// Remove a section by id.
    pub fn remove_section(mut self, section_id: &str) -> Self {
        self.sections.retain(|s| s.id != section_id);
        self
    }

    /// Get a section by id.
    pub fn get_section(&self, section_id: &str) -> Option<&PromptSection> {
        self.sections.iter().find(|s| s.id == section_id)
    }

    /// List all sections as summaries.
    pub fn summaries(&self) -> Vec<PromptSectionSummary> {
        let mut sorted: Vec<_> = self.sections.iter().collect();
        sorted.sort_by_key(|s| s.priority);
        sorted
            .iter()
            .map(|s| PromptSectionSummary {
                id: s.id.clone(),
                title: s.title.clone(),
                priority: s.priority,
                dynamic: s.dynamic,
                content_preview: s.content.chars().take(200).collect(),
            })
            .collect()
    }

    /// Build the final prompt string by joining sections sorted by priority.
    pub fn build(&self) -> String {
        let mut sorted: Vec<_> = self.sections.iter().collect();
        sorted.sort_by_key(|s| s.priority);
        sorted
            .iter()
            .map(|s| s.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Build with the dynamic boundary marker inserted between static and
    /// dynamic sections.
    pub fn build_with_boundary(&self) -> String {
        let mut sorted: Vec<_> = self.sections.iter().collect();
        sorted.sort_by_key(|s| s.priority);

        let mut result = String::new();
        let mut seen_dynamic = false;

        for section in &sorted {
            if section.dynamic && !seen_dynamic {
                seen_dynamic = true;
                if !result.is_empty() {
                    result.push_str("\n\n");
                }
                result.push_str(DYNAMIC_BOUNDARY_MARKER);
                result.push_str("\n\n");
            }
            if !result.is_empty() && !result.ends_with("\n\n") {
                result.push_str("\n\n");
            }
            result.push_str(&section.content);
        }

        result
    }

    /// Number of sections.
    pub fn len(&self) -> usize {
        self.sections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    /// Iterate over all sections.
    pub fn sections(&self) -> &[PromptSection] {
        &self.sections
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_sorts_by_priority() {
        let prompt = SystemPromptBuilder::new()
            .add_section(PromptSection::new("b", "B", "beta").with_priority(20))
            .add_section(PromptSection::new("a", "A", "alpha").with_priority(10))
            .build();

        let a_pos = prompt.find("alpha").unwrap();
        let b_pos = prompt.find("beta").unwrap();
        assert!(a_pos < b_pos, "alpha should appear before beta");
    }

    #[test]
    fn builder_add_replaces_existing_id() {
        let prompt = SystemPromptBuilder::new()
            .add_section(PromptSection::new("x", "First", "v1"))
            .add_section(PromptSection::new("x", "Second", "v2"))
            .build();

        assert!(!prompt.contains("v1"));
        assert!(prompt.contains("v2"));
    }

    #[test]
    fn builder_remove_section() {
        let prompt = SystemPromptBuilder::new()
            .add_section(PromptSection::new("keep", "K", "keep me"))
            .add_section(PromptSection::new("drop", "D", "drop me"))
            .remove_section("drop")
            .build();

        assert!(prompt.contains("keep me"));
        assert!(!prompt.contains("drop me"));
    }

    #[test]
    fn dynamic_boundary_marker_inserted() {
        let prompt = SystemPromptBuilder::new()
            .add_section(
                PromptSection::new("static", "S", "static content").with_priority(10),
            )
            .add_section(
                PromptSection::new("dynamic", "D", "dynamic content")
                    .with_priority(20)
                    .with_dynamic(true),
            )
            .build_with_boundary();

        assert!(prompt.contains(DYNAMIC_BOUNDARY_MARKER));
        let marker_pos = prompt.find(DYNAMIC_BOUNDARY_MARKER).unwrap();
        let static_pos = prompt.find("static content").unwrap();
        let dynamic_pos = prompt.find("dynamic content").unwrap();
        assert!(static_pos < marker_pos);
        assert!(dynamic_pos > marker_pos);
    }

    #[test]
    fn summaries_are_sorted() {
        let builder = SystemPromptBuilder::new()
            .add_section(PromptSection::new("z", "Z", "last").with_priority(99))
            .add_section(PromptSection::new("a", "A", "first").with_priority(1));

        let summaries = builder.summaries();
        assert_eq!(summaries[0].id, "a");
        assert_eq!(summaries[1].id, "z");
    }
}
