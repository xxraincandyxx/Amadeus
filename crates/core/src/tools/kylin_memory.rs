// @amadeus-header
// summary: LLM tool for structured memory, templates, privacy inspection, and confirmed forgetting.
// layer: tools
// status: active
// feature_flags: none
// provides:
// - tool: kylin_memory
// - type: crate::tools::kylin_memory::KylinMemoryTool
// uses:
// - type: amadeus_memory_service::MemoryService
// - type: amadeus_privacy::SensitiveDataDetector
// invariants:
// - Forget execution requires a plan id and hash returned by forget_preview.
// - Agent policy requires one-time human approval for forget_confirm and activate_template.
// - Existing legacy memory tool operations remain independent and compatible.
// side_effects:
// - Writes structured memory state under .amadeus.
// tests:
// - tests/kylin_memory_indicators_test.rs
// @end-amadeus-header

//! Tool boundary for the KylinMem indicator-three and indicator-five workflows.

use std::path::PathBuf;
use std::sync::Arc;

use amadeus_memory_domain::{FactInput, MemoryTemplate, TemplateStatus};
use amadeus_memory_service::{MemoryService, MemoryServiceError};
use amadeus_privacy::SensitiveDataDetector;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

use crate::error::{AgentError, Result};
use crate::tools::tool_trait::Tool;

#[derive(Debug, Deserialize)]
struct KylinMemoryInput {
    operation: String,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    predicate: Option<String>,
    #[serde(default)]
    object: Option<String>,
    #[serde(default)]
    context: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    confidence: Option<f32>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    include_history: Option<bool>,
    #[serde(default)]
    top_k: Option<usize>,
    #[serde(default)]
    instruction: Option<String>,
    #[serde(default)]
    plan_id: Option<String>,
    #[serde(default)]
    plan_hash: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    template_id: Option<String>,
    #[serde(default)]
    template_name: Option<String>,
    #[serde(default)]
    intent: Option<String>,
    #[serde(default)]
    preconditions: Option<Vec<String>>,
    #[serde(default)]
    steps: Option<Vec<String>>,
    #[serde(default)]
    input_schema: Option<String>,
    #[serde(default)]
    output_schema: Option<String>,
    #[serde(default)]
    episode_id: Option<String>,
    #[serde(default)]
    success: Option<bool>,
}

pub struct KylinMemoryTool {
    service: Arc<MemoryService>,
    detector: SensitiveDataDetector,
}

impl KylinMemoryTool {
    pub fn open(path: PathBuf) -> Result<Self> {
        let service = MemoryService::open(path).map_err(service_error)?;
        Ok(Self {
            service: Arc::new(service),
            detector: SensitiveDataDetector::new(),
        })
    }

    pub fn open_in_workspace(workdir: PathBuf) -> Result<Self> {
        let service = MemoryService::open_in_workspace(workdir).map_err(service_error)?;
        Ok(Self {
            service: Arc::new(service),
            detector: SensitiveDataDetector::new(),
        })
    }

    pub fn from_service(service: Arc<MemoryService>) -> Self {
        Self {
            service,
            detector: SensitiveDataDetector::new(),
        }
    }

