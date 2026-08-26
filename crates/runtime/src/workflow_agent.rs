// @amadeus-header
// summary: Executable agent instances that bind workflow architectures to identity and resources.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::workflow_agent
// - type: crate::workflow_agent::WorkflowAgentIdentity
// - type: crate::workflow_agent::AgentRunId
// - type: crate::workflow_agent::WorkflowAgent
// - type: crate::workflow_agent::WorkflowAgentRun
// - type: crate::workflow_agent::WorkflowAgentRunStatus
// - type: crate::workflow_agent::WorkflowAgentCheckpoint
// - type: crate::workflow_agent::WorkflowAgentRegistry
// - type: crate::workflow_agent::WorkflowAgentError
// uses:
// - module: crate::workflow
// - module: amadeus_ids
// - protocol: thiserror structured failures
// invariants:
// - Every workflow agent owns exactly one workflow architecture and one resource set.
// - Suspended checkpoints can only resume through the agent that created them.
// - Agent registries reject duplicate identifiers and route runs by stable agent identity.
// side_effects:
// - Executes workflow nodes through the bound workflow runner.
// tests:
// - cmd: cargo test -p runtime workflow_agent
// @end-amadeus-header

//! Agent instances and registries built on the provider-independent workflow kernel.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use amadeus_ids::AgentId;
use thiserror::Error;

use crate::workflow::{RunStatus, SuspendedRun, Workflow, WorkflowError, WorkflowRunner};

/// Stable identity and routing metadata for one workflow-backed agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowAgentIdentity {
    id: AgentId,
    name: String,
    capabilities: Vec<String>,
}

impl WorkflowAgentIdentity {
    /// Create an agent identity with a generated identifier.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: AgentId::new(),
            name: name.into(),
            capabilities: Vec::new(),
        }
    }

    /// Replace the generated identifier with a stable caller-provided identifier.
    pub fn with_id(mut self, id: AgentId) -> Self {
        self.id = id;
        self
    }

    /// Add a capability used by higher-level routers and supervisors.
    pub fn capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.push(capability.into());
        self
    }

    /// Return the stable agent identifier.
    pub fn id(&self) -> AgentId {
        self.id
    }

    /// Return the display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the declared routing capabilities.
    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }
}

/// Identifier for one execution session of a workflow-backed agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentRunId(u64);

impl AgentRunId {
    /// Return the agent-local run sequence number.
    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for AgentRunId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "run:{}", self.0)
    }
}

/// Executable agent instance binding one workflow to identity and resources.
pub struct WorkflowAgent<S, R> {
    identity: WorkflowAgentIdentity,
    workflow: Arc<Workflow<S, R>>,
    resources: Arc<R>,
    runner: WorkflowRunner,
    next_run_id: AtomicU64,
}

impl<S, R> WorkflowAgent<S, R> {
    /// Create an agent that owns its workflow and resources.
    pub fn new(identity: WorkflowAgentIdentity, workflow: Workflow<S, R>, resources: R) -> Self {
        Self::from_shared(identity, Arc::new(workflow), Arc::new(resources))
    }

    /// Create an agent from a shared workflow and shared resources.
    pub fn from_shared(
        identity: WorkflowAgentIdentity,
        workflow: Arc<Workflow<S, R>>,
        resources: Arc<R>,
    ) -> Self {
        Self {
            identity,
            workflow,
            resources,
            runner: WorkflowRunner::default(),
            next_run_id: AtomicU64::new(1),
        }
    }

    /// Replace the default workflow runner.
    pub fn with_runner(mut self, runner: WorkflowRunner) -> Self {
        self.runner = runner;
        self
    }

    /// Return the agent identity.
    pub fn identity(&self) -> &WorkflowAgentIdentity {
        &self.identity
    }

    /// Return the workflow architecture bound to this agent.
    pub fn workflow(&self) -> &Workflow<S, R> {
        self.workflow.as_ref()
    }

    /// Return the resources bound to this agent.
    pub fn resources(&self) -> &R {
        self.resources.as_ref()
    }

    /// Start a new agent run with isolated owned state.
    pub async fn run(&self, state: S) -> WorkflowAgentResult<WorkflowAgentRun<S>> {
        let run_id = AgentRunId(self.next_run_id.fetch_add(1, Ordering::Relaxed));
        let status = self
            .runner
            .run(self.workflow.as_ref(), self.resources.as_ref(), state)
            .await?;
        Ok(self.wrap_status(run_id, status))
    }

