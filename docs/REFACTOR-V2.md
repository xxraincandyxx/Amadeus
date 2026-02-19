# Amadeus V2 Refactor Document

> Multi-Agent Orchestration System Based on "Agent OS" Philosophy

## Background

### Current Architecture Limitations

| Issue | Impact |
|-------|--------|
| Single Agent | No parallel tasks, no collaboration |
| In-memory State | Lost on restart, no rollback |
| Linear History | No branching exploration, no version control |
| No Collaboration Primitives | Difficult inter-agent communication |
| No Persistence | No recovery, no audit trail |

### Goal

Build an **Agent OS** — manage agent lifecycle, state, communication, and versioning like an operating system.

```
┌─────────────────────────────────────────────────────────────────┐
│                     Amadeus V2 Architecture                     │
├─────────────────────────────────────────────────────────────────┤
│  Scheduler │ Router │ Supervisor │ Checkpointer                │  ← Scheduling
├─────────────────────────────────────────────────────────────────┤
│  Workspace │ Branch │ Commit │ EventLog │ StateStore           │  ← Storage
├─────────────────────────────────────────────────────────────────┤
│  IPC │ PubSub │ RPC │ LockManager │ TransactionManager         │  ← Communication
├─────────────────────────────────────────────────────────────────┤
│  Agent Pool │ ToolRegistry │ LLMClient Pool                    │  ← Execution
└─────────────────────────────────────────────────────────────────┘
```

---

## Core Concepts

### 1. Workspace

Shared workspace for all agents, similar to a Git repository.

```rust
/// Workspace - Shared context for agent collaboration
pub struct Workspace {
    /// Unique identifier
    pub id: Uuid,

    /// Working directory
    pub workdir: PathBuf,

    /// Branch system
    branches: HashMap<String, Branch>,
    active_branch: String,

    /// Event log (append-only)
    event_log: EventLog,

    /// Versioned state
    state: VersionedState,

    /// Agent registry
    agents: HashMap<AgentId, AgentMeta>,

    /// Lock manager
    locks: LockManager,
}

impl Workspace {
    /// Create new workspace
    pub async fn create(path: PathBuf) -> Result<Self>;

    /// Restore from snapshot
    pub async fn restore(snapshot: Snapshot) -> Result<Self>;

    /// Create branch
    pub async fn branch(&mut self, name: &str, from: Option<&str>) -> Result<()>;

    /// Switch branch
    pub async fn checkout(&mut self, name: &str) -> Result<()>;

    /// Merge branch
    pub async fn merge(&mut self, from: &str, into: &str, strategy: MergeStrategy) -> Result<()>;

    /// Create commit
    pub async fn commit(&mut self, message: &str, author: AgentId) -> Result<CommitId>;

    /// Reset to commit
    pub async fn reset(&mut self, to: CommitId, mode: ResetMode) -> Result<()>;

    /// View diff
    pub async fn diff(&self, from: &str, to: &str) -> Result<Diff>;

    /// View history
    pub async fn log(&self, limit: usize) -> Result<Vec<Commit>>;

    /// Create snapshot (for persistence)
    pub async fn snapshot(&self) -> Result<Snapshot>;
}
```

### 2. Branch

**Key Innovation**: Allow agents to "regret", to explore in parallel.

```rust
/// Branch - Independent line of work
pub struct Branch {
    /// Branch name
    pub name: String,

    /// Parent branch and commit (for merge)
    pub parent: Option<(String, CommitId)>,

    /// Commit history
    commits: Vec<CommitId>,

    /// Current HEAD
    head: CommitId,

    /// Creation time
    created_at: DateTime<Utc>,

    /// Last update time
    updated_at: DateTime<Utc>,
}
```

**Use Cases:**

```rust
// 1. Exploratory tasks - trial and error
ws.branch("experiment-fix", from="main")?;
ws.checkout("experiment-fix")?;
// Agent executes, if it fails...
ws.checkout("main")?;
ws.delete_branch("experiment-fix")?;

// 2. Parallel tasks - competing approaches
ws.branch("approach-a", from="main")?;
ws.branch("approach-b", from="main")?;

// Run agents in parallel on different branches
let a = ws.run_in_branch("approach-a", agent_a).await?;
let b = ws.run_in_branch("approach-b", agent_b).await?;

// Choose the best result
let winner = compare(a, b)?;
ws.merge(winner, into="main")?;

// 3. Rollback - restore to previous state
let commit = ws.find_commit_by_message("before refactor")?;
ws.reset(commit, ResetMode::Hard)?;
```

