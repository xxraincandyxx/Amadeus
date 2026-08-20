// @amadeus-header
// summary: Provider-independent workflow graph primitives and deterministic execution runtime.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::workflow
// - type: crate::workflow::NodeId
// - trait: crate::workflow::Node
// - type: crate::workflow::NodeContext
// - type: crate::workflow::Transition
// - type: crate::workflow::Suspension
// - type: crate::workflow::Workflow
// - type: crate::workflow::WorkflowBuilder
// - type: crate::workflow::WorkflowRunner
// - type: crate::workflow::RunStatus
// - type: crate::workflow::SuspendedRun
// - type: crate::workflow::WorkflowError
// uses:
// - protocol: async_trait node execution
// - protocol: thiserror structured failures
// invariants:
// - Workflow entry points and declared node destinations exist before execution.
// - Runner transition limits apply across suspension and resume boundaries.
// - Workflow state remains owned by the caller or returned run status.
// side_effects:
// - Executes caller-provided asynchronous workflow nodes.
// tests:
// - cmd: cargo test -p runtime workflow
// @end-amadeus-header

//! Generic building blocks for composing and executing agent architectures.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

/// Stable identifier for a node within one workflow.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(String);

impl NodeId {
    /// Create a node identifier.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Return the identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn is_valid(&self) -> bool {
        !self.0.trim().is_empty()
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<&str> for NodeId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for NodeId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Dynamically typed error returned by a workflow node.
pub type NodeError = Box<dyn Error + Send + Sync + 'static>;

/// Result returned by a workflow node.
pub type NodeResult<T> = std::result::Result<T, NodeError>;

/// Immutable execution metadata and resources supplied to a node.
pub struct NodeContext<'a, R> {
    resources: &'a R,
    node_id: &'a NodeId,
    transitions_completed: usize,
}

impl<'a, R> NodeContext<'a, R> {
    /// Return the resources shared by this workflow run.
    pub fn resources(&self) -> &'a R {
        self.resources
    }

    /// Return the node currently being executed.
    pub fn node_id(&self) -> &NodeId {
        self.node_id
    }

    /// Return the number of transitions completed before this node execution.
    pub fn transitions_completed(&self) -> usize {
        self.transitions_completed
    }
}

/// External condition that paused workflow execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suspension {
    reason: String,
}

impl Suspension {
    /// Create a suspension with a caller-facing reason.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    /// Return the reason execution was suspended.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// Control-flow decision produced by a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    /// Continue execution at another node.
    Next(NodeId),
    /// Pause and return owned workflow state to the caller.
    Suspend(Suspension),
    /// Complete the workflow successfully.
    Complete,
    /// Fail the workflow with an architecture-defined message.
    Fail(String),
}

impl Transition {
    /// Continue execution at the named node.
    pub fn next(node_id: impl Into<NodeId>) -> Self {
        Self::Next(node_id.into())
    }

    /// Suspend execution with a caller-facing reason.
    pub fn suspend(reason: impl Into<String>) -> Self {
        Self::Suspend(Suspension::new(reason))
    }

    /// Fail execution with an architecture-defined message.
    pub fn fail(message: impl Into<String>) -> Self {
        Self::Fail(message.into())
    }
}

/// Asynchronous unit of agent architecture behavior.
#[async_trait]
pub trait Node<S, R>: Send + Sync {
    /// Execute one node transition over mutable workflow state.
    async fn execute(
        &self,
        context: &mut NodeContext<'_, R>,
        state: &mut S,
    ) -> NodeResult<Transition>;

    /// List every node this node may target with [`Transition::Next`].
    fn destinations(&self) -> Vec<NodeId>;
}

/// Validated collection of nodes forming an executable architecture.
pub struct Workflow<S, R> {
    entry: NodeId,
    nodes: HashMap<NodeId, Arc<dyn Node<S, R>>>,
}

impl<S, R> Workflow<S, R> {
    /// Start building a workflow with the named entry node.
    pub fn builder(entry: impl Into<NodeId>) -> WorkflowBuilder<S, R> {
        WorkflowBuilder::new(entry)
    }

    /// Return the entry node identifier.
    pub fn entry(&self) -> &NodeId {
        &self.entry
    }

    /// Return whether the workflow contains a node identifier.
    pub fn contains(&self, node_id: &NodeId) -> bool {
        self.nodes.contains_key(node_id)
    }

    /// Return the number of nodes in the workflow.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Return whether the workflow contains no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    fn node(&self, node_id: &NodeId) -> Option<&Arc<dyn Node<S, R>>> {
        self.nodes.get(node_id)
    }
}

