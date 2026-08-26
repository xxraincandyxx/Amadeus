// @amadeus-header
// summary: Compiles serialized agent architecture manifests into executable workflows.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::architecture
// - type: crate::architecture::AgentArchitectureManifest
// - type: crate::architecture::ArchitectureNodeKind
// - trait: crate::architecture::ArchitectureNodeExecutor
// - fn: crate::architecture::compile_architecture
// uses:
// - module: crate::workflow
// - protocol: serde serialization
// invariants:
// - Every compiled edge targets a declared node.
// - Branching nodes select one of their declared edge labels.
// - Approval nodes advance only after consuming a one-shot approval marker.
// - Output nodes terminate the workflow.
// side_effects: none
// tests:
// - cmd: cargo test -p runtime architecture
// @end-amadeus-header

//! Serializable agent architectures and their provider-independent compiler.

use std::collections::{BTreeMap, HashSet};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::workflow::{Node, NodeContext, NodeError, NodeId, NodeResult, Transition, Workflow};

fn default_schema_version() -> u32 {
    2
}

fn default_max_transitions() -> usize {
    1_024
}

/// Serialized control-flow graph used to construct one agent runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentArchitectureManifest {
    /// Manifest format version.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Manifest discriminator.
    #[serde(default)]
    pub kind: String,
    /// Stable architecture identifier.
    pub id: String,
    /// User-visible architecture name.
    pub name: String,
    /// Optional preset lineage.
    #[serde(default)]
    pub preset: String,
    /// Node where execution starts.
    pub entry_node_id: String,
    /// Maximum node executions in one run.
    #[serde(default = "default_max_transitions")]
    pub max_transitions: usize,
    /// Declared architecture nodes.
    pub nodes: Vec<ArchitectureNode>,
    /// Directed, labeled transitions between nodes.
    pub edges: Vec<ArchitectureEdge>,
}

/// One node declared by an agent architecture manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureNode {
    /// Stable identifier unique within the manifest.
    pub id: String,
    /// Runtime behavior and user-editable configuration.
    pub data: ArchitectureNodeData,
}

/// Runtime behavior and settings for an architecture node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureNodeData {
    /// Built-in behavior selected for this node.
    pub kind: ArchitectureNodeKind,
    /// User-visible node label.
    #[serde(default)]
    pub label: String,
    /// Optional user-authored behavior description.
    #[serde(default)]
    pub description: String,
    /// Kind-specific settings such as instructions, criteria, and capabilities.
    #[serde(flatten)]
    pub settings: BTreeMap<String, Value>,
}

/// Directed transition declared by an architecture manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureEdge {
    /// Stable edge identifier.
    pub id: String,
    /// Source node identifier.
    pub source: String,
    /// Target node identifier.
    pub target: String,
    /// Branch label used by routing nodes.
    #[serde(default = "default_edge_label")]
    pub label: String,
}

fn default_edge_label() -> String {
    "next".to_string()
}

/// Built-in node behaviors supported by the architecture compiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArchitectureNodeKind {
    /// Initialize workflow state.
    Input,
    /// Expose existing state to the next reasoning node.
    Observe,
    /// Ask the model to reason about the next action.
    Reason,
    /// Run a tool-capable model phase.
    Act,
    /// Produce or revise a task plan.
    Plan,
    /// Execute the current plan step with tools.
    Execute,
    /// Select one declared transition label.
    Route,
    /// Critique the current candidate result.
    Critique,
    /// Revise the candidate using critique feedback.
    Revise,
    /// Delegate work to a specialist agent.
    Delegate,
    /// Review delegated work.
    Review,
    /// Pause for an external approval decision.
    Approval,
    /// Synthesize accumulated node outputs.
    Synthesize,
    /// Complete with the current output.
    Output,
}

/// Mutable state carried through one compiled architecture run.
#[derive(Debug, Clone)]
pub struct ArchitectureRunState {
    /// Original user request.
    pub input: String,
    /// Most recent semantic node output.
    pub current_output: String,
    /// Outputs indexed by node identifier.
    pub outputs: BTreeMap<String, String>,
}

impl ArchitectureRunState {
    /// Create state for a new user request.
    pub fn new(input: impl Into<String>) -> Self {
        Self {
            input: input.into(),
            current_output: String::new(),
            outputs: BTreeMap::new(),
        }
    }

