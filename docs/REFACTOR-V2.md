# Amadeus V2 重构文档

> 基于 "Agent OS" 理念的多 Agent 编排系统重构

## 背景

### 当前架构限制

| 问题 | 表现 |
|------|------|
| 单 Agent | 无法并行任务，无法协作 |
| 状态在内存 | 重启丢失，无法回滚 |
| 线性历史 | 无法分支探索，无法版本控制 |
| 无协作原语 | Agent 间通信困难 |
| 无持久化 | 无法恢复，无法审计 |

### 目标

构建一个 **Agent OS** —— 像操作系统一样管理 Agent 的生命周期、状态、通信和版本。

```
┌─────────────────────────────────────────────────────────────────┐
│                     Amadeus V2 Architecture                     │
├─────────────────────────────────────────────────────────────────┤
│  Scheduler │ Router │ Supervisor │ Checkpointer                │  ← 调度层
├─────────────────────────────────────────────────────────────────┤
│  Workspace │ Branch │ Commit │ EventLog │ StateStore           │  ← 存储层
├─────────────────────────────────────────────────────────────────┤
│  IPC │ PubSub │ RPC │ LockManager │ TransactionManager         │  ← 通信层
├─────────────────────────────────────────────────────────────────┤
│  Agent Pool │ ToolRegistry │ LLMClient Pool                    │  ← 执行层
└─────────────────────────────────────────────────────────────────┘
```

---

## 核心概念

### 1. Workspace（工作空间）

所有 Agent 共享的工作空间，类似 Git repository。

```rust
/// 工作空间 - Agent 协作的共享上下文
pub struct Workspace {
    /// 唯一标识
    pub id: Uuid,

    /// 工作目录
    pub workdir: PathBuf,

    /// 分支系统
    branches: HashMap<String, Branch>,
    active_branch: String,

    /// 事件日志（append-only）
    event_log: EventLog,

    /// 版本化状态
    state: VersionedState,

    /// Agent 注册表
    agents: HashMap<AgentId, AgentMeta>,

    /// 锁管理器
    locks: LockManager,
}

impl Workspace {
    /// 创建新工作空间
    pub async fn create(path: PathBuf) -> Result<Self>;

    /// 从快照恢复
    pub async fn restore(snapshot: Snapshot) -> Result<Self>;

    /// 创建分支
    pub async fn branch(&mut self, name: &str, from: Option<&str>) -> Result<()>;

    /// 切换分支
    pub async fn checkout(&mut self, name: &str) -> Result<()>;

    /// 合并分支
    pub async fn merge(&mut self, from: &str, into: &str, strategy: MergeStrategy) -> Result<()>;

    /// 创建提交
    pub async fn commit(&mut self, message: &str, author: AgentId) -> Result<CommitId>;

    /// 回滚到指定提交
    pub async fn reset(&mut self, to: CommitId, mode: ResetMode) -> Result<()>;

    /// 查看 diff
    pub async fn diff(&self, from: &str, to: &str) -> Result<Diff>;

    /// 查看历史
    pub async fn log(&self, limit: usize) -> Result<Vec<Commit>>;

    /// 创建快照（用于持久化）
    pub async fn snapshot(&self) -> Result<Snapshot>;
}
```

### 2. Branch（分支）

**核心创新**：让 Agent 可以"后悔"，可以并行探索。

```rust
/// 分支 - 独立的工作线
pub struct Branch {
    /// 分支名
    pub name: String,

    /// 父分支和 commit（用于 merge）
    pub parent: Option<(String, CommitId)>,

    /// 提交历史
    commits: Vec<CommitId>,

    /// 当前 HEAD
    head: CommitId,

    /// 创建时间
    created_at: DateTime<Utc>,

    /// 最后更新时间
    updated_at: DateTime<Utc>,
}
```

**使用场景：**

