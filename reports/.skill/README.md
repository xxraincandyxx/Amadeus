# reports/.skill — 报告写作技能库

为撰写比赛技术报告（`reports/tech-report`）与用户手册引入的写作类 Agent Skills 副本。
每个子目录是一个独立技能，入口均为其 `SKILL.md`，可按需加载给写作 Agent。

## 技能清单与选用场景

| 技能 | 用途 | 什么时候用 |
| --- | --- | --- |
| `scientific-writing` | 科学写作核心规范：成段散文（反对罗列要点）、IMRAD 结构、两阶段「先提纲后成文」流程 | 撰写 / 修改技术报告正文各章节、效果验证报告的对比实验叙述 |
| `research-paper-writing` | 学术写作质量打磨：Abstract / Introduction / Method / Experiments 的写法与审稿人视角 | 打磨需求分析、算法设计章节的论证逻辑与呈现 |
| `related-work-writing` | Related Work 对比写作：按主题组织、说明与已有工作的差异 | 撰写引言中的痛点分析、与现有记忆方案（MemGPT / LoCoMo 基线等）的对比 |
| `paper-visual-correctness-polish` | LaTeX 排版视觉检查与修复：编译错误、排版坏例、投稿前打磨 | 编译 `main.tex` 后检查表格溢出、图表间距、字体与引用渲染 |
| `en-to-zh-translator` | 英文文本翻译为中文 | 需要产出中英双语用户手册 / 交付文档时 |

## 当前写作任务建议

- **技术报告**：`scientific-writing`（章节成文）+ `research-paper-writing`（论证打磨）
  + `paper-visual-correctness-polish`（LaTeX 排版收尾）。
- **用户手册**：以操作步骤为主，可参考 `scientific-writing` 的段落规范保证说明完整；
  双语版本用 `en-to-zh-translator`。

技能来源：本机 `~/.agents/skills/`，为只读副本；更新时以源目录为准重新复制。
