# TUI 设计对比：Claude Code 与 Amadeus（基于 tmux-cli 实测）

> 英文版：[TUI_DESIGN_CLAUDE_CODE_VS_AMADEUS_EN.md](./TUI_DESIGN_CLAUDE_CODE_VS_AMADEUS_EN.md)

本文档记录在本地通过 **tmux-cli**（`claude-code-tools`）隔离会话、对 **Claude Code**（`claude` CLI）与 **Amadeus**（`cargo run --features full` / `target/debug/amadeus`）终端界面进行对照观察后的结论。内容覆盖 **欢迎 Dashboard** 与 **运行中动效与布局**（自动补全、上下文压缩、工具执行监视、流式输出、状态条等），不仅限于首屏。流程参考仓库内 `skills/tui-tmux-debugging.md`；**分步骤对照流程**见项目技能 [`.cursor/skills/tui-comparison-tmux/SKILL.md`](../.cursor/skills/tui-comparison-tmux/SKILL.md)。

**范围声明（重要）**：本文仅做产品/设计层面的观察与归纳，**不要求、也不建议**据此向编码代理下达「修改 Amadeus 代码」类指令；若后续要落地改动，应由人类评审后再单独立项。

---

## 1. 测试方法摘要

1. **环境**：`tmux-cli` 处于 *Remote* 模式时，会在会话 `remote-cli-session` 下管理窗口；先 `tmux-cli launch "zsh"`（或 `bash`），再 `tmux-cli send "<命令>" --pane=<窗格>`，避免进程直接退出导致无输出。
2. **观测**：`tmux capture-pane -t remote-cli-session:<窗口索引> -p` 抓取纯文本布局（ANSI 颜色在部分环境下可能简化）；交互前可用 `tmux-cli wait_idle` 等待界面稳定。
3. **注意**：Amadeus 在 **无有效 API 环境变量** 时会立即报错退出；需在具备 `.env` 或已导出密钥的目录启动才能看到完整 TUI（含 Dashboard）。Claude Code 侧依赖其自身登录/计费配置，与本仓库无关。

---

## 2. Claude Code TUI：观感特征

在 80×24 典型 tmux 窗格中，**启动态（空会话）**大致呈现：

- **品牌区**：顶部为块状 ASCII/Unicode 徽标（如 `▐▛███▜▌` 一类几何图形），右侧或邻行标注 **Claude Code 版本**、**当前模型/计费方式**、**工作目录**一行摘要。
- **次要提示**：一行简短能力提示（例如切换模型的命令提示 `/model ...`）。
- **分隔**：整宽 `─` 水平线，将「信息区」与「输入引导」分开。
- **主行动召唤**：中间一行高可见的 **示例提示**（如 `Try "edit app.rs to..."`），前缀 `❯`，直接教用户第一句话说什么。
- **底部留白**：大量空行，整体 **密度低、焦点集中**；再一行 `? for shortcuts` 作为全局帮助入口。

**设计关键词**：**极简、单栏叙事、强引导、少控件暴露**。信息层级是「品牌 → 环境 → 一条线 → 一句话任务建议 → 帮助」，没有独立「仪表盘」区块，侧栏/多面板在启动屏上不可见。

---

## 3. Amadeus TUI：观感特征（暗红主题 + Dashboard）

在相同窗格宽度下，**空历史 + 成功启动**时可见：

### 3.1 专用 Dashboard（与 Claude Code 最大差异）

- **标题条**：`Amadeus v0.1.0` + 主题色强调 + 横向 `─` 填充至行宽（见 `MessagesComponent::render_dashboard_lines`）。
- **欢迎语**：已移除；标题行后直接进入吉祥物区。
- **吉祥物/品牌图形**：根据终端宽度在 **大幅 Braille/块字符图案**（`FULL_ART` / `FACE_ART`）与省略之间切换，视觉存在感明显高于 Claude Code 的小徽标。
- **产品定位文案**：居中一行 **「amadeus ◈ Premium CLI Coding Interface」** 及 **path / 项目名** 提示。
- **Tips 区**：带小标题 `Tips for getting started` + 列表项（如 `/help`、`Esc` 模式切换），底部再一条整宽分隔线。

语义色由主题系统提供；**Dark Red** 主题为偏暖深底（如近黑微红）、**血红色系**强调与链接色、灰阶带红棕倾向（见 `src/ui/themes/dark_red.rs` 中 `SemanticColors` 配置）。

### 3.2 会话与输入区

