# Amadeus V3 Architecture: Production-Ready Multi-Agent System

> Performance optimization and scalability improvements based on production requirements

## Executive Summary

This document outlines the architectural improvements implemented in Amadeus V3, transforming it from a prototype into a production-ready multi-agent orchestration system.

**Implemented Features:**
1. **Concurrent Execution** - Replaced sequential loops with `tokio::task::JoinSet` for parallel task processing.
2. **Task Queuing & Backpressure** - Centralized `TaskQueue` in the Supervisor with configurable capacity (`max_pending_tasks`).
3. **P2P Recursive Delegation** - Integrated `PeerTool` for agents to delegate sub-tasks to specialized workers.
4. **Resilient Error Handling** - Deadlock prevention and immediate error propagation for saturated workers.

---

## 1. Current Architecture Analysis

### 1.1 Orchestration Model

Amadeus uses a **Supervisor-Worker** pattern where a central supervisor manages a pool of specialized agents. 

| Feature | Implementation |
|---------|----------------|
| **Concurrency** | Parallel task execution via `JoinSet` |
| **Queuing** | Async `VecDeque` with periodic processing |
| **Load Balancing** | `LeastLoaded`, `RoundRobin`, and `CapabilityMatch` strategies |
| **P2P Help** | Recursive sub-tasking via the `HelpRequest` bus |

### 1.2 Performance Impact

**Concurrent Execution:**
Tasks are spawned as independent Tokio tasks. In a batch of 5 tasks taking 2s each, total time is ~2s instead of 10s (5x speedup).

**Backpressure Control:**
The `SupervisorConfig::max_pending_tasks` (default: 100) prevents OOM and API exhaustion by rejecting new tasks when the buffer is full.

---

## 2. Multi-Agent Orchestration

### 2.1 The Supervisor Loop

The Supervisor runs a reactive background loop that handles two main event sources:
1. **P2P Help Requests**: Incoming from agents via `HelpRequest` channels.
2. **Task Queue**: Periodic processing of pending tasks whenever workers become available.

```rust
pub async fn run(&self) -> Result<()> {
    loop {
        tokio::select! {
            help_req = self.help_rx.recv() => {
                // Dispatch or fail immediately if no workers
            }
            _ = interval.tick() => {
                self.process_queue().await;
            }
        }
    }
}
```

### 2.2 Task Buffering

When `Supervisor::execute` is called and no workers are immediately available, the task is pushed into a `VecDeque`. This ensures that bursty traffic doesn't fail immediately, provided it stays within the `max_pending_tasks` limit.

### 2.3 Dispatch Strategies

The Supervisor supports three load balancing strategies for distributing tasks across worker agents:

```
┌─────────────────────────────────────────────────────────────────┐
│                        SUPERVISOR                               │
│                                                                 │
│   Task Queue: [Task1, Task2, Task3, ...]                       │
│                                                                 │
│   ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐         │
│   │Worker A │  │Worker B │  │Worker C │  │Worker D │         │
│   │ 2 tasks │  │ 0 tasks │  │ 3 tasks │  │ 1 task  │         │
│   │ [bash]  │  │ [web]   │  │ [file]  │  │ [bash]  │         │
│   └─────────┘  └─────────┘  └─────────┘  └─────────┘         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

#### RoundRobin (default)

Cycles through workers in order, regardless of current load.

```rust
DispatchStrategy::RoundRobin => {
    let mut next_idx = next_idx_mutex.lock().await;
    let idx = *next_idx % candidates.len();
    *next_idx += 1;
    Some(candidates[idx].0)
}
```

**Example:**
```
Task 1 → Worker A
Task 2 → Worker B
Task 3 → Worker C
Task 4 → Worker A  (cycles back)
Task 5 → Worker B
```

| Pros | Cons |
|------|------|
| Simple, predictable | Ignores current workload |
| Even distribution over time | Busy workers get equal share |
| No state tracking beyond index | Can queue on overloaded workers |

**Best for:** Homogeneous tasks, equal worker capacity

#### LeastLoaded

Always picks the worker with the fewest active tasks.

```rust
DispatchStrategy::LeastLoaded => candidates
    .iter()
    .min_by_key(|(_, info)| info.active_tasks)
    .map(|(id, _)| *id)