    /// Resume an agent-bound checkpoint.
    pub async fn resume(
        &self,
        checkpoint: WorkflowAgentCheckpoint<S>,
    ) -> WorkflowAgentResult<WorkflowAgentRun<S>> {
        if checkpoint.agent_id != self.identity.id {
            return Err(WorkflowAgentError::CheckpointAgentMismatch {
                expected: self.identity.id,
                actual: checkpoint.agent_id,
            });
        }

        let run_id = checkpoint.run_id;
        let status = self
            .runner
            .resume(
                self.workflow.as_ref(),
                self.resources.as_ref(),
                checkpoint.suspended,
            )
            .await?;
        Ok(self.wrap_status(run_id, status))
    }

    fn wrap_status(&self, run_id: AgentRunId, status: RunStatus<S>) -> WorkflowAgentRun<S> {
        let status = match status {
            RunStatus::Completed {
                state,
                transitions_completed,
            } => WorkflowAgentRunStatus::Completed {
                state,
                transitions_completed,
            },
            RunStatus::Suspended(suspended) => {
                WorkflowAgentRunStatus::Suspended(WorkflowAgentCheckpoint {
                    agent_id: self.identity.id,
                    run_id,
                    suspended,
                })
            }
        };

        WorkflowAgentRun {
            agent_id: self.identity.id,
            run_id,
            status,
        }
    }
}

/// Result of executing one workflow-backed agent session.
#[derive(Debug)]
pub struct WorkflowAgentRun<S> {
    agent_id: AgentId,
    run_id: AgentRunId,
    status: WorkflowAgentRunStatus<S>,
}

impl<S> WorkflowAgentRun<S> {
    /// Return the agent that owns this run.
    pub fn agent_id(&self) -> AgentId {
        self.agent_id
    }

    /// Return the agent-local run identifier.
    pub fn run_id(&self) -> AgentRunId {
        self.run_id
    }

    /// Borrow the run status and its state or checkpoint.
    pub fn status(&self) -> &WorkflowAgentRunStatus<S> {
        &self.status
    }

    /// Consume the run and return its status.
    pub fn into_status(self) -> WorkflowAgentRunStatus<S> {
        self.status
    }
}

/// Terminal or suspended state of one workflow-backed agent run.
#[derive(Debug)]
pub enum WorkflowAgentRunStatus<S> {
    /// Agent run completed successfully.
    Completed {
        /// Final run state.
        state: S,
        /// Total workflow transitions completed by the run.
        transitions_completed: usize,
    },
    /// Agent run paused and can be resumed through its owning agent.
    Suspended(WorkflowAgentCheckpoint<S>),
}

/// Agent-bound checkpoint returned by a suspended run.
#[derive(Debug)]
pub struct WorkflowAgentCheckpoint<S> {
    agent_id: AgentId,
    run_id: AgentRunId,
    suspended: SuspendedRun<S>,
}

impl<S> WorkflowAgentCheckpoint<S> {
    /// Return the agent required to resume this checkpoint.
    pub fn agent_id(&self) -> AgentId {
        self.agent_id
    }

    /// Return the run identifier preserved across resume operations.
    pub fn run_id(&self) -> AgentRunId {
        self.run_id
    }

    /// Borrow the suspended workflow state.
    pub fn state(&self) -> &S {
        self.suspended.state()
    }

    /// Mutably borrow the suspended workflow state before resuming.
    pub fn state_mut(&mut self) -> &mut S {
        self.suspended.state_mut()
    }

    /// Return the reason the run suspended.
    pub fn reason(&self) -> &str {
        self.suspended.suspension().reason()
    }

    /// Return the node that will execute when the run resumes.
    pub fn node_id(&self) -> &crate::workflow::NodeId {
        self.suspended.node_id()
    }

    /// Return the number of transitions completed before suspension.
    pub fn transitions_completed(&self) -> usize {
        self.suspended.transitions_completed()
    }
}

/// Registry of workflow-backed agents sharing a typed run contract.
pub struct WorkflowAgentRegistry<S, R> {
    agents: HashMap<AgentId, Arc<WorkflowAgent<S, R>>>,
}

impl<S, R> Default for WorkflowAgentRegistry<S, R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, R> WorkflowAgentRegistry<S, R> {
    /// Create an empty agent registry.
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Register an owned workflow agent.
    pub fn register(&mut self, agent: WorkflowAgent<S, R>) -> WorkflowAgentResult<AgentId> {
        self.register_shared(Arc::new(agent))
    }