/// Builder that validates an architecture before it can execute.
pub struct WorkflowBuilder<S, R> {
    entry: NodeId,
    nodes: HashMap<NodeId, Arc<dyn Node<S, R>>>,
}

impl<S, R> WorkflowBuilder<S, R> {
    /// Create a workflow builder with the named entry node.
    pub fn new(entry: impl Into<NodeId>) -> Self {
        Self {
            entry: entry.into(),
            nodes: HashMap::new(),
        }
    }

    /// Add a node under a unique identifier.
    pub fn node<N>(mut self, node_id: impl Into<NodeId>, node: N) -> WorkflowResult<Self>
    where
        N: Node<S, R> + 'static,
    {
        let node_id = node_id.into();
        if !node_id.is_valid() {
            return Err(WorkflowError::InvalidNodeId);
        }
        if self.nodes.contains_key(&node_id) {
            return Err(WorkflowError::DuplicateNode(node_id));
        }
        self.nodes.insert(node_id, Arc::new(node));
        Ok(self)
    }

    /// Validate all declared destinations and produce an executable workflow.
    pub fn build(self) -> WorkflowResult<Workflow<S, R>> {
        if !self.entry.is_valid() {
            return Err(WorkflowError::InvalidNodeId);
        }
        if !self.nodes.contains_key(&self.entry) {
            return Err(WorkflowError::MissingEntry(self.entry));
        }

        for (source, node) in &self.nodes {
            for target in node.destinations() {
                if !target.is_valid() {
                    return Err(WorkflowError::InvalidNodeId);
                }
                if !self.nodes.contains_key(&target) {
                    return Err(WorkflowError::UnknownDestination {
                        node: source.clone(),
                        destination: target,
                    });
                }
            }
        }

        Ok(Workflow {
            entry: self.entry,
            nodes: self.nodes,
        })
    }
}

/// State returned when a workflow pauses for an external condition.
#[derive(Debug)]
pub struct SuspendedRun<S> {
    node_id: NodeId,
    state: S,
    suspension: Suspension,
    transitions_completed: usize,
}

impl<S> SuspendedRun<S> {
    /// Return the node that will execute again when the workflow resumes.
    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    /// Return the suspension details.
    pub fn suspension(&self) -> &Suspension {
        &self.suspension
    }

    /// Return the number of transitions completed before suspension.
    pub fn transitions_completed(&self) -> usize {
        self.transitions_completed
    }

    /// Borrow the suspended workflow state.
    pub fn state(&self) -> &S {
        &self.state
    }

    /// Mutably borrow the suspended workflow state before resuming.
    pub fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }

    /// Consume the suspension and return its workflow state.
    pub fn into_state(self) -> S {
        self.state
    }
}

/// Terminal status returned by the workflow runner.
#[derive(Debug)]
pub enum RunStatus<S> {
    /// Workflow completed successfully with its final state.
    Completed {
        /// Final workflow state.
        state: S,
        /// Total transitions completed by the run.
        transitions_completed: usize,
    },
    /// Workflow paused and can be resumed from the returned checkpoint.
    Suspended(SuspendedRun<S>),
}

/// Deterministic executor for a validated workflow.
#[derive(Debug, Clone)]
pub struct WorkflowRunner {
    max_transitions: usize,
}

impl Default for WorkflowRunner {
    fn default() -> Self {
        Self {
            max_transitions: 1_024,
        }
    }
}

impl WorkflowRunner {
    /// Create a runner with a maximum number of transitions per logical run.
    pub fn new(max_transitions: usize) -> Self {
        Self { max_transitions }
    }

    /// Execute a workflow from its entry node.
    pub async fn run<S, R>(
        &self,
        workflow: &Workflow<S, R>,
        resources: &R,
        state: S,
    ) -> WorkflowResult<RunStatus<S>> {
        self.run_from(workflow, resources, workflow.entry.clone(), state, 0)
            .await
    }

    /// Resume a workflow from a previously returned suspension.
    pub async fn resume<S, R>(
        &self,
        workflow: &Workflow<S, R>,
        resources: &R,
        suspended: SuspendedRun<S>,
    ) -> WorkflowResult<RunStatus<S>> {
        self.run_from(
            workflow,
            resources,
            suspended.node_id,
            suspended.state,
            suspended.transitions_completed,
        )
        .await
    }