### 3. Commit

State snapshot, supports rollback and auditing.

```rust
/// Commit - State snapshot
pub struct Commit {
    /// Unique identifier
    pub id: CommitId,

    /// Parent commit
    pub parent: Option<CommitId>,

    /// State snapshot (KV snapshot)
    pub state: StateSnapshot,

    /// Commit message
    pub message: String,

    /// Author (Agent)
    pub author: AgentId,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Trigger reason
    pub trigger: CommitTrigger,
}

#[derive(Debug, Clone)]
pub enum CommitTrigger {
    /// User explicit request
    UserRequest,

    /// Agent completed key step
    AgentCheckpoint { agent: AgentId, step: String },

    /// Scheduled auto-save
    AutoSave,

    /// Before/after tool execution
    ToolExecution { tool: String, phase: Phase },

    /// Error recovery point
    RecoveryPoint,
}
```

### 4. Event

Append-only event log, for auditing and replay.

```rust
/// Event - Everything that happens in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    // === Agent Lifecycle ===
    AgentSpawned {
        id: AgentId,
        role: String,
        config: AgentConfig
    },
    AgentTerminated {
        id: AgentId,
        reason: TerminationReason
    },
    AgentHeartbeat {
        id: AgentId,
        status: AgentStatus
    },

    // === Thinking & Action ===
    AgentThinking {
        id: AgentId,
        content: String
    },
    AgentAction {
        id: AgentId,
        action: Action
    },

    // === Communication ===
    MessageSent {
        from: AgentId,
        to: AgentId,
        content: String,
        channel: Option<String>,
    },
    MessageBroadcast {
        from: AgentId,
        content: String,
        channel: String,
    },

    // === State Changes ===
    StateUpdated {
        key: String,
        old: Option<Value>,
        new: Value,
        author: AgentId,
    },

    // === Tool Calls ===
    ToolCallStart {
        agent: AgentId,
        tool: String,
        args: Value
    },
    ToolCallComplete {
        agent: AgentId,
        tool: String,
        result: Value,
        duration: Duration,
    },
    ToolCallError {
        agent: AgentId,
        tool: String,
        error: String
    },

    // === Branch & Version ===
    BranchCreated {
        name: String,
        from: String,
        from_commit: CommitId,
    },
    BranchDeleted { name: String },
    CommitCreated {
        id: CommitId,
        message: String,
        author: AgentId,
    },
    MergeCompleted {
        from: String,
        into: String,
        result: MergeResult,
    },
    ResetCompleted {
        to: CommitId,
        mode: ResetMode,
    },

    // === Locks & Transactions ===
    LockAcquired {
        resource: String,
        holder: AgentId,
        mode: LockMode,
    },
    LockReleased {
        resource: String,
        holder: AgentId,
    },
    TransactionStarted {
        tx_id: TxId,
        initiator: AgentId,
    },
    TransactionCommitted { tx_id: TxId },
    TransactionRolledBack { tx_id: TxId, reason: String },
}

/// Event log - append-only storage
pub struct EventLog {
    events: Vec<Event>,
    index: HashMap<String, Vec<usize>>,  // Index for fast lookup
}

impl EventLog {
    /// Append event
    pub fn append(&mut self, event: Event) -> Result<usize>;

    /// Query events (with filter)
    pub fn query(&self, filter: EventFilter) -> Result<Vec<&Event>>;

    /// Replay from position
    pub fn replay_from(&self, index: usize) -> Result<Vec<Event>>;

    /// Persist to disk
    pub async fn flush(&self, path: &Path) -> Result<()>;
}
```

### 5. State

Versioned KV store with transaction support.

```rust
/// Versioned state
pub struct VersionedState {
    /// KV store
    data: BTreeMap<String, (Value, Version)>,

    /// Current version number
    version: u64,

    /// Lock manager reference
    locks: Arc<LockManager>,
}

impl VersionedState {
    /// Read value
    pub async fn read(&self, key: &str) -> Result<Option<Value>>;

    /// Write value
    pub async fn write(&mut self, key: &str, value: Value) -> Result<()>;

    /// Compare-and-Swap (optimistic lock)
    pub async fn cas(
        &mut self,
        key: &str,
        expected: Option<&Value>,
        new: Value
    ) -> Result<bool>;

    /// Atomic update
    pub async fn update<F>(&mut self, key: &str, f: F) -> Result<Value>
    where
        F: FnOnce(Option<&Value>) -> Value;

    /// Batch read
    pub async fn read_batch(&self, keys: &[&str]) -> Result<HashMap<String, Option<Value>>>;

    /// Batch write
    pub async fn write_batch(&mut self, updates: HashMap<String, Value>) -> Result<()>;

    /// Create snapshot
    pub fn snapshot(&self) -> StateSnapshot;

    /// Restore from snapshot
    pub fn restore(&mut self, snapshot: StateSnapshot);
}
```