```

**Example:**
```
Current state:  A(2 tasks), B(0 tasks), C(3 tasks), D(1 task)
Task arrives → Worker B (has 0 active tasks)
Next state:    A(2), B(1), C(3), D(1)
Another task → Worker B or D (tie, both have 1)
```

| Pros | Cons |
|------|------|
| Balances load dynamically | Requires tracking active_tasks per worker |
| Prevents hot spots | Doesn't consider task complexity |
| Better resource utilization | Race conditions possible (handled via locks) |

**Best for:** Variable task durations, uneven workloads

#### CapabilityMatch

First filters workers by required capabilities, then picks least loaded among them.

```rust
DispatchStrategy::CapabilityMatch => candidates
    .iter()
    .filter(|(_, info)| info.has_capabilities(&task.required_capabilities))
    .min_by_key(|(_, info)| info.active_tasks)
    .map(|(id, _)| *id)
```

**Example:**
```
Workers:
  Worker A - capabilities: [bash, file]     - active: 2
  Worker B - capabilities: [web, search]    - active: 0
  Worker C - capabilities: [bash, docker]   - active: 1
  Worker D - capabilities: [web, bash]      - active: 3

Task arrives requiring: [bash]
Eligible: A, C, D (all have bash)
Selected: Worker C (has bash, lowest active count = 1)
```

| Pros | Cons |
|------|------|
| Routes tasks to specialized workers | Can fail if no capable worker available |
| Prevents dispatching to incapable workers | More complex matching logic |
| Combines capability filtering with load balancing | Requires capability declaration at spawn |

**Best for:** Heterogeneous workers, specialized tasks (e.g., web scraping, code execution)

#### Strategy Selection Guide

| Scenario | Best Strategy | Why |
|----------|---------------|-----|
| Quick uniform tasks | RoundRobin | Simplicity, even spread |
| Mixed short/long tasks | LeastLoaded | Prevents queuing |
| Specialized workers | CapabilityMatch | Routes to right worker |
| Unknown task profiles | LeastLoaded | Safe default |

#### Configuration

Set the dispatch strategy in `SupervisorConfig`:

```rust
let config = SupervisorConfig {
    strategy: DispatchStrategy::LeastLoaded,
    max_pending_tasks: 100,
    task_timeout: Duration::from_secs(300),
    retry_failed_tasks: true,
    max_retries: 3,
};
```

#### Worker Capabilities

Workers declare capabilities when spawned:

```rust
let worker_configs = vec![
    WorkerConfig {
        name: "code-executor".to_string(),
        capabilities: vec!["bash".to_string(), "file".to_string()],
        max_concurrent: 3,
        ..Default::default()
    },
    WorkerConfig {
        name: "web-scraper".to_string(),
        capabilities: vec!["web".to_string(), "search".to_string()],
        max_concurrent: 2,
        ..Default::default()
    },
];
```

---

## 3. P2P Collaboration (Help System)

### 3.1 The PeerTool

Agents are initialized with a `PeerTool`, which allows them to send `HelpRequest`s back to the Supervisor. This enables recursive collaboration where a Coder agent can ask a Reviewer agent for feedback mid-task.

### 3.2 Deadlock Prevention

To prevent circular dependency deadlocks (e.g., Worker A waits for Worker B, who is waiting for Worker A), the Supervisor implements:
1. **Timeout Enforcement**: Every task has a `task_timeout`.
2. **Saturation Errors**: If a help request cannot be fulfilled because all potential workers are busy, it returns an error immediately rather than queuing indefinitely (which would block the requester).

---

## 4. Implementation Status

| Improvement | Status | Phase |
|-------------|--------|-------|
| Concurrent Execution (JoinSet) | ✅ Implemented | Phase 1 |
| Task Queuing & Backpressure | ✅ Implemented | Phase 1 |
| P2P Help System | ✅ Implemented | Phase 1 |
| Actor-based Agents | ⏳ Planned | Phase 2 |
| Delta State Management | ⏳ Planned | Phase 3 |

---

## 5. Next Steps

1. **Phase 2: Actor-Based Agents** - Transform agents into persistent tasks with mailboxes to support `Pause`/`Resume` and better state persistence.
2. **Phase 3: Delta State** - Implement surgical state updates to handle large workspaces efficiently.

---

*Document Version: 1.2 (Added dispatch strategies documentation)*
*Last Updated: 2026-03-02*
*Author: Amadeus Team*
