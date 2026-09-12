# 报告资产目录

本目录存放报告引用的图片资产（供 `reports/tech-report/main.tex` 使用，引用相对路径
`../assets/...`）。当前正文的架构图 / 流程图均为 TikZ 矢量绘制（随 LaTeX 源编译，
无需位图）。以下资产待评测与案例工作完成后补入，并在 `tech-report/sections/` 中以
`\includegraphics` 引用：

| 计划资产 | 用途 | 引用位置 | 状态 |
| --- | --- | --- | --- |
| `metrics-preference.png` | 偏好提取准确率对比图（本方案 vs 消融基线） | `tech-report/sections/07-evaluation.tex` | 待生成 |
| `metrics-recall.png` | 知识检索 Recall@k 曲线 | `tech-report/sections/07-evaluation.tex` | 待生成 |
| `metrics-latency.png` | 检索延迟分布（P50/P95，不同库规模） | `tech-report/sections/07-evaluation.tex` | 待生成 |
| `metrics-conflict.png` | 冲突处理正确率对比图 | `tech-report/sections/07-evaluation.tex` | 待生成 |
| `case-workflow.png` | 真实场景案例执行记录截图 | `tech-report/sections/08-case-study.tex` | 待生成 |
| `kylin-adaptation.png` | 银河麒麟环境适配测试截图 | `tech-report/sections/06-deployment.tex` | 待生成 |

图表生成脚本建议放在仓库 `scripts/`（Python/matplotlib），本目录只存产物。
