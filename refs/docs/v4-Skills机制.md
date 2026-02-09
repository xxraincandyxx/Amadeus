# v4: Skills 机制

**核心洞察：Skills 是知识包，不是工具。**

## 知识外化：从训练到编辑的范式转变

Skills 机制体现了一个深刻的范式转变：**知识外化 (Knowledge Externalization)**。

### 传统方式：知识内化于参数

传统 AI 系统的知识都藏在模型参数里。你没法访问、没法修改、没法复用。

想让模型学会新技能？你需要：
1. 收集大量训练数据
2. 设置分布式训练集群
3. 进行复杂的参数微调（LoRA、全量微调等）
4. 部署新模型版本

这就像大脑突然失忆，但你没有任何笔记可以恢复记忆。知识被锁死在神经网络的权重矩阵中，对用户完全不透明。

### 新范式：知识外化为文档

代码执行范式改变了这一切。

```
┌─────────────────────────────────────────────────────────────────┐
│                     知识存储层级                                  │
│                                                                  │
│  Model Parameters → Context Window → File System → Skill Library │
│       (内化)            (运行时)        (持久化)      (结构化)     │
│                                                                  │
│  ←───────── 训练修改 ──────────→  ←────── 自然语言修改 ─────────→  │
│     需要集群、数据、专业知识              任何人都可以编辑           │
└─────────────────────────────────────────────────────────────────┘
```

**关键突破**：
- **过去**：修改模型行为 = 修改参数 = 需要训练 = 需要 GPU 集群 + 训练数据 + 专业知识
- **现在**：修改模型行为 = 修改 SKILL.md = 编辑文本文件 = 任何人都可以做

这就像给 base model 外挂了一个可热插拔的 LoRA 权重，但你不需要对模型本身进行任何参数训练。

### 为什么这很重要

1. **民主化**：不再需要 ML 专业知识来定制模型行为
2. **透明性**：知识以人类可读的 Markdown 存储，可审计、可理解
3. **复用性**：一个 Skill 写一次，可以在任何兼容 Agent 上使用
4. **版本控制**：Git 管理知识变更，支持协作和回滚
5. **在线学习**：模型在更大的上下文窗口中"学习"，无需离线训练

传统的微调是**离线学习**：收集数据→训练→部署→使用。
Skills 是**在线学习**：运行时按需加载知识，立即生效。

### 知识层级对比

| 层级 | 修改方式 | 生效时间 | 持久性 | 成本 |
|------|----------|----------|--------|------|
| Model Parameters | 训练/微调 | 数小时-数天 | 永久 | $10K-$1M+ |
| Context Window | API 调用 | 即时 | 会话内 | ~$0.01/次 |
| File System | 编辑文件 | 下次加载 | 永久 | 免费 |
| **Skill Library** | **编辑 SKILL.md** | **下次触发** | **永久** | **免费** |

Skills 是最甜蜜的平衡点：持久化存储 + 按需加载 + 人类可编辑。

### 实际意义

假设你想让 Claude 学会你公司特有的代码规范：

**传统方式**：
```
1. 收集公司代码库作为训练数据
2. 准备微调脚本和基础设施
3. 运行 LoRA 微调（需要 GPU）
4. 部署自定义模型
5. 成本：$1000+ 和数周时间
```

**Skills 方式**：
```markdown
# skills/company-standards/SKILL.md
---
name: company-standards
description: 公司代码规范和最佳实践
---

## 命名规范
- 函数名使用小写+下划线
- 类名使用 PascalCase
...
```
```
成本：0，时间：5分钟
```

这就是知识外化的力量：**把需要训练才能编码的知识，变成任何人都能编辑的文档**。

## 问题背景

v3 给了我们子代理来分解任务。但还有一个更深的问题：**模型怎么知道如何处理特定领域的任务？**

- 处理 PDF？需要知道用 `pdftotext` 还是 `PyMuPDF`
- 构建 MCP 服务器？需要知道协议规范和最佳实践
- 代码审查？需要一套系统的检查清单

这些知识不是工具——是**专业技能**。Skills 通过让模型按需加载领域知识来解决这个问题。

## 核心概念

### 1. 工具 vs 技能

| 概念 | 是什么 | 例子 |
|------|--------|------|
| **Tool** | 模型能**做**什么 | bash, read_file, write_file |
| **Skill** | 模型**知道怎么做** | PDF 处理、MCP 构建 |

工具是能力，技能是知识。

### 2. 渐进式披露

```
Layer 1: 元数据 (始终加载)     ~100 tokens/skill
         └─ name + description

Layer 2: SKILL.md 主体 (触发时)   ~2000 tokens
         └─ 详细指南

Layer 3: 资源文件 (按需)        无限制
         └─ scripts/, references/, assets/
```

这让上下文保持轻量，同时允许任意深度的知识。

### 3. SKILL.md 标准

```
skills/
├── pdf/
│   └── SKILL.md          # 必需
├── mcp-builder/
│   ├── SKILL.md
│   └── references/       # 可选
└── code-review/
    ├── SKILL.md
    └── scripts/          # 可选
```

**SKILL.md 格式**：YAML 前置 + Markdown 正文

```markdown
---
name: pdf
description: 处理 PDF 文件。用于读取、创建或合并 PDF。
---

# PDF 处理技能

## 读取 PDF

使用 pdftotext 快速提取：
\`\`\`bash
pdftotext input.pdf -
\`\`\`
...
```

## 实现（约 100 行新增）

### SkillLoader 类