```rust
// 1. 探索性任务 - 试错
ws.branch("experiment-fix", from="main")?;
ws.checkout("experiment-fix")?;
// Agent 执行，如果失败...
ws.checkout("main")?;
ws.delete_branch("experiment-fix")?;

// 2. 并行任务 - 多方案竞争
ws.branch("approach-a", from="main")?;
ws.branch("approach-b", from="main")?;

// 在不同分支并行运行 Agent
let a = ws.run_in_branch("approach-a", agent_a).await?;
let b = ws.run_in_branch("approach-b", agent_b).await?;

// 选择最好的结果
let winner = compare(a, b)?;
ws.merge(winner, into="main")?;

// 3. 回滚 - 恢复到之前状态
let commit = ws.find_commit_by_message("before refactor")?;
ws.reset(commit, ResetMode::Hard)?;
```

### 3. Commit（提交）

状态快照，支持回滚和审计。

```rust
/// 提交 - 状态快照
pub struct Commit {
    /// 唯一标识
    pub id: CommitId,

    /// 父提交
    pub parent: Option<CommitId>,

    /// 状态快照（KV 快照）
    pub state: StateSnapshot,

    /// 提交信息
    pub message: String,

    /// 作者（Agent）
    pub author: AgentId,

    /// 时间戳
    pub timestamp: DateTime<Utc>,

    /// 触发原因
    pub trigger: CommitTrigger,
}

#[derive(Debug, Clone)]
pub enum CommitTrigger {
    /// 用户显式请求
    UserRequest,

    /// Agent 完成关键步骤
    AgentCheckpoint { agent: AgentId, step: String },

    /// 定时自动保存
    AutoSave,

    /// 工具执行前后
    ToolExecution { tool: String, phase: Phase },

    /// 错误恢复点
    RecoveryPoint,
}
```

### 4. Event（事件）

Append-only 事件日志，用于审计和回放。

```rust
/// 事件 - 系统中发生的一切
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    // === Agent 生命周期 ===
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

    // === 思考与行动 ===
    AgentThinking {
        id: AgentId,
        content: String
    },
    AgentAction {
        id: AgentId,
        action: Action
    },

    // === 通信 ===
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

    // === 状态变更 ===
    StateUpdated {
        key: String,
        old: Option<Value>,
        new: Value,
        author: AgentId,
    },

    // === 工具调用 ===
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

    // === 分支与版本 ===
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

    // === 锁与事务 ===
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

/// 事件日志 - append-only 存储
pub struct EventLog {
    events: Vec<Event>,
    index: HashMap<String, Vec<usize>>,  // 索引，快速查找
}

impl EventLog {
    /// 追加事件
    pub fn append(&mut self, event: Event) -> Result<usize>;

    /// 查询事件（支持过滤）
    pub fn query(&self, filter: EventFilter) -> Result<Vec<&Event>>;

    /// 从指定位置回放
    pub fn replay_from(&self, index: usize) -> Result<Vec<Event>>;

    /// 持久化到磁盘
    pub async fn flush(&self, path: &Path) -> Result<()>;
}
```

### 5. State（状态）

版本化的 KV 存储，支持事务。

```rust
/// 版本化状态
pub struct VersionedState {
    /// KV 存储
    data: BTreeMap<String, (Value, Version)>,

    /// 当前版本号
    version: u64,

    /// 锁管理器引用
    locks: Arc<LockManager>,
}

impl VersionedState {
    /// 读取值
    pub async fn read(&self, key: &str) -> Result<Option<Value>>;

    /// 写入值
    pub async fn write(&mut self, key: &str, value: Value) -> Result<()>;

    /// Compare-and-Swap（乐观锁）
    pub async fn cas(
        &mut self,
        key: &str,
        expected: Option<&Value>,
        new: Value
    ) -> Result<bool>;

    /// 原子更新
    pub async fn update<F>(&mut self, key: &str, f: F) -> Result<Value>
    where
        F: FnOnce(Option<&Value>) -> Value;

    /// 批量读取
    pub async fn read_batch(&self, keys: &[&str]) -> Result<HashMap<String, Option<Value>>>;

    /// 批量写入
    pub async fn write_batch(&mut self, updates: HashMap<String, Value>) -> Result<()>;

    /// 创建快照
    pub fn snapshot(&self) -> StateSnapshot;

    /// 从快照恢复
    pub fn restore(&mut self, snapshot: StateSnapshot);
}
```