- 有对话后，主区域出现 **turn 分隔**、消息区，以及底部 **输入区**：灰横线、`❯ Try "…"`（示例来自 `AMADEUS_TRY_PROMPT`）、右侧 **字符/行数统计**、可选 **状态提示行**，再为无边框编辑区与占位符 `Type a message...`。
- 与 Claude Code「单条 Try 引导」相比，Amadeus **更偏「IDE 式会话流 + 显式 turn 标记」**。

### 3.3 双行状态栏（Footer）

捕获文本中可见两行摘要，例如：

- **上行**：会话/代理名、`◈` 模型名、上下文占用条 `[░░░░░░░░]`、百分比、`◷` 会话时长等。
- **下行**：`root>` 类提示、`📂` 路径、`⎇` 分支、`◫` sandbox 状态等。

Claude Code 启动屏 **不在同一位置堆叠** 这种「监控型」双行条；其信息更收敛在徽标旁的一两行说明里。

### 3.4 运行时动效与布局（Live 区、输入区、状态条）

以下对应 `Session::render` / `render_live_viewport` 等与 **非欢迎页** 强相关的表现（实现分散在 `src/ui/app.rs`、`src/ui/components/*`）。

**主栏垂直分区（自上而下）**：Live 视口 → 多行输入（含顶部边框标题）→ 可选 **单行 StatusBar**（有活跃请求时）→ **双行 Footer**。侧栏（Context / Files / Help 等）打开时从右侧再切一块宽度。

**Live 视口优先级**（互斥展示，按代码判定顺序）：在「无流式正文、无压缩进行中、无运行中工具、无 stream_rx」且消息为空时显示 **Dashboard**；否则带 **聚焦边框** 的块内依次为：

1. **工具活动优先**：有运行中工具且无流式正文、无压缩排队时，块标题为 **「 Monitor 」**，内容为工具摘要（工具名 + `LoadingIndicator` 的 scramble/省略号提示 + 可选进度），以及 `ctrl+x then i/k/j/l` 类导航提示（见 `render_tool_activity_preview`）。
2. **上下文压缩**：**CompactionAnimator** 在 Live 块内显示 **单行**状态（`Compacting context` + 轻量点号 + 百分比 + 耗时）；结束后短暂展示结果再并入历史。
3. **仅有流式/思考无正文**：`LoadingIndicator::prompt_hint()` 的 scramble + **`.` / `..` / `...`**；审批等待为 **「awaiting approval」**，显示在灰线下方 **独立提示行**。
4. **有流式 Markdown 正文**：块标题为 **「 Live 」**，内区展示 `streaming_buffer` 渲染结果。

**输入区动效与自动补全**：`/` 开头的 slash 命令会弹出 **「 Commands 」** 列表（最多 6 条，`completion.rs`），位于输入区域 **下方**，带边框；**Tab** 采纳、**Shift+Tab** / **Ctrl+方向** 在列表中移动（见 `app.rs` 快捷键处理）。选中行为 **LightCyan** 等与主题并存的 ratatui 颜色。

**StatusBar**（`status_bar.rs`）：在模型请求活跃时出现 **第三行**状态：`thinking` / `generating`、可选 **tok/s**、输入/输出 token 估算（▲/▼）及思考时的 **⟡** 符号——与 Claude Code 运行时状态相比，Amadeus 更偏 **可量化指标行**。

**对比 Claude Code（运行时，观测性描述）**：Claude Code 在工具与回复进行中的 UI 通常更 **内敛**（状态合并进主叙事区、较少独立「监视器」块与双行仪表）；Amadeus 则显式区分 **Monitor / Live / 压缩动画** 与 **Footer 上下文条**，动效层（spinner、scramble、进度条、补全弹层）更多、布局 **更 IDE 化**。若追求「Claude 式简洁」，对比时应 **单独截运行时多帧**，而非只看 Dashboard。

**tmux 捕获说明**：`capture-pane` 是 **单帧**；spinner/scramble 需 **连续多次 capture** 或 **录屏 attach** 才能评价动效；静态对比可依赖 `wait_idle` 后的稳定帧。

---

## 4. 并排对比（设计维度）

| 维度 | Claude Code | Amadeus |
|------|----------------|---------|
| **首屏信息密度** | 低，留白多 | 中高，Dashboard + 多段文案 + 图 |
| **品牌表达** | 小徽标 + 版本/模型一行 | 大吉祥物 + 副标题 + path |
| **用户引导** | 单一「Try …」示例句 | Tips 列表 + `/help` / `Esc` |
| **会话结构** | 启动时几乎不展示 turn | 明确 `turn N` 与消息区 |
| **状态/环境** | 缩在标题区附近 | 独立双行 Footer，偏「仪表盘」 |
| **主题** | 默认偏中性暗色（随终端） | 可切换主题，**Dark Red** 为差异化暗红美学 |
| **侧栏/多面板** | 启动屏不强调 | 代码侧有 Context/File/Help 等侧栏能力（启动后随模式展开） |
| **运行时 Live 区** | 相对合一的会话视图 | Monitor / Live / 压缩动画分状态，带聚焦边框 |
| **加载与工具反馈** | 偏简洁状态文案 | scramble 标签、点号动画、压缩条、工具监视器、可选 StatusBar |
| **命令补全** | 依产品版本而异（需实测） | `/` 触发弹层列表 + 键盘导航 |

