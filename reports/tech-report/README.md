# XA-202612 技术方案报告（LaTeX）

「OS Agent 记忆优化及高效应用研究」（发榜单位：麒麟软件有限公司）的技术方案报告源文件。

## 编译

```bash
cd reports/tech-report
latexmk -xelatex main.tex        # 推荐
# 或
xelatex main.tex && xelatex main.tex   # 第二遍生成目录与交叉引用
```

产物为 `reports/tech-report/main.pdf`（编译中间产物已被 `.gitignore` 忽略）。

## 目录结构

```
reports/
  tech-report/        本报告源文件
    main.tex          文档骨架、宏定义、封面、目录
    sections/         各章节源文件（01–09）
  assets/             图表与截图资产（评测图表、案例截图等，见 assets/README.md）
```

正文引用资产时使用相对路径 `../assets/...`（`\includegraphics` 以 `main.tex`
所在目录为基准）。

## TODO 约定

所有**未实测 / 待完成**内容统一以红色标出，检索方式：

- 源文件中搜索 `\todo{...}`、`\todoblank`、`\statustodo`；
- 宏定义在 `main.tex`（`\newcommand{\todo}` 等），颜色 `todored`。

补齐实测数据后，将对应红色占位替换为数值/文字即可。