    /// Allow one suspended approval node to continue when the run resumes.
    pub fn approve_node(&mut self, node_id: &str) {
        self.outputs.insert(node_id.to_string(), String::new());
    }

    fn take_node_approval(&mut self, node_id: &str) -> bool {
        self.outputs.remove(node_id).is_some()
    }
}

/// Result returned by a model, tool, route, or delegation node implementation.
#[derive(Debug, Clone, Default)]
pub struct ArchitectureNodeOutput {
    /// Semantic output retained in workflow state.
    pub output: String,
    /// Edge label selected when the node has multiple outgoing transitions.
    pub transition: Option<String>,
}

/// Provider-specific behavior used by executable built-in nodes.
#[async_trait]
pub trait ArchitectureNodeExecutor: Send + Sync {
    /// Execute a model, tool, route, or delegation node.
    async fn execute(
        &self,
        node: &ArchitectureNode,
        state: &ArchitectureRunState,
        outgoing: &[ArchitectureEdge],
    ) -> NodeResult<ArchitectureNodeOutput>;
}

/// Resources supplied to every compiled architecture node.
pub struct ArchitectureResources<E> {
    executor: E,
}

impl<E> ArchitectureResources<E> {
    /// Bind a provider-specific executor to a compiled architecture.
    pub fn new(executor: E) -> Self {
        Self { executor }
    }
}

struct BuiltInNode {
    node: ArchitectureNode,
    outgoing: Vec<ArchitectureEdge>,
}

#[async_trait]
impl<E> Node<ArchitectureRunState, ArchitectureResources<E>> for BuiltInNode
where
    E: ArchitectureNodeExecutor,
{
    async fn execute(
        &self,
        context: &mut NodeContext<'_, ArchitectureResources<E>>,
        state: &mut ArchitectureRunState,
    ) -> NodeResult<Transition> {
        match self.node.data.kind {
            ArchitectureNodeKind::Output => return Ok(Transition::Complete),
            ArchitectureNodeKind::Approval => {
                if state.take_node_approval(&self.node.id) {
                    return self.select_transition(None);
                }
                let reason = setting(&self.node, "reason").unwrap_or("Approval required");
                return Ok(Transition::suspend(reason));
            }
            ArchitectureNodeKind::Input | ArchitectureNodeKind::Observe => {
                return self.select_transition(None);
            }
            _ => {}
        }

        let result = context
            .resources()
            .executor
            .execute(&self.node, state, &self.outgoing)
            .await?;
        if !result.output.is_empty() {
            state.current_output.clone_from(&result.output);
            state.outputs.insert(self.node.id.clone(), result.output);
        }
        self.select_transition(result.transition.as_deref())
    }

    fn destinations(&self) -> Vec<NodeId> {
        self.outgoing
            .iter()
            .map(|edge| NodeId::new(edge.target.clone()))
            .collect()
    }
}

impl BuiltInNode {
    fn select_transition(&self, selected: Option<&str>) -> NodeResult<Transition> {
        if self.outgoing.len() == 1 {
            return Ok(Transition::next(self.outgoing[0].target.clone()));
        }
        if self.outgoing.is_empty() {
            return Ok(Transition::fail(format!(
                "Node '{}' has no outgoing transition",
                self.node.id
            )));
        }

        let selected = selected.ok_or_else(|| {
            boxed_error(format!(
                "Node '{}' must select one of its outgoing transition labels",
                self.node.id
            ))
        })?;
        let edge = self
            .outgoing
            .iter()
            .find(|edge| edge.label.eq_ignore_ascii_case(selected.trim()))
            .ok_or_else(|| {
                boxed_error(format!(
                    "Node '{}' selected undeclared transition '{}'",
                    self.node.id, selected
                ))
            })?;
        Ok(Transition::next(edge.target.clone()))
    }
}

fn boxed_error(message: String) -> NodeError {
    Box::new(ArchitectureExecutionError(message))
}