    fn execute_operation(&self, input: KylinMemoryInput) -> Result<String> {
        match input.operation.as_str() {
            "store_fact" => {
                let outcome = self
                    .service
                    .store_fact(FactInput {
                        subject: required(input.subject, "subject", "store_fact")?,
                        predicate: required(input.predicate, "predicate", "store_fact")?,
                        object: required(input.object, "object", "store_fact")?,
                        context: input.context,
                        source: input.source.unwrap_or_else(|| "agent".to_string()),
                        confidence: input.confidence.unwrap_or(0.8),
                    })
                    .map_err(service_error)?;
                json(&outcome)
            }
            "search" => json(
                &self
                    .service
                    .search(
                        &required(input.query, "query", "search")?,
                        input.include_history.unwrap_or(false),
                        input.top_k.unwrap_or(10),
                    )
                    .map_err(service_error)?,
            ),
            "register_template" => {
                let template = MemoryTemplate {
                    id: String::new(),
                    name: required(input.template_name, "template_name", "register_template")?,
                    intent: required(input.intent, "intent", "register_template")?,
                    preconditions: input.preconditions.unwrap_or_default(),
                    input_schema: input.input_schema.unwrap_or_else(|| "{}".to_string()),
                    steps: required(input.steps, "steps", "register_template")?,
                    output_schema: input.output_schema.unwrap_or_else(|| "{}".to_string()),
                    evidence: Vec::new(),
                    evidence_episode_ids: Vec::new(),
                    success_count: 0,
                    failure_count: 0,
                    version: 1,
                    status: TemplateStatus::Draft,
                    human_reviewed: false,
                };
                json(
                    &serde_json::json!({ "template_id": self.service.register_template(template).map_err(service_error)? }),
                )
            }
            "record_template_result" => {
                self.service
                    .record_template_result(
                        &required(input.template_id, "template_id", "record_template_result")?,
                        &required(input.episode_id, "episode_id", "record_template_result")?,
                        input.success.unwrap_or(false),
                    )
                    .map_err(service_error)?;
                json(&serde_json::json!({ "recorded": true }))
            }
            "activate_template" => {
                self.service
                    .activate_template(&required(
                        input.template_id,
                        "template_id",
                        "activate_template",
                    )?)
                    .map_err(service_error)?;
                json(&serde_json::json!({ "activated": true }))
            }
            "find_templates" => json(
                &self
                    .service
                    .active_templates_for(&required(input.intent, "intent", "find_templates")?)
                    .map_err(service_error)?,
            ),
            "privacy_scan" => {
                let content = required(input.content, "content", "privacy_scan")?;
                let (redacted, spans) = self.detector.redact(&content);
                json(&serde_json::json!({ "redacted": redacted, "detections": spans }))
            }
            "forget_preview" => json(
                &self
                    .service
                    .propose_forget(&required(
                        input.instruction,
                        "instruction",
                        "forget_preview",
                    )?)
                    .map_err(service_error)?,
            ),
            "forget_confirm" => json(
                &self
                    .service
                    .confirm_forget(
                        &required(input.plan_id, "plan_id", "forget_confirm")?,
                        &required(input.plan_hash, "plan_hash", "forget_confirm")?,
                    )
                    .map_err(service_error)?,
            ),
            other => Err(input_error(format!("unknown operation '{other}'"))),
        }
    }
}

#[async_trait]
impl Tool for KylinMemoryTool {
    fn name(&self) -> &'static str {
        "kylin_memory"
    }

    fn schema(&self) -> &'static Value {
        static SCHEMA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
        SCHEMA.get_or_init(|| serde_json::json!({
            "name": "kylin_memory",
            "description": "Structured versioned memory with conflict handling, association search, reviewed templates, sensitive-data redaction, and two-step precise forgetting. Always call forget_preview before forget_confirm and show the preview to the user.",
            "parameters": {
                "type": "object",
                "properties": {
                    "operation": { "type": "string", "enum": ["store_fact", "search", "register_template", "record_template_result", "activate_template", "find_templates", "privacy_scan", "forget_preview", "forget_confirm"] },
                    "subject": { "type": "string" }, "predicate": { "type": "string" }, "object": { "type": "string" },
                    "context": { "type": "string" }, "source": { "type": "string" },
                    "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
                    "query": { "type": "string" }, "include_history": { "type": "boolean" },
                    "top_k": { "type": "integer", "minimum": 1, "maximum": 100 },
                    "instruction": { "type": "string" }, "plan_id": { "type": "string" }, "plan_hash": { "type": "string" },
                    "content": { "type": "string" }, "template_id": { "type": "string" }, "template_name": { "type": "string" },
                    "intent": { "type": "string" }, "preconditions": { "type": "array", "items": { "type": "string" } },
                    "steps": { "type": "array", "items": { "type": "string" } }, "input_schema": { "type": "string" },
                    "output_schema": { "type": "string" }, "episode_id": { "type": "string" }, "success": { "type": "boolean" },
                },
                "required": ["operation"]
            }
        }))
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let parsed =
            serde_json::from_value(input).map_err(|error| input_error(error.to_string()))?;
        self.execute_operation(parsed)
    }
}

fn required<T>(value: Option<T>, field: &str, operation: &str) -> Result<T> {
    value.ok_or_else(|| input_error(format!("{field} is required for {operation}")))
}

fn json<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_string_pretty(value).map_err(|error| {
        AgentError::InvalidResponse(format!("kylin_memory serialization: {error}"))
    })
}

fn input_error(reason: String) -> AgentError {
    AgentError::ToolInput {
        tool: "kylin_memory".to_string(),
        reason,
    }
}
fn service_error(error: MemoryServiceError) -> AgentError {
    input_error(error.to_string())
}
