// @amadeus-header
// summary: Persistent structured-memory conflict, retrieval, template, privacy, and forgetting service.
// layer: core
// status: active
// feature_flags: none
// provides:
// - type: crate::MemoryService
// - type: crate::MemoryServiceError
// uses:
// - module: amadeus_memory_domain
// - type: amadeus_privacy::SensitiveDataDetector
// - artifact: structured_memory.json
// invariants:
// - Conflicting exclusive facts create version chains without overwriting history.
// - Forgetting requires an unexpired plan id and matching plan hash.
// - Confirmed forgetting physically removes current and historical versions and verifies disk state.
// - Persisted facts and deletion previews contain only redacted sensitive values.
// - Workspace-backed persistence rejects linked or reparse-point memory paths.
// side_effects:
// - Reads and writes a structured-memory JSON store.
// tests:
// - cmd: cargo test -p memory-service
// @end-amadeus-header

//! Deterministic service layer for KylinMem indicators three and five.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use amadeus_memory_domain::{
    AssociationEdge, AuditRecord, ConflictAction, ConflictOutcome, DeletionPlan, Episode,
    FactInput, FactStatus, FactVersion, ForgetIntent, ForgetResult, MemoryTemplate, SearchHit,
    Sensitivity, TemplateEvidence, TemplateStatus,
};
use amadeus_privacy::{RiskLevel, SensitiveDataDetector};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const PLAN_TTL_MS: u64 = 5 * 60 * 1000;
const TEMPLATE_MIN_EVIDENCE: usize = 5;
const TEMPLATE_MIN_SUCCESS_RATE: f32 = 0.8;

#[derive(Debug, thiserror::Error)]
pub enum MemoryServiceError {
    #[error("invalid memory request: {0}")]
    InvalidInput(String),
    #[error("memory persistence failed: {0}")]
    Persistence(String),
    #[error("forget plan not found: {0}")]
    PlanNotFound(String),
    #[error("forget plan expired: {0}")]
    PlanExpired(String),
    #[error("forget confirmation did not match the previewed plan")]
    ConfirmationMismatch,
    #[error("template not found: {0}")]
    TemplateNotFound(String),
    #[error("template activation requirements are not met: {0}")]
    TemplateNotReady(String),
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct PersistentState {
    episodes: Vec<Episode>,
    facts: Vec<FactVersion>,
    edges: Vec<AssociationEdge>,
    templates: Vec<MemoryTemplate>,
    audits: Vec<AuditRecord>,
}

#[derive(Debug)]
struct ServiceState {
    persistent: PersistentState,
    plans: HashMap<String, DeletionPlan>,
}

#[derive(Debug)]
pub struct MemoryService {
    path: PathBuf,
    workspace_root: Option<PathBuf>,
    detector: SensitiveDataDetector,
    state: Mutex<ServiceState>,
}

impl MemoryService {
    pub fn open(path: PathBuf) -> Result<Self, MemoryServiceError> {
        reject_link_target(&path)?;
        let detector = SensitiveDataDetector::new();
        let mut persistent = load_state(&path)?;
        sanitize_loaded_state(&detector, &mut persistent);
        if path.exists() {
            persist_locked(&path, None, &persistent)?;
        }
        Ok(Self {
            path,
            workspace_root: None,
            detector,
            state: Mutex::new(ServiceState {
                persistent,
                plans: HashMap::new(),
            }),
        })
    }

    pub fn open_in_workspace(workdir: PathBuf) -> Result<Self, MemoryServiceError> {
        let root = workdir.canonicalize().map_err(|error| {
            MemoryServiceError::Persistence(format!(
                "canonicalize workspace {}: {error}",
                workdir.display()
            ))
        })?;
        let memory_dir = root.join(".amadeus");
        if !memory_dir.exists() {
            fs::create_dir(&memory_dir).map_err(|error| {
                MemoryServiceError::Persistence(format!(
                    "create secure memory directory {}: {error}",
                    memory_dir.display()
                ))
            })?;
        }
        let path = memory_dir.join("structured_memory.json");
        validate_workspace_store_path(&root, &path)?;
        let detector = SensitiveDataDetector::new();
        let mut persistent = load_state(&path)?;
        sanitize_loaded_state(&detector, &mut persistent);
        if path.exists() {
            persist_locked(&path, Some(&root), &persistent)?;
        }
        Ok(Self {
            path,
            workspace_root: Some(root),
            detector,
            state: Mutex::new(ServiceState {
                persistent,
                plans: HashMap::new(),
            }),
        })
    }