---

## Agent System

### Agent Definition

```rust
/// Agent configuration
pub struct AgentConfig {
    /// Agent ID (auto-generated or specified)
    pub id: Option<AgentId>,

    /// Role/name
    pub role: String,

    /// System prompt
    pub system_prompt: String,

    /// Model to use
    pub model: ModelConfig,

    /// Available tools (None = all)
    pub tools: Option<Vec<String>>,

    /// Max tool calls
    pub max_tool_calls: usize,

    /// Timeout
    pub timeout: Duration,

    /// Priority (for scheduling)
    pub priority: u8,

    /// Restart policy
    pub restart_policy: RestartPolicy,
}

/// Agent runtime state
pub struct Agent {
    /// Configuration
    config: AgentConfig,

    /// Workspace reference
    workspace: Arc<Workspace>,

    /// LLM client
    client: Arc<dyn LLMClient>,

    /// Tool registry (possibly subset)
    tools: ToolRegistry,

    /// Local state
    local_state: HashMap<String, Value>,

    /// Runtime status
    status: AgentStatus,

    /// Statistics
    stats: AgentStats,
}

impl Agent {
    /// Execute task (blocking until complete)
    pub async fn run(&mut self, input: &str) -> Result<RunResult>;

    /// Execute task (stream events)
    pub fn run_stream(&mut self, input: &str) -> impl Stream<Item = AgentEvent>;

    /// Execute task (channel version)
    pub async fn run_channel(&mut self, input: &str) -> Result<mpsc::Receiver<AgentEvent>>;

    /// Stop execution
    pub async fn stop(&mut self) -> Result<()>;

    /// Pause/resume
    pub async fn pause(&mut self) -> Result<()>;
    pub async fn resume(&mut self) -> Result<()>;

    /// Get status
    pub fn status(&self) -> AgentStatus;

    /// Get stats
    pub fn stats(&self) -> &AgentStats;
}
```

### Agent Events

```rust
/// Agent execution events
#[derive(Debug, Clone)]
pub enum AgentEvent {
    // === Lifecycle ===
    Started { id: AgentId },
    Paused { id: AgentId },
    Resumed { id: AgentId },
    Stopped { id: AgentId, reason: StopReason },

    // === LLM Interaction ===
    TextDelta { delta: String },
    Thinking { content: String },

    // === Tool Calls ===
    ToolStart { id: String, name: String },
    ToolInputDelta { id: String, delta: String },
    ToolComplete {
        id: String,
        name: String,
        input: Value,
        output: String,
        is_error: bool,
    },

    // === State Changes ===
    StateUpdated { key: String, old: Option<Value>, new: Value },

    // === Completion ===
    Done { result: RunResult },
    Error { message: String },
}
```

---

## Collaboration Patterns

### 1. Supervisor-Worker

```rust
/// Supervisor configuration
pub struct SupervisorConfig {
    /// Supervisor agent
    supervisor: AgentConfig,

    /// Worker agents
    workers: Vec<AgentConfig>,

    /// Task dispatch strategy
    strategy: DispatchStrategy,

    /// Max parallel tasks
    max_parallel: usize,
}

/// Dispatch strategies
pub enum DispatchStrategy {
    /// Round-robin
    RoundRobin,
    /// Least loaded first
    LeastLoaded,
    /// Random
    Random,
    /// Match by agent capability
    CapabilityMatch,
}

impl Supervisor {
    /// Dispatch task to workers
    pub async fn dispatch(&mut self, task: Task) -> Result<Vec<TaskResult>>;

    /// Broadcast to all workers
    pub async fn broadcast(&mut self, message: &str) -> Result<Vec<RunResult>>;

    /// Collect worker results
    pub async fn collect(&mut self) -> Result<Vec<TaskResult>>;

    /// Monitor worker status
    pub fn workers_status(&self) -> HashMap<AgentId, AgentStatus>;
}
```

### 2. Pipeline

