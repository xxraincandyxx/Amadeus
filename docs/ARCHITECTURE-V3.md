# Amadeus V3 Architecture: Production-Ready Multi-Agent System

> Performance optimization and scalability improvements based on production requirements

## Executive Summary

This document outlines critical architectural improvements for Amadeus V2, transforming it from a prototype into a production-ready multi-agent orchestration system.

**Key Improvements:**
1. **Actor-based Agents** - Persistent agent tasks with message-driven lifecycle
2. **Concurrent Execution** - Replace sequential loops with `JoinSet`/`FuturesUnordered`
3. **Backpressure Control** - Semaphores to prevent resource exhaustion
4. **Delta State Updates** - Surgical updates instead of full snapshots

---

## Table of Contents

1. [Current Architecture Analysis](#1-current-architecture-analysis)
2. [Actor-Based Agent Model](#2-actor-based-agent-model)
3. [Concurrent Execution Patterns](#3-concurrent-execution-patterns)
4. [Backpressure Control](#4-backpressure-control)
5. [Delta State Management](#5-delta-state-management)
6. [Implementation Roadmap](#6-implementation-roadmap)

---

## 1. Current Architecture Analysis

### 1.1 The "Ephemeral Agent" Problem

**Current Pattern:**
```rust
// Every task creates a new agent
let agent = Agent::new(client, config, workspace);
let result = agent.run(prompt).await?;
// Agent is dropped after use
```

**Issues:**

| Problem | Impact |
|---------|--------|
| No state persistence | Agent loses context between tasks |
| Connection churn | New HTTP connections for each task |
| No lifecycle management | Cannot pause/resume/cancel |
| Memory fragmentation | Frequent allocation/deallocation |

**Cost Analysis:**
```
Task 1: Create Agent (5ms) + Run (2s) + Drop (1ms) = 2.006s
Task 2: Create Agent (5ms) + Run (2s) + Drop (1ms) = 2.006s
Total: 4.012s

With persistent agent:
Task 1: Run (2s)
Task 2: Run (2s)
Total: 4.000s (saves ~1% overhead)
```

Overhead is small for single tasks, but compounds at scale:
- 1000 tasks = 6 seconds wasted
- No ability to reuse connections or caches

### 1.2 Sequential Execution Problem

**Current Code (Supervisor):**
```rust
pub async fn dispatch_batch(&mut self, tasks: Vec<Task>) -> Vec<TaskResult> {
    let mut results = Vec::new();
    for task in tasks {
        let result = self.dispatch(task).await;  // BLOCKING!
        results.push(result);
    }
    results
}
```

**Performance Impact:**

```
5 tasks, each taking 2s:
Sequential: 5 × 2s = 10s

Concurrent (with JoinSet):
All 5 run in parallel = ~2s (5x faster)
```

### 1.3 Resource Exhaustion Risk

**Current Code:**
```rust
// No limit on concurrent tasks
for task in tasks {
    join_set.spawn(async move {
        agent.run(&task.prompt).await
    });
}
```

**Failure Scenarios:**

| Scenario | Result |
|----------|--------|
| 100 concurrent LLM requests | Rate limited (429 errors) |
| 1000 concurrent tasks | OOM (Out of Memory) |
| API key has 10 RPM limit | 90 requests fail immediately |

### 1.4 Full Snapshot Overhead

**Current Code:**
```rust
pub struct WorkspaceSnapshot {
    pub state: StateSnapshot,  // Full KV store
    pub commits: HashMap<CommitId, Commit>,
    // ...
}
```

**Performance:**

| State Size | Save Time | Memory |
|------------|-----------|--------|
| 1 MB | ~5ms | 1 MB |
| 100 MB | ~500ms | 100 MB |
| 1 GB | ~5s | 1 GB |

**Problem:** Every commit/save writes the entire state, even for a single key update.

---

## 2. Actor-Based Agent Model

### 2.1 Concept

Transform agents from ephemeral structs into persistent actor tasks with mailboxes.

**Actor Pattern:**
```
┌─────────────────────────────────────────┐
│           Agent Actor (Task)             │
│  ┌────────────────────────────────────┐ │
│  │  Mailbox (mpsc::Receiver)          │ │
│  │  ┌───┬───┬───┬───┬───┐             │ │
│  │  │ M │ M │ M │...│   │             │ │
│  │  └───┴───┴───┴───┴───┘             │ │
│  └────────────────────────────────────┘ │
│  ┌────────────────────────────────────┐ │
│  │  State                             │ │
│  │  - history: Vec<Message>           │ │
│  │  - local_state: HashMap            │ │
│  │  - status: AgentStatus             │ │
│  └────────────────────────────────────┘ │
│  ┌────────────────────────────────────┐ │
│  │  LLM Client (persistent)           │ │
│  └────────────────────────────────────┘ │
└─────────────────────────────────────────┘
         ▲
         │ Send messages
         │
┌────────┴────────┐
│   External      │
│   Controller    │
└─────────────────┘
```

### 2.2 Implementation

**Agent Message Types:**
```rust
/// Messages that can be sent to an agent actor
pub enum AgentMessage {
    /// Execute a task
    Task {
        id: TaskId,
        prompt: String,
        respond_to: oneshot::Sender<Result<RunResult>>,
    },
    
    /// Pause execution (finish current task, don't start new)
    Pause {
        respond_to: oneshot::Sender<()>,
    },
    
    /// Resume from paused state
    Resume {
        respond_to: oneshot::Sender<()>,
    },
    
    /// Cancel current task
    Cancel {
        respond_to: oneshot::Sender<()>,
    },
    
    /// Get current status
    GetStatus {
        respond_to: oneshot::Sender<AgentStatus>,
    },
    
    /// Shutdown the actor
    Shutdown,
}

/// The agent actor
pub struct AgentActor<C: LLMClient> {
    id: AgentId,
    inbox: mpsc::Receiver<AgentMessage>,
    client: C,
    tools: ToolRegistry,
    config: AgentConfig,
    workspace: Arc<RwLock<Workspace>>,
    
    // Persistent state
    history: Vec<Message>,
    local_state: HashMap<String, Value>,
    status: AgentStatus,
    stats: AgentStats,
    
    // Control flags
    paused: bool,
    cancelled: bool,
}

impl<C: LLMClient + Clone + 'static> AgentActor<C> {
    pub fn spawn(
        client: C,
        config: AgentConfig,
        workspace: Arc<RwLock<Workspace>>,
    ) -> AgentHandle {
        let (tx, rx) = mpsc::channel(64);
        let id = config.id.unwrap_or_else(AgentId::new);
        
        let actor = Self {
            id,
            inbox: rx,
            client,
            tools: ToolRegistry::default(),
            config,
            workspace,
            history: Vec::new(),
            local_state: HashMap::new(),
            status: AgentStatus::Idle,
            stats: AgentStats::default(),
            paused: false,
            cancelled: false,
        };
        
        // Spawn the actor as a persistent task
        tokio::spawn(async move {
            actor.run().await;
        });
        
        AgentHandle {
            id,
            sender: tx,
        }
    }
    
    async fn run(mut self) {
        while let Some(msg) = self.inbox.recv().await {
            match msg {
                AgentMessage::Shutdown => break,
                AgentMessage::Task { id, prompt, respond_to } => {
                    self.status = AgentStatus::Thinking;
                    let result = self.execute_task(&prompt).await;
                    let _ = respond_to.send(result);
                    self.status = AgentStatus::Idle;
                }
                AgentMessage::Pause { respond_to } => {
                    self.paused = true;
                    let _ = respond_to.send(());
                }
                AgentMessage::Resume { respond_to } => {
                    self.paused = false;
                    let _ = respond_to.send(());
                }
                AgentMessage::Cancel { respond_to } => {
                    self.cancelled = true;
                    let _ = respond_to.send(());
                }
                AgentMessage::GetStatus { respond_to } => {
                    let _ = respond_to.send(self.status.clone());
                }
            }
        }
    }
    
    async fn execute_task(&mut self, prompt: &str) -> Result<RunResult> {
        // Reset cancelled flag for new task
        self.cancelled = false;
        
        // Add to history
        self.history.push(Message::user(prompt));
        
        // Execute with cancellation support
        let result = self.run_agent_loop().await;
        
        // Clear cancelled flag
        self.cancelled = false;
        
        result
    }
}
```

**Agent Handle (Public API):**
```rust
/// Handle to communicate with an agent actor
pub struct AgentHandle {
    id: AgentId,
    sender: mpsc::Sender<AgentMessage>,
}

impl AgentHandle {
    /// Execute a task and wait for result
    pub async fn run(&self, prompt: &str) -> Result<RunResult> {
        let (tx, rx) = oneshot::channel();
        
        self.sender.send(AgentMessage::Task {
            id: TaskId::new(),
            prompt: prompt.to_string(),
            respond_to: tx,
        }).await?;
        
        rx.await?
    }
    
    /// Pause the agent
    pub async fn pause(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(AgentMessage::Pause { respond_to: tx }).await?;
        rx.await?;
        Ok(())
    }
    
    /// Resume the agent
    pub async fn resume(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(AgentMessage::Resume { respond_to: tx }).await?;
        rx.await?;
        Ok(())
    }
    
    /// Get current status
    pub async fn status(&self) -> Result<AgentStatus> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(AgentMessage::GetStatus { respond_to: tx }).await?;
        Ok(rx.await?)
    }
    
    /// Shutdown the agent
    pub async fn shutdown(&self) -> Result<()> {
        self.sender.send(AgentMessage::Shutdown).await?;
        Ok(())
    }
}
```

### 2.3 Benefits

| Feature | Before | After |
|---------|--------|-------|
| State persistence | Lost between tasks | Preserved in actor |
| Connection reuse | New per task | Reused across tasks |
| Lifecycle control | None | Pause/Resume/Cancel |
| Backpressure | Not supported | Bounded mailbox |
| Monitoring | Poll-based | Event-driven |

### 2.4 Migration Path

**Phase 1: Wrapper (Non-breaking)**
```rust
// Keep Agent::new() API, internally use actor
impl Agent {
    pub fn new(...) -> Self {
        // Create actor internally
        let handle = AgentActor::spawn(...);
        Self { handle }
    }
    
    pub async fn run(&self, prompt: &str) -> Result<RunResult> {
        self.handle.run(prompt).await
    }
}
```

**Phase 2: Expose Actor API**
```rust
// Add new API alongside old
impl Agent {
    pub fn spawn(...) -> AgentHandle { ... }
}

// Deprecate old API
#[deprecated(note = "Use Agent::spawn() for actor-based agents")]
pub fn new(...) -> Self { ... }
```

---

## 3. Concurrent Execution Patterns

### 3.1 Problem Analysis

**Current Sequential Pattern:**
```rust
// supervisor.rs
pub async fn dispatch_batch(&mut self, tasks: Vec<Task>) -> Vec<TaskResult> {
    let mut results = Vec::new();
    for task in tasks {
        let result = self.dispatch(task).await;  // ← Blocks!
        results.push(result);
    }
    results
}
```

**Timeline:**
```
Task 1: |████████████| 2s
Task 2:              |████████████| 2s
Task 3:                          |████████████| 2s
Total: 6s
```

### 3.2 Solution: JoinSet

**What is JoinSet?**
- Tokio's built-in concurrent task manager
- Spawns multiple async tasks
- Collects results in completion order
- Handles panics and cancellation

**Improved Implementation:**
```rust
use tokio::task::JoinSet;

impl<C: LLMClient + Clone + 'static> Supervisor<C> {
    /// Execute multiple tasks concurrently
    pub async fn dispatch_concurrent(
        &mut self,
        tasks: Vec<Task>,
    ) -> Vec<TaskResult> {
        let mut join_set = JoinSet::new();
        let total = tasks.len();
        
        for task in tasks {
            let client = self.client.clone();
            let workspace = self.workspace.clone();
            let worker_idx = self.select_worker();
            let config = self.config.workers[worker_idx].clone();
            let agent_id = config.id.unwrap_or_else(AgentId::new);
            
            join_set.spawn(async move {
                let mut agent = Agent::new(client, config, workspace);
                let result = agent.run(&task.prompt).await;
                
                TaskResult {
                    task_id: task.id,
                    agent_id,
                    success: result.is_ok(),
                    output: result.ok().map(|r| r.text),
                    error: result.err().map(|e| e.to_string()),
                }
            });
        }
        
        // Collect results as they complete
        let mut results = Vec::with_capacity(total);
        while let Some(res) = join_set.join_next().await {
            match res {
                Ok(result) => results.push(result),
                Err(join_error) => {
                    results.push(TaskResult {
                        task_id: String::new(),
                        agent_id: AgentId::new(),
                        success: false,
                        output: None,
                        error: Some(join_error.to_string()),
                    });
                }
            }
        }
        
        results
    }
}
```

**Timeline (Concurrent):**
```
Task 1: |████████████| 2s
Task 2: |████████████| 2s
Task 3: |████████████| 2s
Total: 2s (3x faster!)
```

### 3.3 Alternative: FuturesUnordered

**When to use:**
- Need to process results in specific order (not completion order)
- Need to cancel specific futures
- Need more control over polling

**Implementation:**
```rust
use futures::stream::{FuturesUnordered, StreamExt};

impl<C: LLMClient + Clone + 'static> Supervisor<C> {
    pub async fn dispatch_ordered(
        &mut self,
        tasks: Vec<Task>,
    ) -> Vec<TaskResult> {
        let mut futures = FuturesUnordered::new();
        
        for (idx, task) in tasks.into_iter().enumerate() {
            let future = self.dispatch_single(task);
            futures.push(async move {
                (idx, future.await)
            });
        }
        
        let mut results = vec![None; futures.len()];
        
        while let Some((idx, result)) = futures.next().await {
            results[idx] = Some(result);
        }
        
        results.into_iter().map(Option::unwrap).collect()
    }
}
```

### 3.4 Comparison

| Feature | JoinSet | FuturesUnordered |
|---------|---------|------------------|
| Completion order | Yes | Yes |
| Input order | No | Can preserve |
| Panic handling | Built-in | Manual |
| Cancellation | Abort all | Individual |
| Memory | Lower | Higher |
| Complexity | Simple | Medium |

**Recommendation:** Use `JoinSet` by default. Use `FuturesUnordered` only when you need ordered results.

### 3.5 Apply to All Patterns

**Supervisor:**
```rust
impl Supervisor {
    pub async fn broadcast(&mut self, prompt: &str) -> Vec<TaskResult> {
        let mut join_set = JoinSet::new();
        
        for worker_config in &self.config.workers {
            let future = self.run_worker(worker_config.clone(), prompt);
            join_set.spawn(future);
        }
        
        // Collect all results
        let mut results = Vec::new();
        while let Some(res) = join_set.join_next().await {
            results.push(res.unwrap());
        }
        results
    }
}
```

**Race:**
```rust
impl Race {
    pub async fn run(&mut self, task: &str) -> RaceResult {
        let mut join_set = JoinSet::new();
        
        for config in &self.agents {
            let future = self.run_agent(config.clone(), task);
            join_set.spawn(future);
        }
        
        let mut winner = None;
        let mut all_results = HashMap::new();
        let start = Instant::now();
        
        while let Some(res) = join_set.join_next().await {
            let (agent_id, result) = res.unwrap();
            let duration = start.elapsed();
            
            all_results.insert(agent_id, result.clone());
            
            // Check stop condition
            match &self.config.stop_on {
                StopCondition::FirstSuccess => {
                    if result.is_ok() {
                        winner = Some((agent_id, duration));
                        join_set.shutdown().await; // Cancel remaining
                        break;
                    }
                }
                StopCondition::FirstComplete => {
                    winner = Some((agent_id, duration));
                    join_set.shutdown().await;
                    break;
                }
                _ => {}
            }
        }
        
        RaceResult {
            winner: winner.map(|(id, _)| id),
            winner_result: winner.and_then(|(id, _)| {
                all_results.get(&id).and_then(|r| r.clone().ok())
            }),
            all_results,
            ranking: vec![],
        }
    }
}
```

**Pipeline (Parallel Stages):**
```rust
impl Pipeline {
    /// Execute stages in parallel where possible
    pub async fn run_parallel(&self, input: Value) -> PipelineResult {
        // Build dependency graph
        let graph = self.build_dependency_graph();
        
        // Execute in topological order with parallel stages
        let mut completed: HashMap<String, Value> = HashMap::new();
        let mut results = Vec::new();
        
        for stage_group in graph.topological_groups() {
            // Stages in same group can run in parallel
            let mut join_set = JoinSet::new();
            
            for stage_name in &stage_group {
                let stage = self.stages.get(stage_name).unwrap();
                let input = self.get_input(&completed, stage);
                
                join_set.spawn(async move {
                    (stage_name.clone(), stage.run(input).await)
                });
            }
            
            // Collect parallel stage results
            while let Some(res) = join_set.join_next().await {
                let (name, result) = res.unwrap();
                completed.insert(name.clone(), result.output.clone());
                results.push(result);
            }
        }
        
        PipelineResult::from_stages(results)
    }
}
```

---

## 4. Backpressure Control

### 4.1 The Problem

**Unlimited Concurrency:**
```rust
// What happens with 1000 tasks?
for task in tasks {
    join_set.spawn(async move {
        agent.run(&task.prompt).await  // All 1000 start immediately!
    });
}
```

**Failure Modes:**

```
1. API Rate Limiting
   - LLM API allows 10 RPM (requests per minute)
   - 1000 concurrent requests → 990 get 429 errors

2. Memory Exhaustion
   - Each task uses ~50MB memory
   - 1000 tasks = 50GB → OOM

3. Connection Pool Exhaustion
   - HTTP client has 100 connection limit
   - 1000 tasks → 900 wait for connections

4. CPU Starvation
   - Context switching overhead
   - All tasks slow down
```

### 4.2 Solution: Semaphore-Based Control

**What is a Semaphore?**
- A counter that limits concurrent access
- Acquire decrements, release increments
- Blocks when counter is zero

**Implementation:**
```rust
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Configuration for concurrency limits
pub struct ConcurrencyConfig {
    /// Maximum concurrent LLM requests
    pub max_llm_concurrent: usize,
    
    /// Maximum concurrent tool executions
    pub max_tool_concurrent: usize,
    
    /// Maximum concurrent file operations
    pub max_io_concurrent: usize,
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            max_llm_concurrent: 10,   // API rate limit
            max_tool_concurrent: 20,  // CPU bound
            max_io_concurrent: 50,    // I/O bound
        }
    }
}

/// Resource limiter with semaphores
pub struct ResourceLimiter {
    llm_semaphore: Arc<Semaphore>,
    tool_semaphore: Arc<Semaphore>,
    io_semaphore: Arc<Semaphore>,
}

impl ResourceLimiter {
    pub fn new(config: ConcurrencyConfig) -> Self {
        Self {
            llm_semaphore: Arc::new(Semaphore::new(config.max_llm_concurrent)),
            tool_semaphore: Arc::new(Semaphore::new(config.max_tool_concurrent)),
            io_semaphore: Arc::new(Semaphore::new(config.max_io_concurrent)),
        }
    }
    
    /// Acquire a permit for LLM request
    pub async fn acquire_llm(&self) -> SemaphorePermit {
        self.llm_semaphore.acquire().await.unwrap()
    }
    
    /// Acquire a permit for tool execution
    pub async fn acquire_tool(&self) -> SemaphorePermit {
        self.tool_semaphore.acquire().await.unwrap()
    }
    
    /// Acquire a permit for I/O operation
    pub async fn acquire_io(&self) -> SemaphorePermit {
        self.io_semaphore.acquire().await.unwrap()
    }
}

pub type SemaphorePermit = tokio::sync::OwnedSemaphorePermit;
```

### 4.3 Integration with Agent

```rust
impl<C: LLMClient + Clone + 'static> Agent<C> {
    async fn run_with_limits(&mut self, prompt: &str) -> Result<RunResult> {
        // Acquire LLM permit before making request
        let _llm_permit = self.limiter.acquire_llm().await;
        
        let mut result = RunResult::default();
        let mut should_continue = true;
        
        while should_continue {
            let mut stream = {
                let history = self.history.read().await;
                self.client.create_message_stream(
                    &self.config.system_prompt,
                    &history,
                    &self.tool_schemas,
                    8000,
                ).await?
            };
            
            while let Some(event) = stream.next().await {
                match event? {
                    StreamEvent::TextDelta(text) => {
                        result.text.push_str(&text);
                    }
                    StreamEvent::ToolCallDone(_) => {
                        // Execute tool with its own permit
                        let _tool_permit = self.limiter.acquire_tool().await;
                        self.execute_tool(...).await?;
                    }
                    _ => {}
                }
            }
        }
        
        Ok(result)
    }
}
```

### 4.4 Integration with Supervisor

```rust
impl<C: LLMClient + Clone + 'static> Supervisor<C> {
    pub async fn dispatch_batch(
        &mut self,
        tasks: Vec<Task>,
    ) -> Vec<TaskResult> {
        let mut join_set = JoinSet::new();
        let limiter = self.limiter.clone();
        
        for task in tasks {
            let client = self.client.clone();
            let workspace = self.workspace.clone();
            let config = self.select_worker_config();
            let limiter = limiter.clone();
            
            join_set.spawn(async move {
                // Semaphore is acquired inside Agent::run_with_limits
                let mut agent = Agent::new_with_limiter(
                    client,
                    config,
                    workspace,
                    limiter,
                );
                
                agent.run_with_limits(&task.prompt).await
            });
        }
        
        // Collect results
        let mut results = Vec::new();
        while let Some(res) = join_set.join_next().await {
            results.push(res.unwrap());
        }
        results
    }
}
```

### 4.5 Dynamic Semaphore Adjustment

**Advanced: Adjust limits based on errors:**

```rust
pub struct AdaptiveLimiter {
    semaphore: Arc<Semaphore>,
    current_limit: AtomicUsize,
    error_rate: AtomicF64,
}

impl AdaptiveLimiter {
    /// Called when request succeeds
    pub fn record_success(&self) {
        // Gradually increase limit if error rate is low
        let error_rate = self.error_rate.load(Ordering::Relaxed);
        if error_rate < 0.01 {
            let current = self.current_limit.load(Ordering::Relaxed);
            if current < self.max_limit {
                self.semaphore.add_permits(1);
                self.current_limit.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    
    /// Called when request fails with rate limit
    pub fn record_rate_limit(&self) {
        // Decrease limit on rate limit errors
        let current = self.current_limit.load(Ordering::Relaxed);
        if current > self.min_limit {
            // Note: Semaphore doesn't have remove_permits
            // Need to recreate or use different approach
            self.current_limit.fetch_sub(1, Ordering::Relaxed);
        }
        
        // Update error rate
        self.error_rate.fetch_update(Ordering::Relaxed, |rate| {
            Some(rate * 0.9 + 0.1)  // Exponential moving average
        }).ok();
    }
}
```

### 4.6 Configuration

```rust
// In config
pub struct AgentConfig {
    // ... existing fields ...
    
    /// Concurrency limits
    pub concurrency: ConcurrencyConfig,
}

// In .env
MAX_LLM_CONCURRENT=10
MAX_TOOL_CONCURRENT=20
MAX_IO_CONCURRENT=50
```

---

## 5. Delta State Management

### 5.1 Problem Analysis

**Current Full Snapshot:**
```rust
pub struct WorkspaceSnapshot {
    pub state: StateSnapshot,  // Entire KV store
    pub commits: HashMap<CommitId, Commit>,  // All commits
    pub branches: HashMap<String, Branch>,   // All branches
}
```

**Example:**
```
State size: 1 GB
Key update: "counter" = 5 → 6

Current: Write 1 GB
Ideal:   Write 10 bytes (delta)
```

### 5.2 Delta Update Structure

**Delta Operation Types:**
```rust
/// A single state change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub operation: DeltaOp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeltaOp {
    /// Set a key to a value
    Set {
        key: String,
        value: Value,
    },
    
    /// Delete a key
    Delete {
        key: String,
    },
    
    /// Update nested field
    Update {
        key: String,
        path: Vec<String>,  // JSON path
        value: Value,
    },
    
    /// Merge with existing value
    Merge {
        key: String,
        value: Value,
    },
}
```

### 5.3 Delta-Aware State

```rust
pub struct DeltaState {
    /// Base snapshot (full state at a point in time)
    base: StateSnapshot,
    
    /// Accumulated deltas since base
    deltas: Vec<Delta>,
    
    /// In-memory cache (computed from base + deltas)
    cache: HashMap<String, Value>,
    
    /// Dirty keys (need flush)
    dirty: HashSet<String>,
}

impl DeltaState {
    /// Read a value
    pub fn get(&self, key: &str) -> Option<&Value> {
        // Check cache first
        self.cache.get(key)
    }
    
    /// Write a value (creates delta)
    pub fn set(&mut self, key: String, value: Value) {
        let delta = Delta {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation: DeltaOp::Set {
                key: key.clone(),
                value: value.clone(),
            },
        };
        
        self.deltas.push(delta);
        self.cache.insert(key.clone(), value);
        self.dirty.insert(key);
    }
    
    /// Flush deltas to storage
    pub async fn flush(&mut self, storage: &Storage) -> Result<()> {
        if self.deltas.is_empty() {
            return Ok(());
        }
        
        // Append deltas to log
        for delta in &self.deltas {
            storage.append_delta(delta).await?;
        }
        
        // Optionally: compact when too many deltas
        if self.deltas.len() > 1000 {
            self.compact(storage).await?;
        }
        
        self.deltas.clear();
        self.dirty.clear();
        
        Ok(())
    }
    
    /// Compact deltas into new base snapshot
    pub async fn compact(&mut self, storage: &Storage) -> Result<()> {
        // Compute new base from current state
        let new_base = StateSnapshot::from_cache(&self.cache);
        
        // Write new base
        storage.write_snapshot(&new_base).await?;
        
        // Clear old deltas
        self.base = new_base;
        self.deltas.clear();
        
        Ok(())
    }
    
    /// Reconstruct state from storage
    pub async fn load(storage: &Storage) -> Result<Self> {
        // Load base snapshot
        let base = storage.load_snapshot().await?
            .unwrap_or_default();
        
        // Replay deltas
        let deltas = storage.load_deltas().await?;
        
        // Build cache
        let mut cache = base.data.clone();
        for delta in &deltas {
            match &delta.operation {
                DeltaOp::Set { key, value } => {
                    cache.insert(key.clone(), value.clone());
                }
                DeltaOp::Delete { key } => {
                    cache.remove(key);
                }
                DeltaOp::Update { key, path, value } => {
                    if let Some(existing) = cache.get_mut(key) {
                        Self::apply_update(existing, path, value);
                    }
                }
                DeltaOp::Merge { key, value } => {
                    if let Some(existing) = cache.get_mut(key) {
                        Self::apply_merge(existing, value);
                    }
                }
            }
        }
        
        Ok(Self {
            base,
            deltas,
            cache,
            dirty: HashSet::new(),
        })
    }
}
```

### 5.4 Workspace Integration

```rust
impl Workspace {
    /// Create a commit with deltas
    pub fn commit_delta(&mut self, message: &str, author: AgentId) -> Result<CommitId> {
        // Get pending deltas
        let deltas = self.state.take_deltas();
        
        if deltas.is_empty() {
            return Err(CoreError::NoChanges);
        }
        
        // Create commit
        let commit = Commit {
            id: CommitId::new(),
            parent: Some(self.current_branch().head),
            deltas: Some(deltas),  // Store deltas, not full snapshot
            snapshot: None,        // Don't store full snapshot
            message: message.to_string(),
            author,
            timestamp: Utc::now(),
        };
        
        let commit_id = commit.id;
        self.commits.insert(commit_id, commit);
        
        // Update branch head
        self.current_branch_mut().head = commit_id;
        
        Ok(commit_id)
    }
    
    /// Get full state (computed on demand)
    pub fn state(&self) -> StateSnapshot {
        // Reconstruct from commit history
        let mut state = StateSnapshot::default();
        
        for commit in self.commit_history() {
            if let Some(deltas) = &commit.deltas {
                for delta in deltas {
                    state.apply(delta);
                }
            } else if let Some(snapshot) = &commit.snapshot {
                // Legacy: full snapshot
                state = snapshot.clone();
            }
        }
        
        state
    }
}
```

### 5.5 Storage Format

**File Structure:**
```
.amadeus/
├── snapshot.json       # Base snapshot (created on compact)
├── deltas.jsonl        # Delta log (append-only)
├── events.jsonl        # Event log
└── config.json         # Workspace config
```

**Delta Log (deltas.jsonl):**
```jsonl
{"id":"...","timestamp":"2024-01-15T10:30:00Z","operation":{"Set":{"key":"counter","value":1}}}
{"id":"...","timestamp":"2024-01-15T10:30:01Z","operation":{"Set":{"key":"counter","value":2}}}
{"id":"...","timestamp":"2024-01-15T10:30:02Z","operation":{"Set":{"key":"status","value":"running"}}}
```

### 5.6 Performance Comparison

| Operation | Full Snapshot | Delta | Improvement |
|-----------|---------------|-------|-------------|
| Set 1 key (1GB state) | Write 1GB | Write 100 bytes | 10,000,000x |
| Load 1 key | Read 1GB | Read base + deltas | ~1000x (cached) |
| Commit | Write 1GB | Append 100 bytes | 10,000,000x |
| Compact | N/A | Write 1GB | Occasional |

### 5.7 Trade-offs

**Delta Approach:**

| Pro | Con |
|-----|-----|
| Fast writes | Slower reads (need replay) |
| Small storage | Complexity in implementation |
| Append-only (crash-safe) | Need periodic compaction |
| Better for large state | Overhead for small state |

**Recommendation:**
- Use delta approach when state > 10 MB
- Use full snapshot for small states
- Hybrid: snapshot every N commits, deltas in between

---

## 6. Implementation Roadmap

### 6.1 Priority Matrix

| Improvement | Impact | Effort | Priority | Phase |
|-------------|--------|--------|----------|-------|
| Concurrent Execution (JoinSet) | High | Low | P0 | 1 |
| Backpressure (Semaphores) | High | Medium | P0 | 1 |
| Actor-based Agents | Medium | High | P1 | 2 |
| Delta State | Medium | High | P2 | 3 |

### 6.2 Phase 1: Critical Performance Fixes (1 week)

**Goal:** Fix sequential execution and resource exhaustion

**Tasks:**
- [ ] Add `tokio::task::JoinSet` dependency
- [ ] Refactor `Supervisor::dispatch_batch` to use `JoinSet`
- [ ] Refactor `Supervisor::broadcast` to use `JoinSet`
- [ ] Refactor `Race::run` to use `JoinSet`
- [ ] Add `ConcurrencyConfig` struct
- [ ] Add `ResourceLimiter` with semaphores
- [ ] Integrate `ResourceLimiter` into `Agent`
- [ ] Add configuration options to `.env`
- [ ] Write tests for concurrent execution
- [ ] Write tests for backpressure

**Acceptance Criteria:**
- `dispatch_batch` with 5 tasks completes in ~2s (not 10s)
- `dispatch_batch` with 100 tasks respects semaphore limit
- No 429 errors under load

### 6.3 Phase 2: Actor-Based Agents (2 weeks)

**Goal:** Persistent agent lifecycle management

**Tasks:**
- [ ] Design `AgentMessage` enum
- [ ] Implement `AgentActor` struct
- [ ] Implement `AgentHandle` for external communication
- [ ] Add pause/resume/cancel support
- [ ] Migrate `Supervisor` to use `AgentHandle`
- [ ] Migrate `Race` to use `AgentHandle`
- [ ] Add agent monitoring/status API
- [ ] Write comprehensive tests
- [ ] Update documentation

**Acceptance Criteria:**
- Agents persist across multiple tasks
- Pause/resume works correctly
- Connection reuse is verified

### 6.4 Phase 3: Delta State (1 week)

**Goal:** Efficient state management for large states

**Tasks:**
- [ ] Design `Delta` and `DeltaOp` types
- [ ] Implement `DeltaState` wrapper
- [ ] Add delta storage to `Storage`
- [ ] Implement compaction logic
- [ ] Integrate with `Workspace`
- [ ] Add metrics (state size, delta count)
- [ ] Write benchmarks
- [ ] Write tests

**Acceptance Criteria:**
- Single key update writes < 1KB
- Load time for 1GB state < 100ms (cached)
- Compaction reduces delta log by > 90%

### 6.5 Migration Strategy

**Non-Breaking Approach:**

```rust
// Phase 1: Add new methods alongside old
impl Supervisor {
    // Keep old method
    pub async fn dispatch_batch(&mut self, tasks: Vec<Task>) -> Vec<TaskResult> {
        // Mark as deprecated
        #[deprecated(note = "Use dispatch_concurrent for better performance")]
        { /* old sequential implementation */ }
    }
    
    // Add new method
    pub async fn dispatch_concurrent(&mut self, tasks: Vec<Task>) -> Vec<TaskResult> {
        // New concurrent implementation
    }
}

// Phase 2: Swap defaults
impl Supervisor {
    pub async fn dispatch_batch(&mut self, tasks: Vec<Task>) -> Vec<TaskResult> {
        // Now calls concurrent internally
        self.dispatch_concurrent(tasks).await
    }
}

// Phase 3: Remove old code
impl Supervisor {
    pub async fn dispatch_batch(&mut self, tasks: Vec<Task>) -> Vec<TaskResult> {
        // Only concurrent remains
    }
}
```

### 6.6 Testing Strategy

**Unit Tests:**
```rust
#[tokio::test]
async fn test_concurrent_execution() {
    let supervisor = create_test_supervisor();
    let tasks: Vec<Task> = (0..5)
        .map(|i| Task::new(format!("task-{}", i), "say hello"))
        .collect();
    
    let start = Instant::now();
    let results = supervisor.dispatch_concurrent(tasks).await;
    let duration = start.elapsed();
    
    // Should complete in ~2s, not ~10s
    assert!(duration < Duration::from_secs(3));
    assert_eq!(results.len(), 5);
}

#[tokio::test]
async fn test_backpressure_respects_limit() {
    let config = ConcurrencyConfig {
        max_llm_concurrent: 2,
        ..Default::default()
    };
    let limiter = ResourceLimiter::new(config);
    
    let mut concurrent = 0;
    let max_concurrent = Arc::new(AtomicUsize::new(0));
    
    let mut join_set = JoinSet::new();
    for _ in 0..10 {
        let limiter = limiter.clone();
        let max = max_concurrent.clone();
        
        join_set.spawn(async move {
            let _permit = limiter.acquire_llm().await;
            concurrent += 1;
            max.fetch_max(concurrent, Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(100)).await;
            concurrent -= 1;
        });
    }
    
    while join_set.join_next().await.is_some() {}
    
    // Should never exceed 2 concurrent
    assert!(max_concurrent.load(Ordering::Relaxed) <= 2);
}
```

**Integration Tests:**
```rust
#[tokio::test]
async fn test_large_batch_with_backpressure() {
    let supervisor = create_real_supervisor().await;
    let tasks: Vec<Task> = (0..100)
        .map(|i| Task::new(format!("task-{}", i), "compute 2+2"))
        .collect();
    
    // Should not:
    // - Run out of memory
    // - Hit API rate limits
    // - Take forever
    let results = supervisor.dispatch_concurrent(tasks).await;
    
    assert_eq!(results.len(), 100);
    assert!(results.iter().all(|r| r.success));
}
```

**Benchmarks:**
```rust
#[bench]
fn bench_sequential_vs_concurrent(b: &mut Bencher) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    b.iter(|| {
        rt.block_on(async {
            let supervisor = create_test_supervisor();
            let tasks = create_test_tasks(10);
            
            let sequential_time = time(|| {
                supervisor.dispatch_batch_sequential(tasks.clone())
            }).await;
            
            let concurrent_time = time(|| {
                supervisor.dispatch_concurrent(tasks)
            }).await;
            
            assert!(concurrent_time < sequential_time / 5);
        });
    });
}
```

---

## Appendix A: Dependencies

**Add to Cargo.toml:**
```toml
[dependencies]
# Existing dependencies...

# For concurrent execution
tokio = { version = "1.39", features = ["full", "tracing"] }

# For semaphores (included in tokio::sync)
# No additional dependency needed

# For futures utilities
futures = "0.3"

# For delta serialization
serde_json = "1.0"
```

---

## Appendix B: Configuration Reference

**.env:**
```bash
# Concurrency limits
MAX_LLM_CONCURRENT=10
MAX_TOOL_CONCURRENT=20
MAX_IO_CONCURRENT=50

# Delta settings
DELTA_COMPACT_THRESHOLD=1000
SNAPSHOT_INTERVAL_MINUTES=60

# Actor settings
AGENT_MAILBOX_SIZE=64
AGENT_SHUTDOWN_TIMEOUT_SECONDS=30
```

**Code:**
```rust
let config = AgentConfig {
    concurrency: ConcurrencyConfig {
        max_llm_concurrent: env("MAX_LLM_CONCURRENT", 10),
        max_tool_concurrent: env("MAX_TOOL_CONCURRENT", 20),
        max_io_concurrent: env("MAX_IO_CONCURRENT", 50),
    },
    // ...
};
```

---

## Appendix C: Monitoring & Metrics

**Key Metrics to Track:**

```rust
pub struct AgentMetrics {
    // Concurrency
    pub active_llm_requests: Gauge,
    pub active_tool_executions: Gauge,
    pub queued_tasks: Gauge,
    
    // Performance
    pub task_duration: Histogram,
    pub wait_time: Histogram,
    
    // Resources
    pub state_size: Gauge,
    pub delta_count: Gauge,
    
    // Errors
    pub rate_limit_errors: Counter,
    pub timeout_errors: Counter,
}
```

**Expose via API:**
```rust
// GET /metrics
{
    "active_llm_requests": 8,
    "queued_tasks": 15,
    "avg_task_duration_ms": 2340,
    "state_size_mb": 128,
    "delta_count": 543
}
```

---

## Conclusion

These architectural improvements transform Amadeus from a prototype into a production-ready system:

| Before V3 | After V3 |
|-----------|----------|
| Sequential execution | Fully concurrent |
| No resource limits | Bounded concurrency |
| Ephemeral agents | Persistent actors |
| Full snapshot overhead | Delta updates |

**Impact:**
- **10x faster** batch processing (concurrent)
- **No crashes** under load (backpressure)
- **Lower latency** (connection reuse)
- **Scalable state** (delta updates)

**Next Steps:**
1. Review and approve this document
2. Begin Phase 1 implementation
3. Write comprehensive tests
4. Benchmark and validate improvements

---

*Document Version: 1.0*
*Last Updated: 2026-02-20*
*Author: Amadeus Team*