    pub fn store_fact(&self, input: FactInput) -> Result<ConflictOutcome, MemoryServiceError> {
        validate_fact_input(&input)?;
        let now = now_ms();
        let (subject, subject_spans) = self.detector.redact(input.subject.trim());
        let predicate = safe_structural_text(&self.detector, "predicate", &input.predicate)?;
        let source = safe_structural_text(&self.detector, "source", &input.source)?;
        let (object, object_spans) = self.detector.redact(input.object.trim());
        let (context, context_spans) = match input.context.as_deref() {
            Some(value) => {
                let (redacted, spans) = self.detector.redact(value.trim());
                (Some(redacted), spans)
            }
            None => (None, Vec::new()),
        };
        let sensitivity = subject_spans
            .iter()
            .chain(&object_spans)
            .chain(&context_spans)
            .map(|span| span.risk)
            .max()
            .map(sensitivity_from_risk)
            .unwrap_or(Sensitivity::Public);
        let episode_content = format!(
            "{} | {} | {} | {}",
            subject,
            predicate,
            object,
            context.as_deref().unwrap_or("")
        );
        let episode = Episode {
            id: id("episode"),
            source,
            content_hash: stable_hash(&episode_content),
            content: episode_content,
            observed_at_ms: now,
            sensitivity,
            deleted: false,
        };

        let mut state = self.state.lock().map_err(lock_error)?;
        let normalized_predicate = normalize(&predicate);
        let active_indices: Vec<usize> = state
            .persistent
            .facts
            .iter()
            .enumerate()
            .filter(|(_, fact)| {
                fact.status == FactStatus::Active
                    && normalize(&fact.subject) == normalize(&subject)
                    && normalize(&fact.predicate) == normalized_predicate
            })
            .map(|(index, _)| index)
            .collect();

        if let Some(index) = active_indices.iter().copied().find(|index| {
            let fact = &state.persistent.facts[*index];
            normalize(&fact.object) == normalize(&object)
                && same_context(fact.context.as_deref(), context.as_deref())
        }) {
            let fact = &mut state.persistent.facts[index];
            fact.confidence = (fact.confidence.max(input.confidence) + 0.05).min(1.0);
            fact.source_episode_ids.push(episode.id.clone());
            let fact_id = fact.id.clone();
            state.persistent.episodes.push(episode);
            persist_locked(
                &self.path,
                self.workspace_root.as_deref(),
                &state.persistent,
            )?;
            return Ok(ConflictOutcome {
                action: ConflictAction::Reinforce,
                fact_id,
                superseded_fact_id: None,
                reason: "same subject, predicate, object, and context".to_string(),
            });
        }

        let merge_index = active_indices.iter().copied().find(|index| {
            let fact = &state.persistent.facts[*index];
            same_context(fact.context.as_deref(), context.as_deref())
                && objects_overlap(&fact.object, &object)
        });
        let exclusive_index = active_indices.iter().copied().find(|index| {
            let fact = &state.persistent.facts[*index];
            same_context(fact.context.as_deref(), context.as_deref())
        });
        let action = if merge_index.is_some() {
            ConflictAction::Merge
        } else if is_exclusive_predicate(&normalized_predicate) && exclusive_index.is_some() {
            ConflictAction::Supersede
        } else if !active_indices.is_empty() {
            ConflictAction::KeepBoth
        } else {
            ConflictAction::Add
        };
        let superseded_index = merge_index.or_else(|| {
            if action == ConflictAction::Supersede {
                exclusive_index
            } else {
                None
            }
        });
        let superseded_id = superseded_index.map(|index| state.persistent.facts[index].id.clone());
        let version = active_indices
            .iter()
            .map(|index| state.persistent.facts[*index].version)
            .max()
            .unwrap_or(0)
            + 1;
        if let Some(index) = superseded_index {
            state.persistent.facts[index].status = FactStatus::Historical;
            state.persistent.facts[index].valid_to_ms = Some(now);
        }
        let merged_object = superseded_index
            .filter(|_| action == ConflictAction::Merge)
            .map(|index| longer_value(&state.persistent.facts[index].object, &object))
            .unwrap_or(object);
        let fact = FactVersion {
            id: id("fact"),
            subject,
            predicate,
            object: merged_object,
            context,
            valid_from_ms: now,
            valid_to_ms: None,
            confidence: input.confidence.clamp(0.0, 1.0),
            status: FactStatus::Active,
            source_episode_ids: vec![episode.id.clone()],
            version,
            supersedes_id: superseded_id.clone(),
        };
        let fact_id = fact.id.clone();
        state.persistent.edges.push(AssociationEdge {
            id: id("edge"),
            from_id: format!("entity:{}", normalize(&fact.subject)),
            to_id: fact.id.clone(),
            relation: fact.predicate.clone(),
            weight: fact.confidence,
            source_episode_id: episode.id.clone(),
            deleted: false,
        });
        state.persistent.episodes.push(episode);
        state.persistent.facts.push(fact);
        persist_locked(
            &self.path,
            self.workspace_root.as_deref(),
            &state.persistent,
        )?;
        Ok(ConflictOutcome {
            action,
            fact_id,
            superseded_fact_id: superseded_id,
            reason: conflict_reason(action).to_string(),
        })
    }

    pub fn search(
        &self,
        query: &str,
        include_history: bool,
        top_k: usize,
    ) -> Result<Vec<SearchHit>, MemoryServiceError> {
        if query.trim().is_empty() {
            return Err(MemoryServiceError::InvalidInput(
                "search query is empty".to_string(),
            ));
        }
        let state = self.state.lock().map_err(lock_error)?;
        Ok(search_facts(
            &state.persistent,
            query,
            include_history,
            top_k,
        ))
    }

    pub fn register_template(
        &self,
        mut template: MemoryTemplate,
    ) -> Result<String, MemoryServiceError> {
        if template.name.trim().is_empty()
            || template.intent.trim().is_empty()
            || template.steps.is_empty()
        {
            return Err(MemoryServiceError::InvalidInput(
                "template name, intent, and steps are required".to_string(),
            ));
        }
        template.id = id("template");
        template.name = self.detector.redact(template.name.trim()).0;
        template.intent = self.detector.redact(template.intent.trim()).0;
        template.preconditions = template
            .preconditions
            .iter()
            .map(|value| self.detector.redact(value.trim()).0)
            .collect();
        template.steps = template
            .steps
            .iter()
            .map(|value| self.detector.redact(value.trim()).0)
            .collect();
        template.input_schema =
            safe_json_schema(&self.detector, "input_schema", &template.input_schema)?;
        template.output_schema =
            safe_json_schema(&self.detector, "output_schema", &template.output_schema)?;
        template.evidence.clear();
        template.evidence_episode_ids.clear();
        template.success_count = 0;
        template.failure_count = 0;
        template.version = 1;
        template.human_reviewed = false;
        template.status = TemplateStatus::Draft;
        let template_id = template.id.clone();
        let mut state = self.state.lock().map_err(lock_error)?;
        state.persistent.templates.push(template);
        persist_locked(
            &self.path,
            self.workspace_root.as_deref(),
            &state.persistent,
        )?;
        Ok(template_id)
    }

    pub fn record_template_result(
        &self,
        template_id: &str,
        episode_id: &str,
        success: bool,
    ) -> Result<(), MemoryServiceError> {
        let mut state = self.state.lock().map_err(lock_error)?;
        let episode_exists = state
            .persistent
            .episodes
            .iter()
            .any(|episode| episode.id == episode_id && !episode.deleted);
        if !episode_exists {
            return Err(MemoryServiceError::InvalidInput(format!(
                "template evidence episode does not exist or was deleted: {episode_id}"
            )));
        }
        let template = state
            .persistent
            .templates
            .iter_mut()
            .find(|template| template.id == template_id)
            .ok_or_else(|| MemoryServiceError::TemplateNotFound(template_id.to_string()))?;
        if template
            .evidence
            .iter()
            .any(|evidence| evidence.episode_id == episode_id)
        {
            return Err(MemoryServiceError::InvalidInput(format!(
                "template evidence was already recorded: {episode_id}"
            )));
        }
        template.evidence.push(TemplateEvidence {
            episode_id: episode_id.to_string(),
            success,
        });
        refresh_template_evidence(template);
        persist_locked(
            &self.path,
            self.workspace_root.as_deref(),
            &state.persistent,
        )
    }

