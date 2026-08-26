// @amadeus-header
// summary: Versioned facts, associations, templates, and controlled-forgetting domain contracts.
// layer: core
// status: active
// feature_flags: none
// provides:
// - type: crate::AssociationEdge
// - type: crate::DeletionPlan
// - type: crate::Episode
// - type: crate::FactVersion
// - type: crate::ForgetIntent
// - type: crate::MemoryTemplate
// - type: crate::TemplateEvidence
// uses:
// - protocol: serde serialization
// invariants:
// - Fact updates preserve version history instead of overwriting prior facts.
// - Deletion plans identify candidates without containing unredacted sensitive content.
// side_effects: none
// tests:
// - cmd: cargo test -p memory-domain
// @end-amadeus-header

//! Domain contracts for structured and privacy-aware agent memory.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    Public,
    Personal,
    Confidential,
    Secret,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub source: String,
    pub content: String,
    pub observed_at_ms: u64,
    pub content_hash: String,
    pub sensitivity: Sensitivity,
    pub deleted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactStatus {
    Active,
    Historical,
    Deleted,
    NeedsReview,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactVersion {
    pub id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub context: Option<String>,
    pub valid_from_ms: u64,
    pub valid_to_ms: Option<u64>,
    pub confidence: f32,
    pub status: FactStatus,
    pub source_episode_ids: Vec<String>,
    pub version: u32,
    pub supersedes_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictAction {
    Add,
    Reinforce,
    Supersede,
    Merge,
    KeepBoth,
    Reject,
    NeedsReview,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConflictOutcome {
    pub action: ConflictAction,
    pub fact_id: String,
    pub superseded_fact_id: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssociationEdge {
    pub id: String,
    pub from_id: String,
    pub to_id: String,
    pub relation: String,
    pub weight: f32,
    pub source_episode_id: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateStatus {
    Draft,
    Active,
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateEvidence {
    pub episode_id: String,
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryTemplate {
    pub id: String,
    pub name: String,
    pub intent: String,
    pub preconditions: Vec<String>,
    pub input_schema: String,
    pub steps: Vec<String>,
    pub output_schema: String,
    #[serde(default)]
    pub evidence: Vec<TemplateEvidence>,
    pub evidence_episode_ids: Vec<String>,
    pub success_count: u32,
    pub failure_count: u32,
    pub version: u32,
    pub status: TemplateStatus,
    pub human_reviewed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactInput {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub context: Option<String>,
    pub source: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub fact: FactVersion,
    pub score: f32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForgetIntent {
    pub original: String,
    pub terms: Vec<String>,
    pub excluded_terms: Vec<String>,
    pub source_scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeletionPlan {
    pub id: String,
    pub intent: ForgetIntent,
    pub fact_ids: Vec<String>,
    pub episode_ids: Vec<String>,
    pub edge_ids: Vec<String>,
    pub template_ids: Vec<String>,
    pub redacted_previews: Vec<String>,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    pub plan_hash: String,
    pub confirmation_required: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForgetResult {
    pub plan_id: String,
    pub deleted_facts: usize,
    pub deleted_episodes: usize,
    pub deleted_edges: usize,
    pub affected_templates: usize,
    pub residual_matches: usize,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub plan_id: String,
    pub plan_hash: String,
    pub executed_at_ms: u64,
    pub deleted_facts: usize,
    pub deleted_episodes: usize,
    pub result: String,
}