---

## Agent 系统

### Agent 定义

```rust
/// Agent 配置
pub struct AgentConfig {
    /// Agent ID（自动生成或指定）
    pub id: Option<AgentId>,

    /// 角色/名称
    pub role: String,

    /// 系统提示词
    pub system_prompt: String,

    /// 使用的模型
    pub model: ModelConfig,

    /// 可用工具（None 表示全部）
    pub tools: Option<Vec<String>>,

    /// 最大工具调用次数
    pub max_tool_calls: usize,

    /// 超时时间
    pub timeout: Duration,

    /// 权重（用于调度）
    pub priority: u8,

    /// 重启策略
    pub restart_policy: RestartPolicy,
}

/// Agent 运行时状态
pub struct Agent {
    /// 配置
    config: AgentConfig,

    /// 所属工作空间
    workspace: Arc<Workspace>,

    /// LLM 客户端
    client: Arc<dyn LLMClient>,

    /// 工具注册表（可能是子集）
    tools: ToolRegistry,

    /// 本地状态
    local_state: HashMap<String, Value>,

    /// 运行时状态
    status: AgentStatus,

    /// 统计信息
    stats: AgentStats,
}

impl Agent {
    /// 执行任务（阻塞直到完成）
    pub async fn run(&mut self, input: &str) -> Result<RunResult>;

    /// 执行任务（流式返回事件）
    pub fn run_stream(&mut self, input: &str) -> impl Stream<Item = AgentEvent>;

    /// 执行任务（channel 版本）
    pub async fn run_channel(&mut self, input: &str) -> Result<mpsc::Receiver<AgentEvent>>;

    /// 停止执行
    pub async fn stop(&mut self) -> Result<()>;

    /// 暂停/恢复
    pub async fn pause(&mut self) -> Result<()>;
    pub async fn resume(&mut self) -> Result<()>;

    /// 获取状态
    pub fn status(&self) -> AgentStatus;

    /// 获取统计
    pub fn stats(&self) -> &AgentStats;
}
```

### Agent 事件

```rust
/// Agent 执行事件
#[derive(Debug, Clone)]
pub enum AgentEvent {
    // === 生命周期 ===
    Started { id: AgentId },
    Paused { id: AgentId },
    Resumed { id: AgentId },
    Stopped { id: AgentId, reason: StopReason },

    // === LLM 交互 ===
    TextDelta { delta: String },
    Thinking { content: String },

    // === 工具调用 ===
    ToolStart { id: String, name: String },
    ToolInputDelta { id: String, delta: String },
    ToolComplete {
        id: String,
        name: String,
        input: Value,
        output: String,
        is_error: bool,
    },

    // === 状态变更 ===
    StateUpdated { key: String, old: Option<Value>, new: Value },

    // === 完成 ===
    Done { result: RunResult },
    Error { message: String },
}
```

---

## 协作模式

### 1. Supervisor-Worker

```rust
/// Supervisor 配置
pub struct SupervisorConfig {
    /// Supervisor Agent
    supervisor: AgentConfig,

    /// Worker Agents
    workers: Vec<AgentConfig>,

    /// 任务分配策略
    strategy: DispatchStrategy,

    /// 最大并行数
    max_parallel: usize,
}

/// 任务分配策略
pub enum DispatchStrategy {
    /// 轮询
    RoundRobin,
    /// 最少任务优先
    LeastLoaded,
    /// 随机
    Random,
    /// 基于 Agent 能力匹配
    CapabilityMatch,
}

impl Supervisor {
    /// 分发任务给 workers
    pub async fn dispatch(&mut self, task: Task) -> Result<Vec<TaskResult>>;

    /// 广播给所有 workers
    pub async fn broadcast(&mut self, message: &str) -> Result<Vec<RunResult>>;

    /// 收集 worker 结果
    pub async fn collect(&mut self) -> Result<Vec<TaskResult>>;

    /// 监控 workers 状态
    pub fn workers_status(&self) -> HashMap<AgentId, AgentStatus>;
}
```