    pub fn activate_template(&self, template_id: &str) -> Result<(), MemoryServiceError> {
        let mut state = self.state.lock().map_err(lock_error)?;
        let live_episode_ids: HashSet<String> = state
            .persistent
            .episodes
            .iter()
            .filter(|episode| !episode.deleted)
            .map(|episode| episode.id.clone())
            .collect();
        let template = state
            .persistent
            .templates
            .iter_mut()
            .find(|template| template.id == template_id)
            .ok_or_else(|| MemoryServiceError::TemplateNotFound(template_id.to_string()))?;
        if template
            .evidence
            .iter()
            .any(|evidence| !live_episode_ids.contains(&evidence.episode_id))
        {
            return Err(MemoryServiceError::TemplateNotReady(
                "one or more evidence episodes no longer exist".to_string(),
            ));
        }
        let attempts = template.evidence.len() as u32;
        let success_rate = if attempts == 0 {
            0.0
        } else {
            template
                .evidence
                .iter()
                .filter(|evidence| evidence.success)
                .count() as f32
                / attempts as f32
        };
        if template.evidence.len() < TEMPLATE_MIN_EVIDENCE
            || success_rate < TEMPLATE_MIN_SUCCESS_RATE
        {
            return Err(MemoryServiceError::TemplateNotReady(format!(
                "evidence={}, success_rate={success_rate:.2}",
                template.evidence.len()
            )));
        }
        template.human_reviewed = true;
        template.status = TemplateStatus::Active;
        persist_locked(
            &self.path,
            self.workspace_root.as_deref(),
            &state.persistent,
        )
    }

    pub fn active_templates_for(
        &self,
        intent: &str,
    ) -> Result<Vec<MemoryTemplate>, MemoryServiceError> {
        let state = self.state.lock().map_err(lock_error)?;
        let intent = normalize(intent);
        Ok(state
            .persistent
            .templates
            .iter()
            .filter(|template| {
                template.status == TemplateStatus::Active
                    && normalize(&template.intent).contains(&intent)
            })
            .cloned()
            .collect())
    }

    pub fn parse_forget_intent(
        &self,
        instruction: &str,
    ) -> Result<ForgetIntent, MemoryServiceError> {
        parse_forget_intent(instruction)
    }

