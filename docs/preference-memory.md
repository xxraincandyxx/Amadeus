# Preference Memory — 偏好记忆动态捕捉

> 对应赛题需求（2）：*构建偏好记忆动态捕捉模块，基于多源融合数据，实现用户操作习惯、输出风格、安全策略等偏好的自动提取与版本化管理，实现跨场景偏好适配与回溯。*

## 1. 设计目标

在 Amadeus/KylinMem 的 `crates/context` 记忆体系之上，新增一个**偏好记忆**模块，提供三大能力：

| 能力 | 说明 |
| --- | --- |
| 自动提取 | 从工具调用执行结果、用户行为数据、手动配置三路数据源自动提取偏好，无需 LLM 调用 |
| 版本化管理 | 每次偏好变更追加新版本而非覆盖，历史永不丢失，支持非破坏性回滚 |
| 跨场景回溯 | 偏好带场景标签，按场景相似度做适配复用，并支持"某时刻生效版本"的时间回溯 |

约束：**零新增第三方依赖**（仅 serde/serde_json/tracing，与 context crate 现状一致）、纯规则启发式、无网络与异步依赖 —— 契合端侧部署的轻量化要求。

## 2. 代码位置

```
crates/context/src/preference/
├── mod.rs          # 类型模型 + PreferenceStore + 版本化 + 跨场景解析 + JSON 持久化
├── extract.rs      # 多源自动提取规则（工具结果 / 用户行为 / 手动配置）
└── scenario.rs     # 场景标签 + 匹配与相似度打分
```

对外入口：`crate::preference::*`（`lib.rs` 通过 `pub mod preference;` 暴露）。

## 3. 核心数据模型

```rust
Preference {                 // 一条偏好的当前状态
    id: String,              // 稳定身份："{category}::{name}"，跨版本不变
    category: PreferenceCategory,  // tool_choice | output_style | safety_policy | workflow | habit | general
    name: String,
    value: String,           // 当前值
    confidence: f32,         // 置信度 [0, 0.95]
    source: PreferenceSource,// tool_result | user_behavior | manual_config
    scenario: ScenarioTag,   // 该偏好适用的场景（name + 键值属性）
    evidence: Vec<Evidence>, // 支撑证据（最多保留 50 条）
    created_at / updated_at / version
}

PreferenceVersion {          // 版本快照（不可变）
    version, value, confidence, source, scenario, timestamp, reason
}

ScenarioTag { name: String, attrs: BTreeMap<String, String> }  // "" 或 "*" 为通配
```

持久化格式：`.amadeus/preferences.json`（`PreferenceData { revision, preferences, tool_stats, habit_counts }`），每次变更全量落盘，`Mutex` 保护并发。

## 4. 自动提取规则（extract.rs）

### 4.1 工具调用执行结果 → 工具选择偏好（`ingest_tool_result`）

按 `(task, tool)` 维护成功/失败计数器，三类动作：

1. **创建**：某工具对某任务的样本数 ≥ 2、成功率 ≥ 0.8、使用占比 ≥ 0.6 时，生成 `tool_choice::{task}`，置信度按 `confidence_from_counts(success, fail)`（成功率 + 样本量饱和曲线，封顶 0.95）。
2. **强化/切换**：另一工具成功数明确超过当前偏好工具（且达标）时，追加新版本（`auto_switched`）；同一工具置信度提升时追加 `auto_reinforced`。
3. **降级**：偏好工具失败 ≥ 2 次且失败率 > 0.5 时，追加置信度 -0.15（下限 0.3）的新版本（`auto_downgraded`）；价值不变，历史保留。

### 4.2 用户行为数据 → 操作习惯 / 纠偏（`ingest_user_behavior`）

- **习惯**：同一场景下同一动作出现 ≥ 2 次 → `habit::{scenario}::{action}`，置信度随次数递增（0.6 → 0.75 → 0.85 → 0.92）。
- **纠偏**：`correction` 非空 → 创建/强化 `avoid::{action}`（安全敏感动作归入 `safety_policy`，如 sudo/delete/chmod 等），同时把冲突习惯的置信度 -0.2（`weakened_by_correction`）。

### 4.3 手动配置 → 显式偏好（`ingest_manual_config`）