```rust
/// Pipeline
pub struct Pipeline {
    stages: Vec<PipelineStage>,
    config: PipelineConfig,
}

pub struct PipelineStage {
    name: String,
    agent: AgentConfig,
    timeout: Duration,
    retry: RetryPolicy,
}

impl Pipeline {
    /// Create pipeline
    pub fn new() -> Self;

    /// Add stage
    pub fn stage(mut self, name: &str, agent: AgentConfig) -> Self;

    /// Execute pipeline
    pub async fn run(&self, input: Value) -> Result<PipelineResult>;

    /// Stream execution
    pub fn run_stream(&self, input: Value) -> impl Stream<Item = PipelineEvent>;
}

// Usage example
let pipeline = Pipeline::new()
    .stage("parse", AgentConfig::for_role("parser"))
    .stage("analyze", AgentConfig::for_role("analyzer"))
    .stage("generate", AgentConfig::for_role("generator"))
    .stage("review", AgentConfig::for_role("reviewer"));

let result = pipeline.run(input).await?;
```

### 3. Mesh

```rust
/// Mesh topology
pub struct Mesh {
    agents: HashMap<AgentId, Agent>,
    topology: Topology,
    router: Router,
}

pub enum Topology {
    /// Fully connected (every agent can communicate directly)
    FullMesh,
    /// Star (central router)
    Star { center: AgentId },
    /// Ring
    Ring,
    /// Custom adjacency list
    Custom(HashMap<AgentId, Vec<AgentId>>),
}

impl Mesh {
    /// Create mesh
    pub fn new(topology: Topology) -> Self;

    /// Add agent
    pub fn add(mut self, id: &str, agent: Agent) -> Self;

    /// Start all agents
    pub async fn start(&mut self) -> Result<()>;

    /// Send message
    pub async fn send(&mut self, from: AgentId, to: AgentId, msg: &str) -> Result<()>;

    /// Broadcast
    pub async fn broadcast(&mut self, from: AgentId, msg: &str) -> Result<()>;

    /// Run until all agents complete
    pub async fn run(&mut self, initial: Option<(AgentId, &str)>) -> Result<MeshResult>;
}
```

### 4. Race

```rust
/// Competitive execution
pub struct Race {
    agents: Vec<Agent>,
    config: RaceConfig,
}

pub struct RaceConfig {
    /// Stop condition
    stop_on: StopCondition,

    /// Timeout
    timeout: Duration,

    /// Return all results (even with winner)
    return_all: bool,
}

pub enum StopCondition {
    /// First success
    FirstSuccess,
    /// First complete (success or failure)
    FirstComplete,
    /// All complete
    AllComplete,
    /// Vote decision
    Vote { voters: Vec<AgentId>, threshold: f32 },
}

impl Race {
    /// Create race
    pub fn new(agents: Vec<Agent>) -> Self;

    /// Run race
    pub async fn run(&mut self, task: &str) -> Result<RaceResult>;
}

pub struct RaceResult {
    /// Winner
    pub winner: Option<AgentId>,

    /// Winner result
    pub winner_result: Option<RunResult>,

    /// All results
    pub all_results: HashMap<AgentId, Result<RunResult>>,

    /// Ranking (by completion time)
    pub ranking: Vec<(AgentId, Duration)>,
}
```

---

## Concurrency Control

### Lock Management

```rust
/// Lock manager
pub struct LockManager {
    locks: HashMap<String, LockEntry>,
    wait_queue: HashMap<String, VecDeque<Waiter>>,
}

struct LockEntry {
    holder: AgentId,
    mode: LockMode,
    acquired_at: Instant,
    timeout: Duration,
}

#[derive(Debug, Clone, Copy)]
pub enum LockMode {
    /// Shared lock (read)
    Shared,
    /// Exclusive lock (write)
    Exclusive,
}

impl LockManager {
    /// Try acquire lock (non-blocking)
    pub fn try_acquire(
        &mut self,
        resource: &str,
        holder: AgentId,
        mode: LockMode
    ) -> Result<bool>;

    /// Acquire lock (blocking)
    pub async fn acquire(
        &mut self,
        resource: &str,
        holder: AgentId,
        mode: LockMode,
        timeout: Duration
    ) -> Result<()>;

    /// Release lock
    pub fn release(&mut self, resource: &str, holder: AgentId) -> Result<()>;

    /// Query lock status
    pub fn status(&self, resource: &str) -> Option<LockStatus>;

    /// Force release (admin)
    pub fn force_release(&mut self, resource: &str) -> Result<()>;
}
```

### Transactions