### 2. Pipeline

```rust
/// 流水线
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
    /// 创建流水线
    pub fn new() -> Self;

    /// 添加阶段
    pub fn stage(mut self, name: &str, agent: AgentConfig) -> Self;

    /// 执行流水线
    pub async fn run(&self, input: Value) -> Result<PipelineResult>;

    /// 流式执行
    pub fn run_stream(&self, input: Value) -> impl Stream<Item = PipelineEvent>;
}

// 使用示例
let pipeline = Pipeline::new()
    .stage("parse", AgentConfig::for_role("parser"))
    .stage("analyze", AgentConfig::for_role("analyzer"))
    .stage("generate", AgentConfig::for_role("generator"))
    .stage("review", AgentConfig::for_role("reviewer"));

let result = pipeline.run(input).await?;
```

### 3. Mesh（网状）

```rust
/// 网状拓扑
pub struct Mesh {
    agents: HashMap<AgentId, Agent>,
    topology: Topology,
    router: Router,
}

pub enum Topology {
    /// 全连接（每个 Agent 可以直接通信）
    FullMesh,
    /// 星形（中心路由）
    Star { center: AgentId },
    /// 环形
    Ring,
    /// 自定义邻接表
    Custom(HashMap<AgentId, Vec<AgentId>>),
}

impl Mesh {
    /// 创建网状
    pub fn new(topology: Topology) -> Self;

    /// 添加 Agent
    pub fn add(mut self, id: &str, agent: Agent) -> Self;

    /// 启动所有 Agent
    pub async fn start(&mut self) -> Result<()>;

    /// 发送消息
    pub async fn send(&mut self, from: AgentId, to: AgentId, msg: &str) -> Result<()>;

    /// 广播
    pub async fn broadcast(&mut self, from: AgentId, msg: &str) -> Result<()>;

    /// 运行直到所有 Agent 完成
    pub async fn run(&mut self, initial: Option<(AgentId, &str)>) -> Result<MeshResult>;
}
```

### 4. Race（竞争）

```rust
/// 竞争执行
pub struct Race {
    agents: Vec<Agent>,
    config: RaceConfig,
}

pub struct RaceConfig {
    /// 停止条件
    stop_on: StopCondition,

    /// 超时
    timeout: Duration,

    /// 是否返回所有结果（即使有赢家）
    return_all: bool,
}

pub enum StopCondition {
    /// 第一个成功
    FirstSuccess,
    /// 第一个完成（不管成功失败）
    FirstComplete,
    /// 所有完成
    AllComplete,
    /// 投票决定
    Vote { voters: Vec<AgentId>, threshold: f32 },
}

impl Race {
    /// 创建竞争
    pub fn new(agents: Vec<Agent>) -> Self;

    /// 运行竞争
    pub async fn run(&mut self, task: &str) -> Result<RaceResult>;
}

pub struct RaceResult {
    /// 赢家
    pub winner: Option<AgentId>,

    /// 赢家结果
    pub winner_result: Option<RunResult>,

    /// 所有结果
    pub all_results: HashMap<AgentId, Result<RunResult>>,

    /// 排名（按完成时间）
    pub ranking: Vec<(AgentId, Duration)>,
}
```

---

## 并发控制

### 锁管理

```rust
/// 锁管理器
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
    /// 共享锁（读）
    Shared,
    /// 排他锁（写）
    Exclusive,
}

impl LockManager {
    /// 尝试获取锁（非阻塞）
    pub fn try_acquire(
        &mut self,
        resource: &str,
        holder: AgentId,
        mode: LockMode
    ) -> Result<bool>;

    /// 获取锁（阻塞）
    pub async fn acquire(
        &mut self,
        resource: &str,
        holder: AgentId,
        mode: LockMode,
        timeout: Duration
    ) -> Result<()>;

    /// 释放锁
    pub fn release(&mut self, resource: &str, holder: AgentId) -> Result<()>;

    /// 查询锁状态
    pub fn status(&self, resource: &str) -> Option<LockStatus>;

    /// 强制释放（管理员）
    pub fn force_release(&mut self, resource: &str) -> Result<()>;
}
```