`config::{key}`，置信度 0.98；按键名关键词推断类别（language/style/format → output_style；permission/approval/blocked → safety_policy；tool/editor/shell → tool_choice；workflow/flow/step → workflow）。值变化时追加新版本（`manual_config_updated`），相同值则为 no-op。

## 5. 版本化与回溯（mod.rs）

- `set_preference(input)`：显式写入。值/置信度/场景任一变化即追加版本；完全一致则 no-op。
- `rollback(id, version, reason)`：**非破坏性**——把目标版本的 value/confidence/scenario 作为新版本追加（reason 标记 `rollback_to_v{N}`），历史不重写。
- `versions(id)` / `trace`：完整版本谱系（旧 → 新，含每版 reason）。
- `delete(id)`：删除偏好及其历史（可用于"精准遗忘"需求）。

## 6. 跨场景适配与回溯（scenario.rs + mod.rs）

- **场景匹配** `scenario_matches(rule, runtime)`：通配符恒真；否则名字相同且 rule 的 attrs 全部被 runtime 满足。
- **相似度** `scenario_similarity(a, b)` ∈ [0,1]：同名基准 0.7 + attrs Jaccard 加权 0.3；异名仅 0.35 × attrs Jaccard（保证同名永远优先）。
- **适配** `resolve_for_scenario(scenario)`：精确匹配 + 通配偏好，按置信度/新鲜度排序。
- **候选** `cross_scenario_candidates(scenario, k)`：其他场景偏好按相似度降序，供跨场景复用。
- **时间回溯** `resolve_as_of(scenario, ts)`：每个偏好取 `timestamp ≤ ts` 的最新版本作为生效态。
- **注入桥** `to_memory_entries(scenario, with_candidates, max)`：把解析结果渲染成 `MemoryEntry`，可直接注册进现有 `MemoryRegistry` 注入 system prompt。

## 7. 与现有记忆体系的关系

- 复用 `crate::memory::MemoryEntry` 作为与 `MemoryRegistry` 的桥，**不改动** `memory.rs` / `memory_json.rs` / `MemoryTool`。
- 偏好模块自带 JSON 持久化，独立于 `JsonFileMemoryProvider` 的 `memory.json`，避免污染通用记忆。
- 与短期/中期记忆的流转：偏好属长期记忆，`resolve_*` 结果通过 `MemoryEntry` 注入当前会话（短期/中期）上下文，见第 6 节 `to_memory_entries`。

## 8. 测试与验证

- 单元测试内嵌于各文件（`cargo test -p context`），覆盖：阈值创建/强化/切换/降级、习惯与纠偏、配置类别推断、版本追加与 no-op、回滚、持久化往返、场景解析/时间回溯/跨场景候选排序、MemoryEntry 桥。
- 质量门禁：`cargo check -p context`、`cargo clippy -p context -- -D warnings`、`python scripts/check_source_headers.py`。

## 9. 使用示例

```rust
use amadeus_context::preference::{
    BehaviorRecord, ManualConfigRecord, PreferenceStore, ScenarioTag, ToolCallRecord,
};

let store = PreferenceStore::new(".amadeus/preferences.json");

// 1) 工具执行结果 → 工具选择偏好
store.ingest_tool_result(ToolCallRecord::new(
    "read_file", "file_ops", true, now_ms(), ScenarioTag::new("file_ops"),
))?;
store.ingest_tool_result(ToolCallRecord::new(
    "read_file", "file_ops", true, now_ms(), ScenarioTag::new("file_ops"),
))?;

// 2) 用户行为 → 习惯；纠偏 → avoid 偏好
store.ingest_user_behavior(BehaviorRecord {
    action: "run_tests".into(),
    scenario: ScenarioTag::new("dev"),
    timestamp: now_ms(),
    correction: None,
})?;

// 3) 手动配置 → 显式偏好
store.ingest_manual_config(ManualConfigRecord {
    key: "tui.language".into(),
    value: "zh-CN".into(),
    scenario: None,
    timestamp: now_ms(),
})?;

// 4) 跨场景解析 + 时间回溯 + 回滚
let prefs = store.resolve_for_scenario(&ScenarioTag::new("dev"));
let historical = store.resolve_as_of(&ScenarioTag::new("dev"), yesterday_ms);
store.rollback("config::tui.language", 1, "restore default")?;
```