---

## 5. 目标取向（仅设计意图，非实现任务）

若产品目标是 **「接近 Claude Code 的简洁、现代感」**，同时 **保留 Amadeus 的暗红主题与专用 Dashboard**，可从**体验层面**理解下列张力与平衡点（**不绑定任何代码修改**）：

1. **简洁**：Claude Code 的「现代」很大程度上来自 **少元素、强一条 CTA、大留白**。Amadeus Dashboard 信息更全，更易「第一眼厚重」——若未来要趋近 Claude Code，需在 **Dashboard 信息架构**上做取舍（例如折叠 Tips、缩小吉祥物触发宽度），而非简单换色。
2. **现代**：Claude Code 使用 **几何徽标 + 全宽分隔线 + 单行命令提示**，形成统一的「卡片式」顶区。Amadeus 已有顶部分隔与标题条，可继续强化 **单一视觉焦点**（例如欢迎语 + 一条 CTA 并列，其余降级为 `/help`）。
3. **暗红主题**：当前 `Dark Red` 已通过语义色区分正文、次要、链接、边框与状态，**与「简洁」不冲突**——简洁是布局与层级问题，红色是调色板；保持 **低饱和背景 + 高饱和点缀** 即可同时显得现代且不刺眼。
4. **专用 Dashboard**：这是 Amadeus 的 **明确差异化**（Claude Code 没有等价首屏）。保留 Dashboard 的前提下，可向 Claude Code 借鉴的是 **首屏只保留 3～5 个最高优先级信息**，其余交给会话内或 `/help`。
5. **运行时动效**：若借鉴 Claude 的「轻」，需审视 **Monitor 与压缩块** 是否在同一时刻堆叠过多轨道，以及 **scramble + 点号 + StatusBar + Footer 条** 是否信息重复；动效本身可保留，但 **层级**可收紧。

---

## 6. tmux-cli 实测片段（文本快照）

以下为同一机器、tmux 文本捕获的节选，仅用于说明布局差异（非完整 UI）。

**Claude Code（启动屏节选）**：

```text
 ▐▛███▜▌   Claude Code v2.1.69
▝▜█████▛▘  <模型> · <计费说明>
  ▘▘ ▝▝    ~/Dev/amadeus

  /model to try Opus 4.6

────────────────────────────────────────────────────────────────────────────────
❯ Try "edit app.rs to..."
────────────────────────────────────────────────────────────────────────────────
  ? for shortcuts
```

**Amadeus（Dashboard + 输入/页脚节选）**：

```text
 Amadeus v0.1.0 ───────────────────────────────────────────────────────────────

                                  <吉祥物 Braille 图案多行>

  ...（Tips：/help、Esc 等，视窗格高度可能需滚动查看）...

────────────────────────────────────────────────────────────────────────────────
❯ Try "how does src/main.rs work?"                    0 ch · 1 line
────────────────────────────────────────────────────────────────────────────────
  Type a message... (Enter: send, Alt+Enter: newline)

main  │ ◈ <模型> [░░░░░░░░] 0% │ ◷ 00:03
root> │📂 ~/Dev/amadeus ⎇ dev │ ◫ no sandbox
```

**运行时（示意）**：提示行可为 scramble + 点号或 `awaiting approval`；压缩可为单行如 `  Compacting context.  ·  45%  ·  3s`。

---

## 7. 清理建议

按 `skills/tui-tmux-debugging.md`，实验结束后可：

```bash
tmux-cli kill --pane=remote-cli-session:<窗口>
# 或
tmux-cli cleanup
```

避免长时间占用 `remote-cli-session` 或遗留 `amadeus` 进程。

---

## 8. 文档维护

- **对比基准**：Claude Code `claude --version` 实测为 **2.1.69**（随安装变化）。
- **Amadeus 版本**：Dashboard 文案硬编码为 **v0.1.0**（以 `messages.rs` 为准，发布时需人工对齐）。
- **对照操作清单**：见 [`.cursor/skills/tui-comparison-tmux/SKILL.md`](../.cursor/skills/tui-comparison-tmux/SKILL.md)。
- 若仅更新本文档，无需改动应用代码。