```python
class SkillLoader:
    def __init__(self, skills_dir: Path):
        self.skills = {}
        self.load_skills()

    def parse_skill_md(self, path: Path) -> dict:
        """解析 YAML 前置 + Markdown 正文"""
        content = path.read_text()
        match = re.match(r'^---\s*\n(.*?)\n---\s*\n(.*)$', content, re.DOTALL)
        # 返回 {name, description, body, path, dir}

    def get_descriptions(self) -> str:
        """生成系统提示词的元数据"""
        return "\n".join(f"- {name}: {skill['description']}"
                        for name, skill in self.skills.items())

    def get_skill_content(self, name: str) -> str:
        """获取完整内容用于上下文注入"""
        return f"# Skill: {name}\n\n{skill['body']}"
```

### Skill 工具

```python
SKILL_TOOL = {
    "name": "Skill",
    "description": "加载技能获取专业知识。",
    "input_schema": {
        "properties": {"skill": {"type": "string"}},
        "required": ["skill"]
    }
}
```

### 消息注入（保持缓存）

关键洞察：Skill 内容进入 **tool_result**（user message 的一部分），而不是 system prompt：

```python
def run_skill(skill_name: str) -> str:
    content = SKILLS.get_skill_content(skill_name)
    # 完整内容作为 tool_result 返回
    # 成为对话历史的一部分（user message）
    return f"""<skill-loaded name="{skill_name}">
{content}
</skill-loaded>

Follow the instructions in the skill above."""

def agent_loop(messages: list) -> list:
    while True:
        response = client.messages.create(
            model=MODEL,
            system=SYSTEM,  # 永不改变 - 缓存保持有效！
            messages=messages,
            tools=ALL_TOOLS,
        )
        # Skill 内容作为 tool_result 进入 messages...
```

**关键洞察**：
- Skill 内容作为新消息**追加到末尾**
- 之前的所有内容（system prompt + 历史消息）都被缓存复用
- 只有新追加的 skill 内容需要计算，**整个前缀都命中缓存**

## 与生产版本对比

| 机制 | Claude Code / Kode | v4 |
|------|-------------------|-----|
| 格式 | SKILL.md (YAML + MD) | 相同 |
| 加载 | Container API | SkillLoader 类 |
| 触发 | 自动 + Skill 工具 | 仅 Skill 工具 |
| 注入 | newMessages (user message) | tool_result (user message) |
| 缓存机制 | 追加到末尾，前缀全部缓存 | 追加到末尾，前缀全部缓存 |
| 版本控制 | Skill Versions API | 省略 |
| 权限 | allowed-tools 字段 | 省略 |

**关键共同点**：两者都将 skill 内容注入对话历史（而非 system prompt），保持 prompt cache 有效。

## 为什么这很重要：缓存与成本

### 自回归模型与 KV Cache

大模型是自回归的：生成每个 token 都要 attend 之前所有 token。为避免重复计算，提供商实现了 **KV Cache**：

```
请求 1: [System, User1, Asst1, User2]
        ←────── 全部计算 ──────→

请求 2: [System, User1, Asst1, User2, Asst2, User3]
        ←────── 缓存命中 ──────→ ←─ 新计算 ─→
               (更便宜)            (正常价格)
```

缓存命中要求**前缀完全相同**。

### 需要注意的模式

| 操作 | 影响 | 结果 |
|------|------|------|
| 编辑历史 | 改变前缀 | 缓存无法复用 |
| 中间插入 | 后续前缀变化 | 需要重新计算 |
| 修改 system prompt | 最前面变化 | 整个前缀需重新计算 |

### 推荐：只追加

```python
# 避免: 编辑历史
messages[2]["content"] = "edited"  # 缓存失效

# 推荐: 只追加
messages.append(new_msg)  # 前缀不变，缓存命中
```

### 长上下文支持

主流模型支持较大的上下文窗口：
- Claude Sonnet 4.5 / Opus 4.5: **200K**
- GPT-5.2: **256K+**
- Gemini 3 Flash/Pro: **1M**

200K tokens 约等于 15 万词，一本 500 页的书。对于大多数 Agent 任务，现有上下文窗口已经足够。

> **把上下文当作只追加日志，而非可编辑文档。**

## 设计哲学：知识外化的实践

> **知识作为一等公民**

回到开篇讨论的知识外化范式。传统观点：AI Agent 是"工具调用器"——模型决定用什么工具，代码执行工具。

但这忽略了一个关键维度：**模型怎么知道应该怎么做？**

Skills 是知识外化的完整实践：

**过去（知识内化）**：
- 知识锁在模型参数里
- 修改需要训练（LoRA、全量微调）
- 用户无法访问或理解
- 成本：$10K-$1M+，周期：数周

**现在（知识外化）**：
- 知识存在 SKILL.md 文件中
- 修改就是编辑文本
- 人类可读、可审计
- 成本：免费，周期：即时生效

Skills 承认：**领域知识本身就是一种资源**，需要被显式管理。

1. **分离元数据与内容**：description 是索引，body 是内容
2. **按需加载**：上下文窗口是宝贵的认知资源
3. **标准化格式**：写一次，在任何兼容的 Agent 上使用
4. **注入而非返回**：Skills 改变认知，不只是提供数据
5. **在线学习**：在更大的上下文窗口中即时"学习"，无需离线训练

知识外化的本质是**把隐式知识变成显式文档**：
- 开发者用自然语言"教"模型新技能
- Git 管理和共享知识
- 版本控制、审计、回滚

**这是从"训练 AI"到"教育 AI"的范式转变。**

## 系列总结

| 版本 | 主题 | 新增行数 | 核心洞察 |
|------|------|----------|----------|
| v1 | Model as Agent | ~200 | 模型是 80%，代码只是循环 |
| v2 | 结构化规划 | ~100 | Todo 让计划可见 |
| v3 | 分而治之 | ~150 | 子代理隔离上下文 |
| **v4** | **领域专家** | **~100** | **Skills 注入专业知识** |

---

**工具让模型能做事，技能让模型知道怎么做。**