```rust
/// Transaction manager
pub struct TransactionManager {
    active: HashMap<TxId, Transaction>,
    state: Arc<RwLock<VersionedState>>,
}

pub struct Transaction {
    id: TxId,
    initiator: AgentId,
    operations: Vec<Operation>,
    state: TxState,
    started_at: Instant,
    timeout: Duration,
}

#[derive(Debug, Clone)]
pub enum Operation {
    Write { key: String, value: Value },
    Delete { key: String },
}

#[derive(Debug, Clone, Copy)]
pub enum TxState {
    Active,
    Committed,
    RolledBack,
}

impl TransactionManager {
    /// Begin transaction
    pub async fn begin(&mut self, initiator: AgentId) -> Result<TxId>;

    /// Write in transaction
    pub async fn write(&mut self, tx: TxId, key: &str, value: Value) -> Result<()>;

    /// Commit transaction
    pub async fn commit(&mut self, tx: TxId) -> Result<()>;

    /// Rollback transaction
    pub async fn rollback(&mut self, tx: TxId, reason: &str) -> Result<()>;

    /// Get transaction state
    pub fn status(&self, tx: TxId) -> Option<TxState>;
}
```

---

## API Design

### Core API

```rust
// === Workspace API ===

// Create workspace
let ws = Workspace::create("./my-project").await?;

// Register agent
ws.register_agent("analyzer", AgentConfig {
    role: "code-analyzer".into(),
    system_prompt: "You analyze code...".into(),
    ..Default::default()
}).await?;

// Create branch
ws.branch("feature-auth", from="main").await?;
ws.checkout("feature-auth").await?;

// Execute agent
let result = ws.run("analyzer", "Analyze the authentication module").await?;

// View events
let events = ws.events(EventFilter::agent("analyzer")).await?;

// Commit
let commit = ws.commit("Completed auth analysis", author).await?;

// Merge back to main
ws.checkout("main").await?;
ws.merge("feature-auth", into="main", MergeStrategy::Auto).await?;
```

### TUI Integration

```rust
/// TUI App (V2)
pub struct App {
    /// Workspace
    workspace: Arc<Workspace>,

    /// Currently active agent
    active_agent: Option<AgentId>,

    /// UI components
    components: Components,

    /// Event subscription
    event_sub: mpsc::Receiver<Event>,
}

impl App {
    /// Start TUI
    pub async fn run(&mut self) -> Result<()>;

    /// Handle keyboard events
    async fn handle_key(&mut self, key: KeyEvent) -> Result<()>;

    /// Handle agent events
    async fn handle_agent_event(&mut self, event: AgentEvent) -> Result<()>;

    /// Render UI
    fn render(&mut self, frame: &mut Frame);
}
```

---

## Directory Structure (Refactored)

```
src/
├── lib.rs                    # Public exports
├── main.rs                   # CLI entry point
│
├── core/                     # Core primitives
│   ├── mod.rs
│   ├── workspace.rs          # Workspace
│   ├── branch.rs             # Branch
│   ├── commit.rs             # Commit
│   ├── event.rs              # Event, EventLog
│   ├── state.rs              # VersionedState
│   ├── snapshot.rs           # Snapshot/restore
│   └── error.rs              # Error types
│
├── agent/                    # Agent system
│   ├── mod.rs
│   ├── agent.rs              # Agent, AgentConfig
│   ├── events.rs             # AgentEvent
│   ├── registry.rs           # Agent registry
│   ├── supervisor.rs         # Supervisor-Worker
│   ├── pipeline.rs           # Pipeline
│   ├── mesh.rs               # Mesh
│   └── race.rs               # Race
│
├── client/                   # LLM clients (keep)
│   ├── mod.rs
│   ├── trait.rs              # LLMClient trait
│   ├── anthropic.rs
│   └── openai.rs
│
├── tools/                    # Tool system (keep)
│   ├── mod.rs
│   ├── trait.rs              # Tool trait
│   ├── registry.rs           # ToolRegistry
│   ├── bash.rs
│   ├── file.rs
│   └── schema.rs
│
├── concurrency/              # Concurrency control
│   ├── mod.rs
│   ├── lock.rs               # LockManager
│   └── transaction.rs        # TransactionManager
│
├── api/                      # HTTP API (keep)
│   ├── mod.rs
│   ├── server.rs
│   ├── handlers/
│   └── types.rs
│
├── tui/                      # TUI (refactor)
│   ├── mod.rs
│   ├── app.rs                # App state
│   ├── event.rs              # Event handling
│   └── components/
│       ├── mod.rs
│       ├── messages.rs
│       ├── sidebar.rs
│       ├── branch.rs         # NEW: Branch selector
│       ├── agents.rs         # NEW: Agent status panel
│       └── ...
│
└── storage/                  # Persistence
    ├── mod.rs
    ├── file.rs               # File storage
    ├── sqlite.rs             # SQLite storage (optional)
    └── format.rs             # Serialization format
```

