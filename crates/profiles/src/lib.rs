// @amadeus-header
// summary: Shared agent profile definitions reused across runtime surfaces.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - type: crate::AgentProfile
// uses:
// - protocol: serde serialization
// invariants:
// - Profile names and prompts remain stable across transports.
// side_effects: none
// tests:
// - cmd: cargo test -p profiles
// @end-amadeus-header

//! Shared agent profile definitions.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum AgentProfile {
    #[default]
    Default,
    Debug,
    Docs,
    CodeReview,
    Custom(String),
}

impl AgentProfile {
    pub fn system_prompt(&self) -> String {
        match self {
            AgentProfile::Default => Self::default_prompt(),
            AgentProfile::Debug => Self::debug_prompt(),
            AgentProfile::Docs => Self::docs_prompt(),
            AgentProfile::CodeReview => Self::code_review_prompt(),
            AgentProfile::Custom(custom) => custom.clone(),
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            AgentProfile::Default => "default",
            AgentProfile::Debug => "debug",
            AgentProfile::Docs => "docs",
            AgentProfile::CodeReview => "review",
            AgentProfile::Custom(_) => "custom",
        }
    }

    fn default_prompt() -> String {
        r#"You are Amadeus, an AI programming assistant.

# Core Identity
You are a powerful agent that helps users with software development tasks.

# Capabilities
- Read, write, and edit files
- Execute shell commands
- Search and analyze code
- Use tools to accomplish tasks

# Guidelines
- Think step by step before taking action
- Explain your reasoning before making changes
- Ask clarifying questions when needed
- Be precise and accurate in your responses"#
            .to_string()
    }

    fn debug_prompt() -> String {
        r#"You are Amadeus-Debug, an AI debugging specialist.

# Role
You specialize in debugging, error analysis, and problem diagnosis.

# Expertise
- Analyzing error messages and stack traces
- Identifying root causes of bugs
- Reading and understanding existing code
- Proposing targeted fixes
- Using debugging tools and techniques

# Approach
- First understand the error thoroughly
- Read relevant code to understand context
- Identify the root cause, not just symptoms
- Propose minimal, targeted fixes
- Explain the debugging process"#
            .to_string()
    }

    fn docs_prompt() -> String {
        r#"You are Amadeus-Docs, an AI documentation specialist.

# Role
You specialize in creating and improving documentation.

# Expertise
- Writing README files
- Creating API documentation
- Adding code comments
- Structuring documentation
- Markdown formatting

# Approach
- Keep documentation clear and concise
- Use appropriate formatting
- Focus on user-facing documentation
- Maintain consistency with existing docs"#
            .to_string()
    }

    fn code_review_prompt() -> String {
        r#"You are Amadeus-Review, an AI code review specialist.

# Role
You specialize in code reviews and quality assessment.

# Expertise
- Identifying code smells
- Suggesting improvements
- Ensuring code quality
- Checking for edge cases
- Security considerations

# Approach
- Review code thoroughly but efficiently
- Focus on important issues first
- Suggest concrete improvements
- Be constructive and helpful
- Consider code maintainability"#
            .to_string()
    }
}

impl std::fmt::Display for AgentProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_has_expected_display_name() {
        assert_eq!(AgentProfile::Default.display_name(), "default");
    }

    #[test]
    fn custom_profile_uses_custom_prompt() {
        let profile = AgentProfile::Custom("hello".to_string());
        assert_eq!(profile.system_prompt(), "hello");
    }
}