### 事务

```rust
/// 事务管理器
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
    /// 开始事务
    pub async fn begin(&mut self, initiator: AgentId) -> Result<TxId>;

    /// 事务内写入
    pub async fn write(&mut self, tx: TxId, key: &str, value: Value) -> Result<()>;

    /// 提交事务
    pub async fn commit(&mut self, tx: TxId) -> Result<()>;

    /// 回滚事务
    pub async fn rollback(&mut self, tx: TxId, reason: &str) -> Result<()>;

    /// 获取事务状态
    pub fn status(&self, tx: TxId) -> Option<TxState>;
}
```

---

## API 设计

### 核心 API

```rust
// === Workspace API ===

// 创建工作空间
let ws = Workspace::create("./my-project").await?;

// 注册 Agent
ws.register_agent("analyzer", AgentConfig {
    role: "code-analyzer".into(),
    system_prompt: "You analyze code...".into(),
    ..Default::default()
}).await?;

// 创建分支
ws.branch("feature-auth", from="main").await?;
ws.checkout("feature-auth").await?;

// 执行 Agent
let result = ws.run("analyzer", "Analyze the authentication module").await?;

// 查看事件
let events = ws.events(EventFilter::agent("analyzer")).await?;

// 提交
let commit = ws.commit("Completed auth analysis", author).await?;

// 合并回 main
ws.checkout("main").await?;
ws.merge("feature-auth", into="main", MergeStrategy::Auto).await?;
```

### TUI 集成

```rust
/// TUI App（V2）
pub struct App {
    /// 工作空间
    workspace: Arc<Workspace>,

    /// 当前活跃 Agent
    active_agent: Option<AgentId>,

    /// UI 组件
    components: Components,

    /// 事件订阅
    event_sub: mpsc::Receiver<Event>,
}

impl App {
    /// 启动 TUI
    pub async fn run(&mut self) -> Result<()>;

    /// 处理键盘事件
    async fn handle_key(&mut self, key: KeyEvent) -> Result<()>;

    /// 处理 Agent 事件
    async fn handle_agent_event(&mut self, event: AgentEvent) -> Result<()>;

    /// 渲染 UI
    fn render(&mut self, frame: &mut Frame);
}
```

---

## 目录结构（重构后）

```
src/
├── lib.rs                    # 公共导出
├── main.rs                   # CLI 入口
│
├── core/                     # 核心原语
│   ├── mod.rs
│   ├── workspace.rs          # Workspace
│   ├── branch.rs             # Branch
│   ├── commit.rs             # Commit
│   ├── event.rs              # Event, EventLog
│   ├── state.rs              # VersionedState
│   ├── snapshot.rs           # 快照/恢复
│   └── error.rs              # 错误类型
│
├── agent/                    # Agent 系统
│   ├── mod.rs
│   ├── agent.rs              # Agent, AgentConfig
│   ├── events.rs             # AgentEvent
│   ├── registry.rs           # Agent 注册表
│   ├── supervisor.rs         # Supervisor-Worker
│   ├── pipeline.rs           # Pipeline
│   ├── mesh.rs               # Mesh
│   └── race.rs               # Race
│
├── client/                   # LLM 客户端（保留）
│   ├── mod.rs
│   ├── trait.rs              # LLMClient trait
│   ├── anthropic.rs
│   └── openai.rs
│
├── tools/                    # 工具系统（保留）
│   ├── mod.rs
│   ├── trait.rs              # Tool trait
│   ├── registry.rs           # ToolRegistry
│   ├── bash.rs
│   ├── file.rs
│   └── schema.rs
│
├── concurrency/              # 并发控制
│   ├── mod.rs
│   ├── lock.rs               # LockManager
│   └── transaction.rs        # TransactionManager
│
├── api/                      # HTTP API（保留）
│   ├── mod.rs
│   ├── server.rs
│   ├── handlers/
│   └── types.rs
│
├── tui/                      # TUI（重构）
│   ├── mod.rs
│   ├── app.rs                # App state
│   ├── event.rs              # 事件处理
│   └── components/
│       ├── mod.rs
│       ├── messages.rs
│       ├── sidebar.rs
│       ├── branch.rs         # 新增：分支选择器
│       ├── agents.rs         # 新增：Agent 状态面板
│       └── ...
│
└── storage/                  # 持久化
    ├── mod.rs
    ├── file.rs               # 文件存储
    ├── sqlite.rs             # SQLite 存储（可选）
    └── format.rs             # 序列化格式
```