/// Compile a serialized manifest into an executable typed workflow.
pub fn compile_architecture<E>(
    manifest: &AgentArchitectureManifest,
) -> Result<Workflow<ArchitectureRunState, ArchitectureResources<E>>, ArchitectureCompileError>
where
    E: ArchitectureNodeExecutor + 'static,
{
    validate_architecture(manifest)?;
    let mut builder = Workflow::builder(manifest.entry_node_id.clone());
    for node in &manifest.nodes {
        let outgoing = manifest
            .edges
            .iter()
            .filter(|edge| edge.source == node.id)
            .cloned()
            .collect();
        builder = builder.node(
            node.id.clone(),
            BuiltInNode {
                node: node.clone(),
                outgoing,
            },
        )?;
    }
    Ok(builder.build()?)
}

/// Validate structural and version invariants for an architecture manifest.
pub fn validate_architecture(
    manifest: &AgentArchitectureManifest,
) -> Result<(), ArchitectureCompileError> {
    if manifest.schema_version != 2 {
        return Err(ArchitectureCompileError::UnsupportedSchema(
            manifest.schema_version,
        ));
    }
    if manifest.kind != "agent-architecture" {
        return Err(ArchitectureCompileError::InvalidKind(manifest.kind.clone()));
    }
    if manifest.max_transitions == 0 {
        return Err(ArchitectureCompileError::InvalidTransitionLimit);
    }
    let node_ids = manifest
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    if node_ids.len() != manifest.nodes.len() {
        return Err(ArchitectureCompileError::DuplicateNode);
    }
    if !node_ids.contains(manifest.entry_node_id.as_str()) {
        return Err(ArchitectureCompileError::MissingEntry(
            manifest.entry_node_id.clone(),
        ));
    }
    for edge in &manifest.edges {
        if !node_ids.contains(edge.source.as_str()) || !node_ids.contains(edge.target.as_str()) {
            return Err(ArchitectureCompileError::DanglingEdge(edge.id.clone()));
        }
    }
    if !manifest
        .nodes
        .iter()
        .any(|node| node.data.kind == ArchitectureNodeKind::Output)
    {
        return Err(ArchitectureCompileError::MissingOutput);
    }
    Ok(())
}

/// Read a string setting from a manifest node.
pub fn setting<'a>(node: &'a ArchitectureNode, key: &str) -> Option<&'a str> {
    node.data.settings.get(key).and_then(Value::as_str)
}

#[derive(Debug, Error)]
struct ArchitectureExecutionError(String);

impl std::fmt::Display for ArchitectureExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Failure returned while validating or compiling an architecture manifest.
#[derive(Debug, Error)]
pub enum ArchitectureCompileError {
    /// The manifest uses an unsupported schema version.
    #[error("Unsupported agent architecture schema version {0}")]
    UnsupportedSchema(u32),
    /// The manifest discriminator is invalid.
    #[error("Invalid agent architecture kind '{0}'")]
    InvalidKind(String),
    /// The transition budget must be positive.
    #[error("Agent architecture maxTransitions must be greater than zero")]
    InvalidTransitionLimit,
    /// Two nodes use the same identifier.
    #[error("Agent architecture contains duplicate node identifiers")]
    DuplicateNode,
    /// The entry node does not exist.
    #[error("Agent architecture entry node '{0}' does not exist")]
    MissingEntry(String),
    /// An edge references an unknown node.
    #[error("Agent architecture edge '{0}' references an unknown node")]
    DanglingEdge(String),
    /// No terminal output node is declared.
    #[error("Agent architecture must contain an output node")]
    MissingOutput,
    /// The generic workflow rejected the compiled graph.
    #[error(transparent)]
    Workflow(#[from] crate::workflow::WorkflowError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::{RunStatus, WorkflowRunner};

    struct ScriptedExecutor;

    #[async_trait]
    impl ArchitectureNodeExecutor for ScriptedExecutor {
        async fn execute(
            &self,
            node: &ArchitectureNode,
            _state: &ArchitectureRunState,
            _outgoing: &[ArchitectureEdge],
        ) -> NodeResult<ArchitectureNodeOutput> {
            Ok(ArchitectureNodeOutput {
                output: node.data.label.clone(),
                transition: (node.data.kind == ArchitectureNodeKind::Route)
                    .then(|| "done".to_string()),
            })
        }
    }

    fn node(id: &str, kind: ArchitectureNodeKind) -> ArchitectureNode {
        ArchitectureNode {
            id: id.to_string(),
            data: ArchitectureNodeData {
                kind,
                label: id.to_string(),
                description: String::new(),
                settings: BTreeMap::new(),
            },
        }
    }

    fn edge(id: &str, source: &str, target: &str, label: &str) -> ArchitectureEdge {
        ArchitectureEdge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            label: label.to_string(),
        }
    }