    pub fn propose_forget(&self, instruction: &str) -> Result<DeletionPlan, MemoryServiceError> {
        let intent = parse_forget_intent(instruction)?;
        let now = now_ms();
        let mut state = self.state.lock().map_err(lock_error)?;
        let candidates: Vec<&FactVersion> = state
            .persistent
            .facts
            .iter()
            .filter(|fact| fact_matches_intent(&state.persistent, fact, &intent))
            .collect();
        if candidates.is_empty() {
            return Err(MemoryServiceError::InvalidInput(
                "forget instruction matched no memories; clarify the target".to_string(),
            ));
        }
        let fact_ids: Vec<String> = candidates.iter().map(|fact| fact.id.clone()).collect();
        let episode_ids: Vec<String> = candidates
            .iter()
            .flat_map(|fact| fact.source_episode_ids.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let edge_ids: Vec<String> = state
            .persistent
            .edges
            .iter()
            .filter(|edge| fact_ids.contains(&edge.from_id) || fact_ids.contains(&edge.to_id))
            .map(|edge| edge.id.clone())
            .collect();
        let template_ids: Vec<String> = state
            .persistent
            .templates
            .iter()
            .filter(|template| {
                template
                    .evidence
                    .iter()
                    .any(|evidence| episode_ids.contains(&evidence.episode_id))
                    || template
                        .evidence_episode_ids
                        .iter()
                        .any(|id| episode_ids.contains(id))
            })
            .map(|template| template.id.clone())
            .collect();
        let redacted_previews: Vec<String> = candidates
            .iter()
            .map(|fact| format!("{} / {} / {}", fact.subject, fact.predicate, fact.object))
            .collect();
        let plan_id = id("forget");
        let plan_hash = stable_hash(&format!(
            "{}|{}|{}|{}",
            plan_id,
            instruction,
            fact_ids.join(","),
            episode_ids.join(",")
        ));
        let plan = DeletionPlan {
            id: plan_id.clone(),
            intent,
            fact_ids,
            episode_ids,
            edge_ids,
            template_ids,
            redacted_previews,
            created_at_ms: now,
            expires_at_ms: now + PLAN_TTL_MS,
            plan_hash,
            confirmation_required: true,
        };
        state.plans.insert(plan_id, plan.clone());
        Ok(plan)
    }

    pub fn confirm_forget(
        &self,
        plan_id: &str,
        plan_hash: &str,
    ) -> Result<ForgetResult, MemoryServiceError> {
        let now = now_ms();
        let mut state = self.state.lock().map_err(lock_error)?;
        let plan = state
            .plans
            .get(plan_id)
            .cloned()
            .ok_or_else(|| MemoryServiceError::PlanNotFound(plan_id.to_string()))?;
        if plan.expires_at_ms < now {
            state.plans.remove(plan_id);
            return Err(MemoryServiceError::PlanExpired(plan_id.to_string()));
        }
        if plan.plan_hash != plan_hash {
            return Err(MemoryServiceError::ConfirmationMismatch);
        }
        state.plans.remove(plan_id);

        let fact_ids: HashSet<&str> = plan.fact_ids.iter().map(String::as_str).collect();
        let edge_ids: HashSet<&str> = plan.edge_ids.iter().map(String::as_str).collect();
        let episode_ids: HashSet<&str> = plan.episode_ids.iter().map(String::as_str).collect();
        let before_facts = state.persistent.facts.len();
        state
            .persistent
            .facts
            .retain(|fact| !fact_ids.contains(fact.id.as_str()));
        let deleted_facts = before_facts - state.persistent.facts.len();

        let before_edges = state.persistent.edges.len();
        state
            .persistent
            .edges
            .retain(|edge| !edge_ids.contains(edge.id.as_str()));
        let deleted_edges = before_edges - state.persistent.edges.len();

        let referenced_episode_ids: HashSet<String> = state
            .persistent
            .facts
            .iter()
            .flat_map(|fact| fact.source_episode_ids.clone())
            .collect();
        let removable_episode_ids: HashSet<&str> = episode_ids
            .iter()
            .copied()
            .filter(|id| !referenced_episode_ids.contains(*id))
            .collect();
        let before_episodes = state.persistent.episodes.len();
        state
            .persistent
            .episodes
            .retain(|episode| !removable_episode_ids.contains(episode.id.as_str()));
        let deleted_episodes = before_episodes - state.persistent.episodes.len();

        let mut affected_templates = 0;
        for template in &mut state.persistent.templates {
            let before = template.evidence.len();
            template
                .evidence
                .retain(|evidence| !episode_ids.contains(evidence.episode_id.as_str()));
            template
                .evidence_episode_ids
                .retain(|id| !episode_ids.contains(id.as_str()));
            if template.evidence.len() != before {
                affected_templates += 1;
                refresh_template_evidence(template);
                let attempts = template.evidence.len();
                let successes = template
                    .evidence
                    .iter()
                    .filter(|evidence| evidence.success)
                    .count();
                let success_rate = if attempts == 0 {
                    0.0
                } else {
                    successes as f32 / attempts as f32
                };
                if attempts < TEMPLATE_MIN_EVIDENCE || success_rate < TEMPLATE_MIN_SUCCESS_RATE {
                    template.status = TemplateStatus::Retired;
                }
            }
        }

        persist_locked(
            &self.path,
            self.workspace_root.as_deref(),
            &state.persistent,
        )?;
        let persisted = load_state(&self.path)?;
        let residual_matches = persisted
            .facts
            .iter()
            .filter(|fact| fact_ids.contains(fact.id.as_str()))
            .count()
            + persisted
                .edges
                .iter()
                .filter(|edge| edge_ids.contains(edge.id.as_str()))
                .count()
            + persisted
                .episodes
                .iter()
                .filter(|episode| removable_episode_ids.contains(episode.id.as_str()))
                .count();
        let verified = residual_matches == 0;
        state.persistent.audits.push(AuditRecord {
            plan_id: plan.id.clone(),
            plan_hash: plan.plan_hash.clone(),
            executed_at_ms: now,
            deleted_facts,
            deleted_episodes,
            result: if verified { "verified" } else { "partial" }.to_string(),
        });
        persist_locked(
            &self.path,
            self.workspace_root.as_deref(),
            &state.persistent,
        )?;
        Ok(ForgetResult {
            plan_id: plan.id,
            deleted_facts,
            deleted_episodes,
            deleted_edges,
            affected_templates,
            residual_matches,
            verified,
        })
    }

    pub fn fact_history(
        &self,
        subject: &str,
        predicate: &str,
    ) -> Result<Vec<FactVersion>, MemoryServiceError> {
        let state = self.state.lock().map_err(lock_error)?;
        let mut facts: Vec<_> = state
            .persistent
            .facts
            .iter()
            .filter(|fact| {
                normalize(&fact.subject) == normalize(subject)
                    && normalize(&fact.predicate) == normalize(predicate)
            })
            .cloned()
            .collect();
        facts.sort_by_key(|fact| fact.version);
        Ok(facts)
    }

    pub fn audit_records(&self) -> Result<Vec<AuditRecord>, MemoryServiceError> {
        Ok(self
            .state
            .lock()
            .map_err(lock_error)?
            .persistent
            .audits
            .clone())
    }
}

fn validate_fact_input(input: &FactInput) -> Result<(), MemoryServiceError> {
    if input.subject.trim().is_empty()
        || input.predicate.trim().is_empty()
        || input.object.trim().is_empty()
        || input.source.trim().is_empty()
    {
        return Err(MemoryServiceError::InvalidInput(
            "subject, predicate, object, and source are required".to_string(),
        ));
    }
    if !input.confidence.is_finite() {
        return Err(MemoryServiceError::InvalidInput(
            "confidence must be finite".to_string(),
        ));
    }
    Ok(())
}

fn safe_structural_text(
    detector: &SensitiveDataDetector,
    field: &str,
    value: &str,
) -> Result<String, MemoryServiceError> {
    let value = value.trim();
    if !detector.detect(value).is_empty() {
        return Err(MemoryServiceError::InvalidInput(format!(
            "sensitive data is not allowed in structural field '{field}'"
        )));
    }
    Ok(value.to_string())
}

fn safe_json_schema(
    detector: &SensitiveDataDetector,
    field: &str,
    value: &str,
) -> Result<String, MemoryServiceError> {
    let value = safe_structural_text(detector, field, value)?;
    serde_json::from_str::<serde_json::Value>(&value).map_err(|error| {
        MemoryServiceError::InvalidInput(format!("{field} must be valid JSON: {error}"))
    })?;
    Ok(value)
}

fn refresh_template_evidence(template: &mut MemoryTemplate) {
    template.evidence_episode_ids = template
        .evidence
        .iter()
        .map(|evidence| evidence.episode_id.clone())
        .collect();
    template.success_count = template
        .evidence
        .iter()
        .filter(|evidence| evidence.success)
        .count() as u32;
    template.failure_count = template.evidence.len() as u32 - template.success_count;
}

fn sanitize_loaded_state(detector: &SensitiveDataDetector, state: &mut PersistentState) {
    let deleted_fact_ids: HashSet<String> = state
        .facts
        .iter()
        .filter(|fact| fact.status == FactStatus::Deleted)
        .map(|fact| fact.id.clone())
        .collect();
    let deleted_episode_ids: HashSet<String> = state
        .facts
        .iter()
        .filter(|fact| deleted_fact_ids.contains(&fact.id))
        .flat_map(|fact| fact.source_episode_ids.clone())
        .collect();
    state
        .facts
        .retain(|fact| !deleted_fact_ids.contains(&fact.id));
    state.edges.retain(|edge| {
        !deleted_fact_ids.contains(&edge.from_id) && !deleted_fact_ids.contains(&edge.to_id)
    });
    let referenced_episode_ids: HashSet<String> = state
        .facts
        .iter()
        .flat_map(|fact| fact.source_episode_ids.clone())
        .collect();
    let removed_episode_ids: HashSet<String> = deleted_episode_ids
        .difference(&referenced_episode_ids)
        .cloned()
        .collect();
    state
        .episodes
        .retain(|episode| !removed_episode_ids.contains(&episode.id));
    let live_episode_ids: HashSet<String> = state
        .episodes
        .iter()
        .map(|episode| episode.id.clone())
        .collect();
    for template in &mut state.templates {
        let before = template.evidence.len();
        template
            .evidence
            .retain(|evidence| live_episode_ids.contains(&evidence.episode_id));
        template
            .evidence_episode_ids
            .retain(|id| live_episode_ids.contains(id));
        if template.evidence.len() != before {
            refresh_template_evidence(template);
            if template.evidence.len() < TEMPLATE_MIN_EVIDENCE {
                template.status = TemplateStatus::Retired;
            }
        }
    }
    for episode in &mut state.episodes {
        episode.source = detector.redact(&episode.source).0;
        episode.content = detector.redact(&episode.content).0;
    }
    for fact in &mut state.facts {
        fact.subject = detector.redact(&fact.subject).0;
        fact.predicate = detector.redact(&fact.predicate).0;
        fact.object = detector.redact(&fact.object).0;
        fact.context = fact
            .context
            .as_deref()
            .map(|value| detector.redact(value).0);
    }
    for edge in &mut state.edges {
        edge.from_id = detector.redact(&edge.from_id).0;
        edge.relation = detector.redact(&edge.relation).0;
    }
    for template in &mut state.templates {
        template.name = detector.redact(&template.name).0;
        template.intent = detector.redact(&template.intent).0;
        template.preconditions = template
            .preconditions
            .iter()
            .map(|value| detector.redact(value).0)
            .collect();
        template.input_schema = detector.redact(&template.input_schema).0;
        template.steps = template
            .steps
            .iter()
            .map(|value| detector.redact(value).0)
            .collect();
        template.output_schema = detector.redact(&template.output_schema).0;
    }
}

fn parse_forget_intent(instruction: &str) -> Result<ForgetIntent, MemoryServiceError> {
    let normalized = instruction.trim();
    let lowered = normalized.to_lowercase();
    if normalized.is_empty()
        || !(lowered.contains("忘")
            || lowered.contains("删除")
            || lowered.contains("清除")
            || lowered.contains("forget")
            || lowered.contains("delete"))
    {
        return Err(MemoryServiceError::InvalidInput(
            "instruction must explicitly request forgetting or deletion".to_string(),
        ));
    }
    let (target, excluded) = split_exclusion(normalized);
    let mut terms = concept_terms(target);
    let excluded_terms = concept_terms(excluded);
    terms.retain(|term| !excluded_terms.contains(term));
    if terms.is_empty() {
        return Err(MemoryServiceError::InvalidInput(
            "forget target is ambiguous; name a subject, relation, value, or source".to_string(),
        ));
    }
    Ok(ForgetIntent {
        original: normalized.to_string(),
        terms,
        excluded_terms,
        source_scope: parse_source_scope(normalized),
    })
}

fn split_exclusion(value: &str) -> (&str, &str) {
    for marker in ["但保留", "保留", "except", "but keep"] {
        if let Some(index) = value.to_lowercase().find(marker) {
            return (&value[..index], &value[index + marker.len()..]);
        }
    }
    (value, "")
}

fn concept_terms(value: &str) -> Vec<String> {
    let lowered = value.to_lowercase();
    let concepts = [
        (["住址", "地址", "address"].as_slice(), "address"),
        (["城市"].as_slice(), "city"),
        (["偏好", "喜欢", "preference"].as_slice(), "preference"),
        (["工作", "职业", "job", "employment"].as_slice(), "work"),
        (["电话", "手机号", "phone"].as_slice(), "phone"),
        (["邮箱", "email"].as_slice(), "email"),
        (["身份证", "证件", "national id"].as_slice(), "national_id"),
    ];
    let mut terms = Vec::new();
    for (aliases, canonical) in concepts {
        if aliases.iter().any(|alias| lowered.contains(alias)) {
            terms.push(canonical.to_string());
        }
    }
    for token in lowered.split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-') {
        if token.len() >= 3
            && !["忘", "删除", "清除", "forget", "delete"]
                .iter()
                .any(|command| token.contains(command))
            && ![
                "forget", "delete", "memory", "about", "please", "所有", "关于", "我的", "去年",
                "说过", "清除", "删除", "忘掉", "忘记",
            ]
            .contains(&token)
        {
            terms.push(token.to_string());
        }
    }
    terms.sort();
    terms.dedup();
    terms
}

fn parse_source_scope(value: &str) -> Option<String> {
    let lowered = value.to_lowercase();
    for prefix in ["来源:", "来源：", "source:"] {
        if let Some(index) = lowered.find(prefix) {
            let scope = value[index + prefix.len()..]
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(|ch: char| ",，;；。".contains(ch));
            if !scope.is_empty() {
                return Some(scope.to_string());
            }
        }
    }
    None
}

fn fact_matches_intent(state: &PersistentState, fact: &FactVersion, intent: &ForgetIntent) -> bool {
    if let Some(scope) = &intent.source_scope {
        let source_matches = state.episodes.iter().any(|episode| {
            fact.source_episode_ids.contains(&episode.id)
                && normalize(&episode.source) == normalize(scope)
        });
        if !source_matches {
            return false;
        }
    }
    let searchable = format!(
        "{} {} {} {}",
        normalize(&fact.subject),
        canonicalize(&fact.predicate),
        normalize(&fact.object),
        fact.context.as_deref().map(normalize).unwrap_or_default()
    );
    let included = intent
        .terms
        .iter()
        .any(|term| searchable.contains(&canonicalize(term)));
    let excluded = intent
        .excluded_terms
        .iter()
        .any(|term| searchable.contains(&canonicalize(term)));
    included && !excluded
}

fn search_facts(
    state: &PersistentState,
    query: &str,
    include_history: bool,
    top_k: usize,
) -> Vec<SearchHit> {
    let terms = concept_terms(query);
    let query_normalized = canonicalize(query);
    let mut hits = Vec::new();
    for fact in &state.facts {
        if fact.status == FactStatus::Deleted
            || (!include_history && fact.status != FactStatus::Active)
        {
            continue;
        }
        let fields = [
            normalize(&fact.subject),
            canonicalize(&fact.predicate),
            normalize(&fact.object),
            fact.context.as_deref().map(normalize).unwrap_or_default(),
        ];
        let mut score = 0.0;
        let mut reasons = Vec::new();
        let mut matched = false;
        for field in &fields {
            if !query_normalized.is_empty() && field.contains(&query_normalized) {
                score += 4.0;
                reasons.push("exact field match".to_string());
                matched = true;
            }
            let overlaps = terms
                .iter()
                .filter(|term| field.contains(&canonicalize(term)))
                .count();
            if overlaps > 0 {
                score += overlaps as f32;
                reasons.push("keyword match".to_string());
                matched = true;
            }
        }
        if state.edges.iter().any(|edge| {
            let entity = edge.from_id.trim_start_matches("entity:");
            !edge.deleted && edge.to_id == fact.id && query_normalized == entity
        }) {
            score += 1.5;
            reasons.push("association edge".to_string());
            matched = true;
        }
        if matched && fact.status == FactStatus::Active {
            score += 0.5;
        }
        if matched {
            score += fact.confidence.clamp(0.0, 1.0);
            reasons.sort();
            reasons.dedup();
            hits.push(SearchHit {
                fact: fact.clone(),
                score,
                reasons,
            });
        }
    }
    hits.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.fact.valid_from_ms.cmp(&left.fact.valid_from_ms))
    });
    hits.truncate(top_k);
    hits
}