    /// Register a shared workflow agent.
    pub fn register_shared(
        &mut self,
        agent: Arc<WorkflowAgent<S, R>>,
    ) -> WorkflowAgentResult<AgentId> {
        let agent_id = agent.identity.id;
        if self.agents.contains_key(&agent_id) {
            return Err(WorkflowAgentError::DuplicateAgent(agent_id));
        }
        self.agents.insert(agent_id, agent);
        Ok(agent_id)
    }

    /// Return a shared agent by identifier.
    pub fn get(&self, agent_id: AgentId) -> Option<Arc<WorkflowAgent<S, R>>> {
        self.agents.get(&agent_id).cloned()
    }

    /// Return agent identities in deterministic name and identifier order.
    pub fn identities(&self) -> Vec<WorkflowAgentIdentity> {
        let mut identities = self
            .agents
            .values()
            .map(|agent| agent.identity.clone())
            .collect::<Vec<_>>();
        identities.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.id.to_string().cmp(&right.id.to_string()))
        });
        identities
    }

    /// Return the number of registered agents.
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Return whether no agents are registered.
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }

    /// Remove and return an agent by identifier.
    pub fn remove(&mut self, agent_id: AgentId) -> Option<Arc<WorkflowAgent<S, R>>> {
        self.agents.remove(&agent_id)
    }

    /// Route a new run to a registered agent.
    pub async fn run(
        &self,
        agent_id: AgentId,
        state: S,
    ) -> WorkflowAgentResult<WorkflowAgentRun<S>> {
        let agent = self
            .get(agent_id)
            .ok_or(WorkflowAgentError::UnknownAgent(agent_id))?;
        agent.run(state).await
    }

    /// Route a suspended checkpoint back to its owning agent.
    pub async fn resume(
        &self,
        checkpoint: WorkflowAgentCheckpoint<S>,
    ) -> WorkflowAgentResult<WorkflowAgentRun<S>> {
        let agent_id = checkpoint.agent_id;
        let agent = self
            .get(agent_id)
            .ok_or(WorkflowAgentError::UnknownAgent(agent_id))?;
        agent.resume(checkpoint).await
    }
}

/// Failure produced while registering or executing a workflow-backed agent.
#[derive(Debug, Error)]
pub enum WorkflowAgentError {
    /// An agent identifier is already registered.
    #[error("Workflow agent '{0}' is already registered")]
    DuplicateAgent(AgentId),
    /// No registered agent has the requested identifier.
    #[error("Workflow agent '{0}' is not registered")]
    UnknownAgent(AgentId),
    /// A checkpoint was presented to an agent other than its owner.
    #[error("Workflow checkpoint belongs to agent '{actual}', not '{expected}'")]
    CheckpointAgentMismatch {
        /// Agent asked to resume the checkpoint.
        expected: AgentId,
        /// Agent that originally created the checkpoint.
        actual: AgentId,
    },
    /// The bound workflow failed to construct a run result.
    #[error(transparent)]
    Workflow(#[from] WorkflowError),
}

/// Result returned by workflow-backed agent operations.
pub type WorkflowAgentResult<T> = std::result::Result<T, WorkflowAgentError>;

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use async_trait::async_trait;

    use super::*;
    use crate::workflow::{Node, NodeContext, NodeId, NodeResult, Transition, WorkflowBuilder};

    struct AddNode;

    #[async_trait]
    impl Node<i32, i32> for AddNode {
        async fn execute(
            &self,
            context: &mut NodeContext<'_, i32>,
            state: &mut i32,
        ) -> NodeResult<Transition> {
            *state += context.resources();
            Ok(Transition::Complete)
        }

        fn destinations(&self) -> Vec<NodeId> {
            Vec::new()
        }
    }

    struct MultiplyNode;

