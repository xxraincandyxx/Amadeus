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

*Document Version: 1.1 (Updated to reflect current implementation)*
*Last Updated: 2026-02-27*
*Author: Amadeus Team*