    #[tokio::test]
    async fn compiles_and_runs_a_branching_manifest() {
        let manifest = AgentArchitectureManifest {
            schema_version: 2,
            kind: "agent-architecture".to_string(),
            id: "test".to_string(),
            name: "Test".to_string(),
            preset: "reflection".to_string(),
            entry_node_id: "input".to_string(),
            max_transitions: 8,
            nodes: vec![
                node("input", ArchitectureNodeKind::Input),
                node("draft", ArchitectureNodeKind::Reason),
                node("route", ArchitectureNodeKind::Route),
                node("revise", ArchitectureNodeKind::Revise),
                node("output", ArchitectureNodeKind::Output),
            ],
            edges: vec![
                edge("e1", "input", "draft", "next"),
                edge("e2", "draft", "route", "next"),
                edge("e3", "route", "revise", "revise"),
                edge("e4", "route", "output", "done"),
                edge("e5", "revise", "route", "next"),
            ],
        };
        let workflow = compile_architecture::<ScriptedExecutor>(&manifest).expect("compile");
        let result = WorkflowRunner::new(manifest.max_transitions)
            .run(
                &workflow,
                &ArchitectureResources::new(ScriptedExecutor),
                ArchitectureRunState::new("request"),
            )
            .await
            .expect("run");

        match result {
            RunStatus::Completed { state, .. } => assert_eq!(state.current_output, "route"),
            RunStatus::Suspended(_) => panic!("unexpected suspension"),
        }
    }

    #[tokio::test]
    async fn resumes_an_approved_manifest_node_once() {
        let manifest = AgentArchitectureManifest {
            schema_version: 2,
            kind: "agent-architecture".to_string(),
            id: "approval-test".to_string(),
            name: "Approval test".to_string(),
            preset: String::new(),
            entry_node_id: "input".to_string(),
            max_transitions: 8,
            nodes: vec![
                node("input", ArchitectureNodeKind::Input),
                node("approval", ArchitectureNodeKind::Approval),
                node("output", ArchitectureNodeKind::Output),
            ],
            edges: vec![
                edge("e1", "input", "approval", "next"),
                edge("e2", "approval", "output", "approved"),
            ],
        };
        let workflow = compile_architecture::<ScriptedExecutor>(&manifest).expect("compile");
        let resources = ArchitectureResources::new(ScriptedExecutor);
        let runner = WorkflowRunner::new(manifest.max_transitions);
        let result = runner
            .run(&workflow, &resources, ArchitectureRunState::new("request"))
            .await
            .expect("run");
        let mut suspended = match result {
            RunStatus::Suspended(suspended) => suspended,
            RunStatus::Completed { .. } => panic!("expected approval suspension"),
        };
        assert_eq!(suspended.node_id().as_str(), "approval");
        suspended.state_mut().approve_node("approval");

        let resumed = runner
            .resume(&workflow, &resources, suspended)
            .await
            .expect("resume");

        match resumed {
            RunStatus::Completed { state, .. } => {
                assert!(!state.outputs.contains_key("approval"));
            }
            RunStatus::Suspended(_) => panic!("approval should be consumed exactly once"),
        }
    }

    #[test]
    fn rejects_dangling_edges() {
        let manifest = AgentArchitectureManifest {
            schema_version: 2,
            kind: "agent-architecture".to_string(),
            id: "broken".to_string(),
            name: "Broken".to_string(),
            preset: String::new(),
            entry_node_id: "input".to_string(),
            max_transitions: 8,
            nodes: vec![
                node("input", ArchitectureNodeKind::Input),
                node("output", ArchitectureNodeKind::Output),
            ],
            edges: vec![edge("bad", "input", "missing", "next")],
        };

        assert!(matches!(
            validate_architecture(&manifest),
            Err(ArchitectureCompileError::DanglingEdge(edge)) if edge == "bad"
        ));
    }
}