---

## Implementation Plan

### Phase 1: Core Primitives (2 weeks)

**Goal:** Implement Workspace + Branch + Commit + EventLog

**Deliverables:**
- [ ] `core/workspace.rs` - Workspace create/load
- [ ] `core/branch.rs` - Branch create/switch/merge
- [ ] `core/commit.rs` - Commit create/query
- [ ] `core/event.rs` - Event log append/query
- [ ] `core/state.rs` - Versioned KV store
- [ ] `storage/` - File persistence

**Test:**
```rust
#[tokio::test]
async fn test_workspace_branch_merge() {
    let mut ws = Workspace::create("./test-ws").await.unwrap();

    ws.branch("feature", from="main").await.unwrap();
    ws.checkout("feature").await.unwrap();

    ws.state().write("key", json!("value")).await.unwrap();
    ws.commit("test commit", AgentId::system()).await.unwrap();

    ws.checkout("main").await.unwrap();
    ws.merge("feature", into="main", MergeStrategy::Auto).await.unwrap();

    let value = ws.state().read("key").await.unwrap().unwrap();
    assert_eq!(value, json!("value"));
}
```

### Phase 2: Agent System (2 weeks)

**Goal:** Refactor Agent, integrate with Workspace

**Deliverables:**
- [ ] `agent/agent.rs` - Agent refactor, Workspace support
- [ ] `agent/events.rs` - AgentEvent extension
- [ ] `agent/registry.rs` - Agent registry
- [ ] Integrate existing `client/` and `tools/`

**API Change:**
```rust
// V1
let agent = Agent::new(client, workdir, timeout, stream);
let result = agent.run(prompt, history).await?;

// V2
let agent = workspace.spawn_agent("analyzer", config).await?;
let result = agent.run("analyze this").await?;
```

### Phase 3: Collaboration Patterns (2 weeks)

**Goal:** Implement Supervisor/Pipeline/Mesh/Race

**Deliverables:**
- [ ] `agent/supervisor.rs` - Supervisor-Worker
- [ ] `agent/pipeline.rs` - Pipeline
- [ ] `agent/mesh.rs` - Mesh
- [ ] `agent/race.rs` - Race

### Phase 4: Concurrency Control (1 week)

**Goal:** Locks and transactions

**Deliverables:**
- [ ] `concurrency/lock.rs` - LockManager
- [ ] `concurrency/transaction.rs` - TransactionManager

### Phase 5: TUI Refactor (2 weeks)

**Goal:** Integrate new architecture into TUI

**Deliverables:**
- [ ] Branch selector component
- [ ] Agent status panel
- [ ] Event log viewer
- [ ] Tool execution history

### Phase 6: Documentation & Testing (1 week)

**Goal:** Improve documentation and test coverage

**Deliverables:**
- [ ] API documentation
- [ ] Architecture diagram updates
- [ ] Unit tests > 80%
- [ ] Integration tests

---

## Migration Guide

### V1 to V2 Migration

**V1 Code:**
```rust
let config = Config::load()?;
let client = AnthropicClient::new(config.api_key, None, config.model);
let agent = Agent::new(client, workdir.to_string(), 300, false);

let history = Arc::new(RwLock::new(vec![]));
let result = agent.run("prompt", history).await?;
```

**V2 Equivalent:**
```rust
let ws = Workspace::create(workdir).await?;
let agent = ws.spawn_agent("default", AgentConfig {
    model: ModelConfig::anthropic(&api_key, &model),
    ..Default::default()
}).await?;

let result = agent.run("prompt").await?;
```

**Auto-migration script:** TODO

---

## Risks & Mitigation

| Risk | Mitigation |
|------|-----------|
| Complexity explosion | Gradual migration, maintain V1 API compatibility |
| Performance degradation | Benchmarks, optimize hot paths |
| Persistence overhead | Async writes, batch commits |
| Concurrency bugs | Thorough testing, formal verification of critical paths |

---

## References

- Git internals
- Kafka event log
- Raft consensus algorithm (partial concepts)
- Erlang/OTP actor model

---

*El Psy Kongroo*