fn load_state(path: &Path) -> Result<PersistentState, MemoryServiceError> {
    match fs::read_to_string(path) {
        Ok(content) if content.trim().is_empty() => Ok(PersistentState::default()),
        Ok(content) => serde_json::from_str(&content).map_err(|error| {
            MemoryServiceError::Persistence(format!("parse {}: {error}", path.display()))
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(PersistentState::default())
        }
        Err(error) => Err(MemoryServiceError::Persistence(format!(
            "read {}: {error}",
            path.display()
        ))),
    }
}

fn persist_locked(
    path: &Path,
    workspace_root: Option<&Path>,
    state: &PersistentState,
) -> Result<(), MemoryServiceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            MemoryServiceError::Persistence(format!("create {}: {error}", parent.display()))
        })?;
    }
    if let Some(root) = workspace_root {
        validate_workspace_store_path(root, path)?;
    } else {
        reject_link_target(path)?;
    }
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| MemoryServiceError::Persistence(format!("serialize: {error}")))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("structured_memory.json");
    let temporary = path.with_file_name(format!("{file_name}.{}.tmp", Uuid::new_v4()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| {
            MemoryServiceError::Persistence(format!("create {}: {error}", temporary.display()))
        })?;
    if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(MemoryServiceError::Persistence(format!(
            "write {}: {error}",
            temporary.display()
        )));
    }
    drop(file);
    if let Some(root) = workspace_root {
        validate_workspace_store_path(root, path)?;
    } else {
        reject_link_target(path)?;
    }
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path).map_err(|error| {
            MemoryServiceError::Persistence(format!("replace {}: {error}", path.display()))
        })?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(MemoryServiceError::Persistence(format!(
            "replace {}: {error}",
            path.display()
        )));
    }
    Ok(())
}

