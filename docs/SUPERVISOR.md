# Supervisor Mode Guide

The Supervisor pattern enables multi-agent orchestration with a pool of worker agents. Workers can collaborate by requesting help from each other, and the supervisor intelligently dispatches tasks based on capabilities and load.

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Quick Start](#quick-start)
4. [Worker Configuration](#worker-configuration)
5. [Dispatch Strategies](#dispatch-strategies)
6. [Inter-Agent Communication](#inter-agent-communication)
7. [File Locking (Concurrency Control)](#file-locking-concurrency-control)
8. [Advanced Usage](#advanced-usage)

---

## Overview

The Supervisor manages a pool of specialized worker agents:

```
┌─────────────────────────────────────────────────────────────┐
│                        Supervisor                           │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Task Queue (with backpressure)                      │   │
│  └──────────────────────────────────────────────────────┘   │
│                         ↓ ↑                                 │
│  ┌──────────────┐  ┌──────────────┐   ┌─────────────┐       │
│  │  Worker 1    │  │  Worker 2    │   │  Worker N   │       │
│  │  "builder"   │  │  "tester"    │   │  "docs"     │       │
│  │  caps: [code │  │  caps: [test │   │  caps: [docs│       │
│  │      , build]│  │      , bash] │   │      , read]│       │
│  └──────────────┘  └──────────────┘   └─────────────┘       │
│         ↑                 ↑                 ↑               │
│         └─────────────────┼─────────────────┘               │
│                           │                                 │
│              call_peer tool (via channel)                   │
└─────────────────────────────────────────────────────────────┘
```

---

## Architecture

### Core Components

| Component | Description |
|-----------|-------------|
| `Supervisor` | Manages worker pool, dispatches tasks, routes help requests |
| `Worker` | Individual Agent instance with capabilities |
| `PeerTool` | Enables workers to request help from each other |
| `DispatchStrategy` | Algorithm for selecting workers (RoundRobin, LeastLoaded, CapabilityMatch) |
| `FileLockManager` | RW locking for concurrent file access (optional) |

### Key Types

```rust
use amadeus::{
    Supervisor, SupervisorConfig, DispatchStrategy,
    WorkerConfig, Task, AgentId,
    FileLockManager,
};

// Task to dispatch to a worker
Task::new("task-1", "Fix the bug in user authentication")
    .requires(vec!["code".to_string(), "debug".to_string()])
    .priority(10)
```

---

## Quick Start

### Basic Example

```rust
use amadeus::{
    Agent, Supervisor, SupervisorConfig, WorkerConfig,
    LLMClient, Config,
};
use std::sync::Arc;

async fn example<C: LLMClient + Clone + 'static>(client: C) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create supervisor configuration
    let config = SupervisorConfig {
        strategy: DispatchStrategy::CapabilityMatch,
        max_pending_tasks: 100,
        task_timeout: std::time::Duration::from_secs(300),
        retry_failed_tasks: true,
        max_retries: 3,
    };

    // 2. Create agent configuration
    let agent_config = Arc::new(Config::load()?);

    // 3. Create and spawn supervisor
    let mut supervisor = Supervisor::new(client.clone(), config, agent_config.clone());

    // 4. Define workers with capabilities
    let workers = vec![
        WorkerConfig::new("builder")
            .capability("code")
            .capability("build")
            .max_concurrent(2),
        WorkerConfig::new("tester")
            .capability("test")
            .capability("bash")
            .max_concurrent(1),
        WorkerConfig::new("researcher")
            .capability("read")
            .capability("search")
            .max_concurrent(3),
    ];

    let worker_ids = supervisor.spawn(workers).await?;
    println!("Spawned workers: {:?}", worker_ids);

    // 5. Run supervisor in background (handles help requests)
    tokio::spawn(async move {
        if let Err(e) = supervisor.run().await {
            eprintln!("Supervisor error: {}", e);
        }
    });

    // 6. Execute tasks
    let task = Task::new("build-feature", "Implement user authentication")
        .requires(vec!["code".to_string(), "build".to_string()]);

    let result = supervisor.execute(task).await?;

    println!("Task result: {}", result.output.unwrap_or_default());

    Ok(())
}
```

---

## Worker Configuration

### Basic Worker

```rust
// Simple worker with just a name
WorkerConfig::new("worker-1")
```

### Worker with Capabilities

```rust
// Worker specialized for coding tasks
WorkerConfig::new("coder")
    .capability("code")
    .capability("edit")
    .capability("read")
    .max_concurrent(2)
```

### Worker with Custom Model

```rust
// Worker using a faster/cheaper model for simple tasks
WorkerConfig::new("fast-worker")
    .capability("search")
    .capability("read")
    .model("claude-haiku-3-5-20250620")
    .max_concurrent(5)
```

### Full Configuration

```rust
use amadeus::core::id::AgentId;

WorkerConfig::new("specialized-worker")
    .id(Some(AgentId::new()))  // Explicit ID
    .capability("code")
    .capability("debug")
    .capability("test")
    .model("claude-sonnet-4-5-20250929")  // Override model
    .max_concurrent(2)  // Handle up to 2 concurrent tasks
```

---

## Dispatch Strategies

### RoundRobin

Rotates through available workers in order. Good for load distribution when all workers are equal.

```rust
SupervisorConfig {
    strategy: DispatchStrategy::RoundRobin,
    ..Default::default()
}
```

### LeastLoaded

Picks the worker with the fewest active tasks. Best for heterogeneous workloads.

```rust
SupervisorConfig {
    strategy: DispatchStrategy::LeastLoaded,
    ..Default::default()
}
```

### CapabilityMatch (Recommended)

Matches task requirements to worker capabilities. Selects the least loaded worker that has all required capabilities.

```rust
SupervisorConfig {
    strategy: DispatchStrategy::CapabilityMatch,
    ..Default::default()
}
```

**Example with capabilities:**

```rust
// Task requires specific capabilities
let task = Task::new("test-task", "Run the test suite")
    .requires(vec!["test".to_string(), "bash".to_string()]);

// Supervisor will only dispatch to workers with BOTH capabilities
let result = supervisor.execute(task).await?;
```

---

## Inter-Agent Communication

Workers can request help from other workers using the `call_peer` tool:

```rust
// In worker agent prompt, they can call:
// call_peer(task="Help debug this crash", capabilities=["debug", "bash"])
```

### How It Works

1. Worker encounters a task needing specialized help
2. Worker calls `call_peer` tool with task description and required capabilities
3. Tool sends `HelpRequest` to supervisor via channel
4. Supervisor selects appropriate worker based on dispatch strategy
5. Task is dispatched to selected worker
6. Result is returned to requesting worker

### Example: Worker Collaboration

```rust
// Worker 1 ("builder") is implementing a feature but needs test help
// It calls:
call_peer(
    task="Write tests for the user authentication module at src/auth.rs",
    capabilities=["test", "write"]
)

// Supervisor routes to Worker 2 ("tester") which has test capability
// Worker 2 returns test code to Worker 1
```

---

## File Locking (Concurrency Control)

When multiple workers may access the same files, enable file locking to prevent race conditions:

```rust
use amadeus::concurrency::FileLockManager;

// 1. Create shared FileLockManager
let file_lock_manager = Arc::new(FileLockManager::new());

// 2. Pass to supervisor
let mut supervisor = Supervisor::new(
    client.clone(),
    config,
    agent_config.clone(),
);

// Note: File lock manager is accessed via supervisor.lock_manager()
// Workers inherit it when spawned
```

### How File Locking Works

| Operation | Lock Type | Behavior |
|-----------|-----------|----------|
| `read_file` | Shared (Read) | Multiple workers can read simultaneously |
| `write_file` | Exclusive (Write) | Blocks all reads/writes until complete |
| `edit_file` | Exclusive (Write) | Blocks all reads/writes until complete |

### Read Freshness Validation

The system tracks when each worker last read each file:

```rust
// Worker A reads file.txt at time T1
read_file("file.txt")  // acquires read lock, caches: modified_at = T1

// Worker B writes to file.txt at time T2
write_file("file.txt", "new content")  // acquires write lock

// Worker A tries to write to file.txt
write_file("file.txt", "more content")
// ERROR: File was modified at T2 (after Worker A's read at T1)
// Worker A must re-read the file first
```

This prevents overwriting changes made by other workers.

---

## Advanced Usage

### Custom Task Metadata

```rust
let task = Task::new("build", "Build the project")
    .requires(vec!["build".to_string()])
    .priority(100)  // Higher = more urgent (0-255)
    .meta("timeout", serde_json::json!(300));  // Custom metadata
```

### Monitoring Worker Status

```rust
// Get info about a specific worker
if let Some(info) = supervisor.worker(worker_id).await {
    println!("Worker: {}", info.name);
    println!("Status: {:?}", info.status);
    println!("Active tasks: {}", info.active_tasks);
    println!("Completed: {}", info.completed_tasks);
    println!("Errors: {}", info.total_errors);
}
```

### Supervisor with File Locks

```rust
use amadeus::concurrency::FileLockManager;
use std::sync::Arc;

async fn with_file_locks<C: LLMClient + Clone + 'static>(client: C) -> Result<(), Box<dyn std::error::Error>> {
    let agent_config = Arc::new(Config::load()?);

    // Create supervisor
    let mut supervisor = Supervisor::new(
        client,
        SupervisorConfig::default(),
        agent_config,
    );

    // File lock manager is available via supervisor.lock_manager()
    // Each worker's tools will use it for concurrency control

    let workers = vec![
        WorkerConfig::new("worker-1").capability("code"),
    ];

    supervisor.spawn(workers).await?;

    // Run supervisor
    supervisor.run().await?;

    Ok(())
}
```

### Using with Subagents

Workers can also spawn subagents for task decomposition:

```rust
// Worker spawns a subagent for a focused subtask
sub_agnet(
    prompt="Refactor the auth module to use JWT tokens",
    description="JWT refactor"
)
```

Subagents inherit the worker's file lock manager for consistent concurrency control.

---

## Error Handling

```rust
match supervisor.execute(task).await {
    Ok(result) => {
        if result.success {
            println!("Output: {}", result.output.unwrap_or_default());
        } else {
            eprintln!("Error: {}", result.error.unwrap_or_default());
        }
    }
    Err(e) => {
        eprintln!("Execution failed: {}", e);
    }
}
```

### TaskResult Fields

| Field | Type | Description |
|-------|------|-------------|
| `task_id` | String | Unique task identifier |
| `worker_id` | AgentId | Worker that executed the task |
| `success` | bool | Whether task succeeded |
| `output` | Option<String> | Task output if successful |
| `error` | Option<String> | Error message if failed |
| `duration_ms` | u64 | Execution time in milliseconds |
| `tool_calls` | Vec<ToolCall> | Tools used during execution |

---

## Best Practices

1. **Use CapabilityMatch** - Let the supervisor route tasks to appropriate workers
2. **Define clear capabilities** - Match worker capabilities to task requirements
3. **Set appropriate max_concurrent** - Don't overload workers
4. **Enable file locking** - When workers may access same files
5. **Set timeouts** - Prevent long-running tasks from blocking
6. **Enable retries** - Handle transient failures gracefully
7. **Monitor worker stats** - Track completed tasks and errors

---

## Feature Flags

Ensure the `supervisor` feature is enabled:

```toml
# Cargo.toml
[dependencies]
amadeus = { version = "0.1", features = ["supervisor", "full"] }
```

Or enable all features:

```toml
amadeus = { version = "0.1", features = ["full"] }
```

---

## AgentManager (Standalone Multi-Agent)

In addition to the Supervisor pattern, Amadeus provides a simpler `AgentManager` for managing multiple standalone agents without the worker pool architecture.

### When to use AgentManager vs Supervisor

| Use Case | Recommended Pattern |
|----------|---------------------|
| Multiple independent agents with different roles | AgentManager |
| Collaborative problem-solving with capability routing | Supervisor + Workers |
| Simple TUI with agent switching | AgentManager |
| Complex multi-agent task orchestration | Supervisor |

### AgentManager Features

- **Agent Profiles**: Specialized system prompts (default, debug, docs, code_review, custom)
- **Agent Lifecycle**: Create, switch, kill agents
- **API Integration**: REST endpoints for agent management
- **TUI Support**: Agent panel for visual management

### Example

```rust
use amadeus::agent::{AgentManager, AgentProfile};
use amadeus::agent::config::Config;
use amadeus::client::anthropic::AnthropicClient;

// Create manager
let client = AnthropicClient::new(api_key, base_url, model);
let config = Arc::new(Config::load()?);
let mut manager = AgentManager::new(client, config);

// Create agents with different profiles
manager.create_agent(Some("debugger".to_string()), AgentProfile::Debug).await?;
manager.create_agent(Some("docs-writer".to_string()), AgentProfile::Docs).await?;

// List and switch
let agents = manager.list_agents();
manager.switch_next();
```

### API Endpoints

See [REST_API.md](./REST_API.md#5-multi-agent-endpoints) for the HTTP API.