    #[async_trait]
    impl Node<i32, i32> for MultiplyNode {
        async fn execute(
            &self,
            context: &mut NodeContext<'_, i32>,
            state: &mut i32,
        ) -> NodeResult<Transition> {
            *state *= context.resources();
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

    fn single_node_workflow<S, R, N>(node: N) -> Workflow<S, R>
    where
        N: Node<S, R> + 'static,
    {
        WorkflowBuilder::new("execute")
            .node("execute", node)
            .and_then(WorkflowBuilder::build)
            .expect("single-node workflow")
    }

    #[tokio::test]
    async fn one_agent_binds_identity_workflow_and_resources() {
        let identity = WorkflowAgentIdentity::new("adder").capability("math");
        let agent_id = identity.id();
        let agent = WorkflowAgent::new(identity, single_node_workflow(AddNode), 4);

        let run = agent.run(3).await.expect("agent run");

        assert_eq!(run.agent_id(), agent_id);
        assert_eq!(run.run_id().get(), 1);
        assert!(matches!(
            run.into_status(),
            WorkflowAgentRunStatus::Completed {
                state: 7,
                transitions_completed: 1
            }
        ));
    }

    #[tokio::test]
    async fn registry_holds_agents_with_different_workflows() {
        let add_agent = WorkflowAgent::new(
            WorkflowAgentIdentity::new("adder"),
            single_node_workflow(AddNode),
            2,
        );
        let add_id = add_agent.identity().id();
        let multiply_agent = WorkflowAgent::new(
            WorkflowAgentIdentity::new("multiplier"),
            single_node_workflow(MultiplyNode),
            4,
        );
        let multiply_id = multiply_agent.identity().id();
        let mut registry = WorkflowAgentRegistry::new();
        registry.register(add_agent).expect("register adder");
        registry
            .register(multiply_agent)
            .expect("register multiplier");

        let added = registry.run(add_id, 3).await.expect("adder run");
        let multiplied = registry.run(multiply_id, 3).await.expect("multiplier run");

        assert_eq!(registry.len(), 2);
        assert!(matches!(
            added.into_status(),
            WorkflowAgentRunStatus::Completed { state: 5, .. }
        ));
        assert!(matches!(
            multiplied.into_status(),
            WorkflowAgentRunStatus::Completed { state: 12, .. }
        ));
    }

    #[test]
    fn registry_rejects_duplicate_agent_identity() {
        let identity = WorkflowAgentIdentity::new("first");
        let duplicate = identity.clone();
        let mut registry = WorkflowAgentRegistry::new();
        registry
            .register(WorkflowAgent::new(
                identity,
                single_node_workflow(AddNode),
                1,
            ))
            .expect("register first agent");

        let error = registry
            .register(WorkflowAgent::new(
                duplicate,
                single_node_workflow(MultiplyNode),
                2,
            ))
            .expect_err("duplicate agent");

        assert!(matches!(error, WorkflowAgentError::DuplicateAgent(_)));
    }

    #[tokio::test]
    async fn checkpoint_can_only_resume_through_owning_agent() {
        let owner_resources = Arc::new(AtomicBool::new(false));
        let workflow = Arc::new(single_node_workflow(GateNode));
        let owner = WorkflowAgent::from_shared(
            WorkflowAgentIdentity::new("owner"),
            Arc::clone(&workflow),
            Arc::clone(&owner_resources),
        );
        let other = WorkflowAgent::from_shared(
            WorkflowAgentIdentity::new("other"),
            workflow,
            Arc::new(AtomicBool::new(true)),
        );

        let checkpoint = match owner.run(0).await.expect("suspended run").into_status() {
            WorkflowAgentRunStatus::Suspended(checkpoint) => checkpoint,
            WorkflowAgentRunStatus::Completed { .. } => panic!("run unexpectedly completed"),
        };
        let run_id = checkpoint.run_id();

        let error = other
            .resume(checkpoint)
            .await
            .expect_err("checkpoint owner mismatch");
        assert!(matches!(
            error,
            WorkflowAgentError::CheckpointAgentMismatch { .. }
        ));

        let checkpoint = match owner
            .run(0)
            .await
            .expect("second suspended run")
            .into_status()
        {
            WorkflowAgentRunStatus::Suspended(checkpoint) => checkpoint,
            WorkflowAgentRunStatus::Completed { .. } => panic!("run unexpectedly completed"),
        };
        let resumed_id = checkpoint.run_id();
        owner_resources.store(true, Ordering::SeqCst);
        let resumed = owner.resume(checkpoint).await.expect("resumed run");

        assert_ne!(run_id, resumed_id);
        assert_eq!(resumed.run_id(), resumed_id);
        assert!(matches!(
            resumed.into_status(),
            WorkflowAgentRunStatus::Completed {
                state: 2,
                transitions_completed: 2
            }
        ));
    }
}
