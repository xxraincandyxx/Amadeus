# XA-202612 用户手册（LaTeX）

「OS Agent 记忆优化及高效应用研究」的用户手册源文件，面向终端用户与部署运维人员，
覆盖记忆模块的安装部署、配置、日常操作（偏好 / 知识 / 遗忘）与排障。

## 编译

```bash
cd reports/user-manual
latexmk -xelatex main.tex
# 或
xelatex main.tex && xelatex main.tex
```

产物为 `reports/user-manual/main.pdf`（中间产物已被 `.gitignore` 忽略）。

## 写作约定

- 遵循 `reports/.skill/scientific-writing` 的规范：以成段说明文字为主，表格仅用于
  可枚举事实（配置项、命令、错误码），不在正文中堆砌罗列要点；
- 所有**未落实 / 待截图 / 待实测**内容以红色 TODO 标出：搜索 `\todo{...}` 与
  `\todoblank`，宏定义在 `main.tex`；
- 示例中的配置键名与工具操作名均取自仓库实现（`crates/config`、
  `crates/core/src/tools/kylin_memory.rs`），手册内容与代码同步维护；
- 引用 `reports/assets/` 下的截图时使用相对路径 `../assets/...`。