fn validate_workspace_store_path(root: &Path, path: &Path) -> Result<(), MemoryServiceError> {
    let parent = path.parent().ok_or_else(|| {
        MemoryServiceError::Persistence("structured memory path has no parent".to_string())
    })?;
    reject_reparse_component(parent)?;
    let canonical_parent = parent.canonicalize().map_err(|error| {
        MemoryServiceError::Persistence(format!(
            "canonicalize memory directory {}: {error}",
            parent.display()
        ))
    })?;
    if !canonical_parent.starts_with(root) {
        return Err(MemoryServiceError::Persistence(format!(
            "memory path escapes workspace: {}",
            path.display()
        )));
    }
    reject_reparse_component(root)?;
    reject_reparse_component(&canonical_parent)?;
    reject_link_target(path)
}

fn reject_link_target(path: &Path) -> Result<(), MemoryServiceError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link_or_reparse(&metadata) => Err(MemoryServiceError::Persistence(
            format!("refusing linked memory path: {}", path.display()),
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(MemoryServiceError::Persistence(format!(
            "inspect {}: {error}",
            path.display()
        ))),
    }
}

fn reject_reparse_component(path: &Path) -> Result<(), MemoryServiceError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        MemoryServiceError::Persistence(format!("inspect {}: {error}", path.display()))
    })?;
    if is_link_or_reparse(&metadata) {
        return Err(MemoryServiceError::Persistence(format!(
            "refusing linked memory directory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    false
}

fn lock_error<T>(error: std::sync::PoisonError<T>) -> MemoryServiceError {
    MemoryServiceError::Persistence(format!("state lock poisoned: {error}"))
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4())
}
fn stable_hash<T: Hash>(value: &T) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
fn normalize(value: &str) -> String {
    value.trim().to_lowercase()
}
fn same_context(left: Option<&str>, right: Option<&str>) -> bool {
    left.map(normalize).unwrap_or_default() == right.map(normalize).unwrap_or_default()
}
fn objects_overlap(left: &str, right: &str) -> bool {
    let left = normalize(left);
    let right = normalize(right);
    left.len() >= 3 && right.len() >= 3 && (left.contains(&right) || right.contains(&left))
}
fn longer_value(left: &str, right: &str) -> String {
    if left.chars().count() >= right.chars().count() {
        left.to_string()
    } else {
        right.to_string()
    }
}
fn is_exclusive_predicate(predicate: &str) -> bool {
    [
        "address",
        "lives_at",
        "住址",
        "地址",
        "employer",
        "工作单位",
        "phone",
        "电话",
        "email",
        "邮箱",
        "status",
        "状态",
    ]
    .iter()
    .any(|value| predicate.contains(value))
}
fn conflict_reason(action: ConflictAction) -> &'static str {
    match action {
        ConflictAction::Add => "no active fact has the same subject and predicate",
        ConflictAction::Reinforce => "identical fact observed again",
        ConflictAction::Supersede => "exclusive fact changed in the same context",
        ConflictAction::Merge => "new value specializes or expands the active value",
        ConflictAction::KeepBoth => "facts are compatible or context-scoped",
        ConflictAction::Reject => "candidate rejected",
        ConflictAction::NeedsReview => "human review required",
    }
}
fn sensitivity_from_risk(risk: RiskLevel) -> Sensitivity {
    match risk {
        RiskLevel::Medium => Sensitivity::Personal,
        RiskLevel::High => Sensitivity::Confidential,
        RiskLevel::Critical => Sensitivity::Secret,
    }
}
fn canonicalize(value: &str) -> String {
    let mut value = normalize(value);
    for (from, to) in [
        ("住址", "address"),
        ("地址", "address"),
        ("城市", "city"),
        ("偏好", "preference"),
        ("喜欢", "preference"),
        ("工作", "work"),
        ("职业", "work"),
        ("电话", "phone"),
        ("手机号", "phone"),
        ("邮箱", "email"),
        ("身份证", "national_id"),
    ] {
        value = value.replace(from, to);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn service(temp: &TempDir) -> MemoryService {
        MemoryService::open(temp.path().join("structured_memory.json")).unwrap()
    }
    fn fact(subject: &str, predicate: &str, object: &str, context: Option<&str>) -> FactInput {
        FactInput {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: object.to_string(),
            context: context.map(str::to_string),
            source: "test-session".to_string(),
            confidence: 0.9,
        }
    }

    #[test]
    fn exclusive_conflicts_preserve_version_history() {
        let temp = TempDir::new().unwrap();
        let service = service(&temp);
        assert_eq!(
            service
                .store_fact(fact("用户", "住址", "南京", None))
                .unwrap()
                .action,
            ConflictAction::Add
        );
        let outcome = service
            .store_fact(fact("用户", "住址", "北京", None))
            .unwrap();
        assert_eq!(outcome.action, ConflictAction::Supersede);
        let history = service.fact_history("用户", "住址").unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].status, FactStatus::Historical);
        assert_eq!(
            history[1].supersedes_id.as_deref(),
            Some(history[0].id.as_str())
        );
    }

    #[test]
    fn compatible_preferences_are_kept_and_reinforced() {
        let temp = TempDir::new().unwrap();
        let service = service(&temp);
        service
            .store_fact(fact("用户", "偏好", "咖啡", Some("工作")))
            .unwrap();
        assert_eq!(
            service
                .store_fact(fact("用户", "偏好", "茶", Some("工作")))
                .unwrap()
                .action,
            ConflictAction::KeepBoth
        );
        assert_eq!(
            service
                .store_fact(fact("用户", "偏好", "咖啡", Some("工作")))
                .unwrap()
                .action,
            ConflictAction::Reinforce
        );
        assert_eq!(service.search("工作偏好", false, 10).unwrap().len(), 2);
    }

    #[test]
    fn search_explains_association_and_filters_history() {
        let temp = TempDir::new().unwrap();
        let service = service(&temp);
        service
            .store_fact(fact("小明", "住址", "南京", None))
            .unwrap();
        service
            .store_fact(fact("小明", "住址", "北京", None))
            .unwrap();
        let current = service.search("小明 address", false, 10).unwrap();
        assert_eq!(current.len(), 1);
        assert_eq!(current[0].fact.object, "北京");
        assert!(!current[0].reasons.is_empty());
        assert_eq!(service.search("小明 address", true, 10).unwrap().len(), 2);
    }

    #[test]
    fn sensitive_values_are_redacted_before_persistence() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("structured_memory.json");
        let service = MemoryService::open(path.clone()).unwrap();
        service
            .store_fact(fact(
                "用户13800138000",
                "联系方式",
                "test@example.com",
                None,
            ))
            .unwrap();
        let persisted = fs::read_to_string(path).unwrap();
        assert!(!persisted.contains("13800138000"));
        assert!(!persisted.contains("test@example.com"));
        assert!(persisted.contains("REDACTED"));
    }

    #[test]
    fn templates_require_evidence_success_rate_and_review() {
        let temp = TempDir::new().unwrap();
        let service = service(&temp);
        let template_id = service
            .register_template(MemoryTemplate {
                id: String::new(),
                name: "travel-plan".to_string(),
                intent: "plan travel".to_string(),
                preconditions: vec!["destination".to_string()],
                input_schema: "{}".to_string(),
                steps: vec!["search".to_string()],
                output_schema: "{}".to_string(),
                evidence: Vec::new(),
                evidence_episode_ids: Vec::new(),
                success_count: 0,
                failure_count: 0,
                version: 1,
                status: TemplateStatus::Draft,
                human_reviewed: false,
            })
            .unwrap();
        for index in 0..5 {
            service
                .store_fact(fact("模板案例", "执行记录", &format!("案例-{index}"), None))
                .unwrap();
        }
        let episode_ids: Vec<String> = service
            .search("执行记录", false, 10)
            .unwrap()
            .into_iter()
            .map(|hit| hit.fact.source_episode_ids[0].clone())
            .collect();
        assert!(service
            .record_template_result(&template_id, "invented-episode", true)
            .is_err());
        for (index, episode_id) in episode_ids.iter().enumerate() {
            service
                .record_template_result(&template_id, episode_id, index < 4)
                .unwrap();
        }
        assert!(service
            .record_template_result(&template_id, &episode_ids[0], true)
            .is_err());
        service.activate_template(&template_id).unwrap();
        assert_eq!(service.active_templates_for("plan").unwrap().len(), 1);
    }

    #[test]
    fn forgetting_requires_preview_and_matching_confirmation() {
        let temp = TempDir::new().unwrap();
        let service = service(&temp);
        service
            .store_fact(fact("用户", "住址", "南京鼓楼区", None))
            .unwrap();
        service
            .store_fact(fact("用户", "城市偏好", "南京", None))
            .unwrap();
        let plan = service
            .propose_forget("忘掉我的所有住址，但保留城市偏好")
            .unwrap();
        assert_eq!(plan.fact_ids.len(), 1);
        assert!(service.confirm_forget(&plan.id, "wrong-hash").is_err());
        let result = service.confirm_forget(&plan.id, &plan.plan_hash).unwrap();
        assert!(result.verified);
        assert_eq!(result.deleted_facts, 1);
        assert_eq!(service.search("城市偏好", false, 10).unwrap().len(), 1);
        assert!(service.search("address", false, 10).unwrap().is_empty());
    }

    #[test]
    fn vague_or_non_destructive_instructions_are_rejected() {
        let temp = TempDir::new().unwrap();
        let service = service(&temp);
        assert!(service.parse_forget_intent("请记住我的地址").is_err());
        assert!(service.parse_forget_intent("全部忘掉").is_err());
    }

    #[test]
    fn confirmed_forgetting_writes_content_free_audit_record() {
        let temp = TempDir::new().unwrap();
        let service = service(&temp);
        service
            .store_fact(fact("用户", "电话", "13800138000", None))
            .unwrap();
        let plan = service.propose_forget("删除我的电话").unwrap();
        let result = service.confirm_forget(&plan.id, &plan.plan_hash).unwrap();
        assert!(result.verified);
        let audits = service.audit_records().unwrap();
        assert_eq!(audits.len(), 1);
        let serialized = serde_json::to_string(&audits).unwrap();
        assert!(!serialized.contains("13800138000"));
    }

    #[test]
    fn forgetting_physically_removes_current_and_historical_versions() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("structured_memory.json");
        let service = MemoryService::open(path.clone()).unwrap();
        service
            .store_fact(fact("用户", "住址", "南京玄武区", None))
            .unwrap();
        service
            .store_fact(fact("用户", "住址", "北京海淀区", None))
            .unwrap();
        let plan = service.propose_forget("删除我的住址").unwrap();
        assert_eq!(plan.fact_ids.len(), 2);
        let result = service.confirm_forget(&plan.id, &plan.plan_hash).unwrap();
        assert!(result.verified);
        assert!(service.search("address", true, 10).unwrap().is_empty());
        assert!(service.fact_history("用户", "住址").unwrap().is_empty());
        let persisted = fs::read_to_string(path).unwrap();
        assert!(!persisted.contains("南京玄武区"));
        assert!(!persisted.contains("北京海淀区"));
        for fact_id in plan.fact_ids {
            assert!(!persisted.contains(&fact_id));
        }
    }

    #[test]
    fn structural_and_template_fields_cannot_persist_secrets() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("structured_memory.json");
        let service = MemoryService::open(path.clone()).unwrap();
        let mut unsafe_source = fact("用户", "备注", "安全内容", None);
        unsafe_source.source = "password:super-secret".to_string();
        assert!(service.store_fact(unsafe_source).is_err());
        let result = service.register_template(MemoryTemplate {
            id: String::new(),
            name: "联系 test@example.com".to_string(),
            intent: "安全演示".to_string(),
            preconditions: vec!["电话 13800138000".to_string()],
            input_schema: "{}".to_string(),
            steps: vec!["使用 password:super-secret".to_string()],
            output_schema: "{}".to_string(),
            evidence: Vec::new(),
            evidence_episode_ids: Vec::new(),
            success_count: 0,
            failure_count: 0,
            version: 1,
            status: TemplateStatus::Draft,
            human_reviewed: false,
        });
        assert!(result.is_ok());
        let persisted = fs::read_to_string(path).unwrap();
        for secret in ["test@example.com", "13800138000", "super-secret"] {
            assert!(!persisted.contains(secret));
        }
        assert!(persisted.contains("REDACTED"));
    }

    #[test]
    fn forgetting_can_be_limited_to_a_source() {
        let temp = TempDir::new().unwrap();
        let service = service(&temp);
        let mut home = fact("用户", "住址", "南京", Some("家庭"));
        home.source = "session-a".to_string();
        let mut work = fact("用户", "住址", "北京", Some("工作"));
        work.source = "session-b".to_string();
        service.store_fact(home).unwrap();
        service.store_fact(work).unwrap();
        let plan = service
            .propose_forget("删除我的地址 来源:session-a")
            .unwrap();
        assert_eq!(plan.fact_ids.len(), 1);
        service.confirm_forget(&plan.id, &plan.plan_hash).unwrap();
        let remaining = service.search("address", false, 10).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].fact.object, "北京");
    }

    #[test]
    fn state_survives_service_restart() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("structured_memory.json");
        MemoryService::open(path.clone())
            .unwrap()
            .store_fact(fact("用户", "偏好", "安静", None))
            .unwrap();
        let reopened = MemoryService::open(path).unwrap();
        assert_eq!(reopened.search("preference", false, 10).unwrap().len(), 1);
    }

    #[test]
    fn reopening_migrates_legacy_deleted_content_off_disk() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("structured_memory.json");
        let service = MemoryService::open(path.clone()).unwrap();
        service
            .store_fact(fact("用户", "住址", "应被迁移清除的旧地址", None))
            .unwrap();
        drop(service);
        let mut json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        json["facts"][0]["status"] = serde_json::Value::String("deleted".to_string());
        fs::write(&path, serde_json::to_vec_pretty(&json).unwrap()).unwrap();
        let reopened = MemoryService::open(path.clone()).unwrap();
        assert!(reopened.fact_history("用户", "住址").unwrap().is_empty());
        assert!(!fs::read_to_string(path)
            .unwrap()
            .contains("应被迁移清除的旧地址"));
    }

    #[test]
    fn workspace_store_rejects_linked_memory_directory_when_supported() {
        let workspace = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let memory_dir = workspace.path().join(".amadeus");
        #[cfg(unix)]
        let link_result = std::os::unix::fs::symlink(outside.path(), &memory_dir);
        #[cfg(windows)]
        let link_result = std::os::windows::fs::symlink_dir(outside.path(), &memory_dir);
        if link_result.is_err() {
            return;
        }
        assert!(MemoryService::open_in_workspace(workspace.path().to_path_buf()).is_err());
        assert!(!outside.path().join("structured_memory.json").exists());
    }
}