---

## 实施计划

### Phase 1: 核心原语（2 周）

**目标：** 实现 Workspace + Branch + Commit + EventLog

**交付物：**
- [ ] `core/workspace.rs` - Workspace 创建/加载
- [ ] `core/branch.rs` - 分支创建/切换/合并
- [ ] `core/commit.rs` - 提交创建/查询
- [ ] `core/event.rs` - 事件日志 append/query
- [ ] `core/state.rs` - 版本化 KV 存储
- [ ] `storage/` - 文件持久化

**测试：**
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

### Phase 2: Agent 系统（2 周）

**目标：** 重构 Agent，集成 Workspace

**交付物：**
- [ ] `agent/agent.rs` - Agent 重构，支持 Workspace
- [ ] `agent/events.rs` - AgentEvent 扩展
- [ ] `agent/registry.rs` - Agent 注册表
- [ ] 集成现有 `client/` 和 `tools/`

**API 变更：**
```rust
// V1
let agent = Agent::new(client, workdir, timeout, stream);
let result = agent.run(prompt, history).await?;

// V2
let agent = workspace.spawn_agent("analyzer", config).await?;
let result = agent.run("analyze this").await?;
```

### Phase 3: 协作模式（2 周）

**目标：** 实现 Supervisor/Pipeline/Mesh/Race

**交付物：**
- [ ] `agent/supervisor.rs` - Supervisor-Worker
- [ ] `agent/pipeline.rs` - Pipeline
- [ ] `agent/mesh.rs` - Mesh
- [ ] `agent/race.rs` - Race

### Phase 4: 并发控制（1 周）

**目标：** 锁和事务

**交付物：**
- [ ] `concurrency/lock.rs` - LockManager
- [ ] `concurrency/transaction.rs` - TransactionManager

### Phase 5: TUI 重构（2 周）

**目标：** 集成新架构到 TUI

**交付物：**
- [ ] 分支选择器组件
- [ ] Agent 状态面板
- [ ] 事件日志查看器
- [ ] 工具执行历史

### Phase 6: 文档与测试（1 周）

**目标：** 完善文档和测试覆盖率

**交付物：**
- [ ] API 文档
- [ ] 架构图更新
- [ ] 单元测试 > 80%
- [ ] 集成测试

---

## 迁移指南

### 从 V1 迁移到 V2

**V1 代码：**
```rust
let config = Config::load()?;
let client = AnthropicClient::new(config.api_key, None, config.model);
let agent = Agent::new(client, workdir.to_string(), 300, false);

let history = Arc::new(RwLock::new(vec![]));
let result = agent.run("prompt", history).await?;
```

**V2 等价代码：**
```rust
let ws = Workspace::create(workdir).await?;
let agent = ws.spawn_agent("default", AgentConfig {
    model: ModelConfig::anthropic(&api_key, &model),
    ..Default::default()
}).await?;

let result = agent.run("prompt").await?;
```

**自动迁移脚本：** TODO

---

## 风险与缓解

| 风险 | 缓解措施 |
|------|---------|
| 复杂度激增 | 渐进式迁移，保持 V1 API 兼容 |
| 性能下降 | 基准测试，优化热点 |
| 持久化开销 | 异步写入，批量提交 |
| 并发 Bug | 充分测试，形式化验证关键路径 |

---

## 参考

- Git 内部设计
- Kafka 事件日志
- Raft 共识算法（部分概念）
- Erlang/OTP Actor 模型

---

*El Psy Kongroo*