    async fn run_from<S, R>(
        &self,
        workflow: &Workflow<S, R>,
        resources: &R,
        mut node_id: NodeId,
        mut state: S,
        mut transitions_completed: usize,
    ) -> WorkflowResult<RunStatus<S>> {
        loop {
            if transitions_completed >= self.max_transitions {
                return Err(WorkflowError::TransitionLimitExceeded {
                    limit: self.max_transitions,
                    node: node_id,
                });
            }

            let node = workflow
                .node(&node_id)
                .ok_or_else(|| WorkflowError::UnknownNode(node_id.clone()))?;
            let mut context = NodeContext {
                resources,
                node_id: &node_id,
                transitions_completed,
            };
            let transition = node
                .execute(&mut context, &mut state)
                .await
                .map_err(|source| WorkflowError::NodeExecution {
                    node: node_id.clone(),
                    source,
                })?;
            transitions_completed += 1;

            match transition {
                Transition::Next(target) => {
                    if !workflow.contains(&target) {
                        return Err(WorkflowError::UnknownDestination {
                            node: node_id,
                            destination: target,
                        });
                    }
                    node_id = target;
                }
                Transition::Suspend(suspension) => {
                    return Ok(RunStatus::Suspended(SuspendedRun {
                        node_id,
                        state,
                        suspension,
                        transitions_completed,
                    }));
                }
                Transition::Complete => {
                    return Ok(RunStatus::Completed {
                        state,
                        transitions_completed,
                    });
                }
                Transition::Fail(message) => {
                    return Err(WorkflowError::ArchitectureFailure {
                        node: node_id,
                        message,
                    });
                }
            }
        }
    }
}

/// Failure produced while constructing or executing a workflow.
#[derive(Debug, Error)]
pub enum WorkflowError {
    /// A node identifier was empty or whitespace-only.
    #[error("Workflow node identifiers cannot be empty")]
    InvalidNodeId,
    /// The workflow entry does not identify a registered node.
    #[error("Workflow entry node '{0}' is not registered")]
    MissingEntry(NodeId),
    /// A node identifier was registered more than once.
    #[error("Workflow node '{0}' is registered more than once")]
    DuplicateNode(NodeId),
    /// An execution checkpoint refers to a node absent from this workflow.
    #[error("Workflow node '{0}' is not registered")]
    UnknownNode(NodeId),
    /// A node declared or returned a destination absent from this workflow.
    #[error("Workflow node '{node}' targets unknown node '{destination}'")]
    UnknownDestination {
        /// Node producing the transition.
        node: NodeId,
        /// Missing transition target.
        destination: NodeId,
    },
    /// A node returned an execution error.
    #[error("Workflow node '{node}' failed: {source}")]
    NodeExecution {
        /// Node that failed.
        node: NodeId,
        /// Original node error.
        #[source]
        source: NodeError,
    },
    /// A node deliberately failed the architecture.
    #[error("Workflow node '{node}' failed: {message}")]
    ArchitectureFailure {
        /// Node that failed.
        node: NodeId,
        /// Architecture-defined failure message.
        message: String,
    },
    /// The runner exhausted its transition budget.
    #[error("Workflow exceeded its transition limit of {limit} before node '{node}'")]
    TransitionLimitExceeded {
        /// Configured transition limit.
        limit: usize,
        /// Next node that would have executed.
        node: NodeId,
    },
}

/// Result returned by workflow construction and execution operations.
pub type WorkflowResult<T> = std::result::Result<T, WorkflowError>;

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    struct IncrementNode {
        next: Option<NodeId>,
    }

    #[async_trait]
    impl<R: Sync> Node<usize, R> for IncrementNode {
        async fn execute(
            &self,
            _context: &mut NodeContext<'_, R>,
            state: &mut usize,
        ) -> NodeResult<Transition> {
            *state += 1;
            Ok(match &self.next {
                Some(next) => Transition::Next(next.clone()),
                None => Transition::Complete,
            })
        }

        fn destinations(&self) -> Vec<NodeId> {
            self.next.iter().cloned().collect()
        }
    }

    struct BranchNode;

    #[async_trait]
    impl Node<bool, ()> for BranchNode {
        async fn execute(
            &self,
            _context: &mut NodeContext<'_, ()>,
            state: &mut bool,
        ) -> NodeResult<Transition> {
            Ok(Transition::next(if *state { "yes" } else { "no" }))
        }

        fn destinations(&self) -> Vec<NodeId> {
            vec![NodeId::from("yes"), NodeId::from("no")]
        }
    }

    struct TerminalNode;

    #[async_trait]
    impl<S: Send, R: Sync> Node<S, R> for TerminalNode {
        async fn execute(
            &self,
            _context: &mut NodeContext<'_, R>,
            _state: &mut S,
        ) -> NodeResult<Transition> {
            Ok(Transition::Complete)
        }

        fn destinations(&self) -> Vec<NodeId> {
            Vec::new()
        }
    }

    struct GateNode;

    #[async_trait]
    impl Node<usize, AtomicBool> for GateNode {
        async fn execute(
            &self,
            context: &mut NodeContext<'_, AtomicBool>,
            state: &mut usize,
        ) -> NodeResult<Transition> {
            *state += 1;
            if context.resources().load(Ordering::SeqCst) {
                Ok(Transition::Complete)
            } else {
                Ok(Transition::suspend("approval required"))
            }
        }

        fn destinations(&self) -> Vec<NodeId> {
            Vec::new()
        }
    }

    struct LoopNode;

    #[async_trait]
    impl Node<(), ()> for LoopNode {
        async fn execute(
            &self,
            _context: &mut NodeContext<'_, ()>,
            _state: &mut (),
        ) -> NodeResult<Transition> {
            Ok(Transition::next("loop"))
        }

        fn destinations(&self) -> Vec<NodeId> {
            vec![NodeId::from("loop")]
        }
    }

    #[tokio::test]
    async fn executes_linear_workflow() {
        let workflow = Workflow::builder("first")
            .node(
                "first",
                IncrementNode {
                    next: Some(NodeId::from("second")),
                },
            )
            .and_then(|builder| builder.node("second", IncrementNode { next: None }))
            .and_then(WorkflowBuilder::build)
            .expect("linear workflow");

        let status = WorkflowRunner::default()
            .run(&workflow, &(), 0)
            .await
            .expect("workflow run");

        match status {
            RunStatus::Completed {
                state,
                transitions_completed,
            } => {
                assert_eq!(state, 2);
                assert_eq!(transitions_completed, 2);
            }
            RunStatus::Suspended(_) => panic!("workflow unexpectedly suspended"),
        }
    }

    #[tokio::test]
    async fn executes_state_driven_branch() {
        let workflow = Workflow::builder("branch")
            .node("branch", BranchNode)
            .and_then(|builder| builder.node("yes", TerminalNode))
            .and_then(|builder| builder.node("no", TerminalNode))
            .and_then(WorkflowBuilder::build)
            .expect("branch workflow");

        let status = WorkflowRunner::default()
            .run(&workflow, &(), true)
            .await
            .expect("workflow run");

        assert!(matches!(
            status,
            RunStatus::Completed {
                state: true,
                transitions_completed: 2
            }
        ));
    }

    #[tokio::test]
    async fn resumes_suspended_workflow_with_owned_state() {
        let workflow = Workflow::builder("gate")
            .node("gate", GateNode)
            .and_then(WorkflowBuilder::build)
            .expect("gate workflow");
        let resources = AtomicBool::new(false);

        let suspended = match WorkflowRunner::default()
            .run(&workflow, &resources, 0)
            .await
            .expect("suspended run")
        {
            RunStatus::Suspended(suspended) => suspended,
            RunStatus::Completed { .. } => panic!("workflow unexpectedly completed"),
        };

        assert_eq!(suspended.node_id().as_str(), "gate");
        assert_eq!(suspended.suspension().reason(), "approval required");
        assert_eq!(*suspended.state(), 1);
        resources.store(true, Ordering::SeqCst);

        let status = WorkflowRunner::default()
            .resume(&workflow, &resources, suspended)
            .await
            .expect("resumed run");

        assert!(matches!(
            status,
            RunStatus::Completed {
                state: 2,
                transitions_completed: 2
            }
        ));
    }

    #[test]
    fn rejects_unknown_declared_destination() {
        let result = Workflow::<usize, ()>::builder("first")
            .node(
                "first",
                IncrementNode {
                    next: Some(NodeId::from("missing")),
                },
            )
            .and_then(WorkflowBuilder::build);

        assert!(matches!(
            result,
            Err(WorkflowError::UnknownDestination { node, destination })
                if node.as_str() == "first" && destination.as_str() == "missing"
        ));
    }

    #[tokio::test]
    async fn stops_unbounded_workflow_at_transition_limit() {
        let workflow = Workflow::builder("loop")
            .node("loop", LoopNode)
            .and_then(WorkflowBuilder::build)
            .expect("loop workflow");

        let error = WorkflowRunner::new(3)
            .run(&workflow, &(), ())
            .await
            .expect_err("transition limit");

        assert!(matches!(
            error,
            WorkflowError::TransitionLimitExceeded { limit: 3, node }
                if node.as_str() == "loop"
        ));
    }
}
