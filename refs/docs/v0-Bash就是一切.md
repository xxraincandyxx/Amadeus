# v0: Bash 就是一切

**终极简化：~50 行代码，1 个工具，完整的 Agent 能力。**

在构建 v1、v2、v3 之后，一个问题浮现：Agent 的*本质*到底是什么？

v0 通过反向思考来回答——剥离一切，直到只剩下核心。

## 核心洞察

Unix 哲学：一切皆文件，一切皆可管道。Bash 是这个世界的入口：

| 你需要 | Bash 命令 |
|--------|-----------|
| 读文件 | `cat`, `head`, `grep` |
| 写文件 | `echo '...' > file` |
| 搜索 | `find`, `grep`, `rg` |
| 执行 | `python`, `npm`, `make` |
| **子代理** | `python v0_bash_agent.py "task"` |

最后一行是关键洞察：**通过 bash 调用自身就实现了子代理**。不需要 Task 工具，不需要 Agent Registry——只需要递归。

## 完整代码

```python
#!/usr/bin/env python
from anthropic import Anthropic
import subprocess, sys, os

client = Anthropic(api_key="your-key", base_url="...")
TOOL = [{
    "name": "bash",
    "description": """执行 shell 命令。模式：
- 读取: cat/grep/find/ls
- 写入: echo '...' > file
- 子代理: python v0_bash_agent.py 'task description'""",
    "input_schema": {"type": "object", "properties": {"command": {"type": "string"}}, "required": ["command"]}
}]
SYSTEM = f"CLI agent at {os.getcwd()}. Use bash. Spawn subagent for complex tasks."

def chat(prompt, history=[]):
    history.append({"role": "user", "content": prompt})
    while True:
        r = client.messages.create(model="...", system=SYSTEM, messages=history, tools=TOOL, max_tokens=8000)
        history.append({"role": "assistant", "content": r.content})
        if r.stop_reason != "tool_use":
            return "".join(b.text for b in r.content if hasattr(b, "text"))
        results = []
        for b in r.content:
            if b.type == "tool_use":
                out = subprocess.run(b.input["command"], shell=True, capture_output=True, text=True, timeout=300)
                results.append({"type": "tool_result", "tool_use_id": b.id, "content": out.stdout + out.stderr})
        history.append({"role": "user", "content": results})

if __name__ == "__main__":
    if len(sys.argv) > 1:
        print(chat(sys.argv[1]))  # 子代理模式
    else:
        h = []
        while (q := input(">> ")) not in ("q", ""):
            print(chat(q, h))
```

这就是整个 Agent。~50 行。

## 子代理工作原理

```
主代理
  └─ bash: python v0_bash_agent.py "分析架构"
       └─ 子代理（独立进程，全新历史）
            ├─ bash: find . -name "*.py"
            ├─ bash: cat src/main.py
            └─ 通过 stdout 返回摘要
```

**进程隔离 = 上下文隔离**
- 子进程有自己的 `history=[]`
- 父进程捕获 stdout 作为工具结果
- 递归调用实现无限嵌套

## v0 牺牲了什么

| 特性 | v0 | v3 |
|------|----|----|
| 代理类型 | 无 | explore/code/plan |
| 工具过滤 | 无 | 白名单 |
| 进度显示 | 普通 stdout | 行内更新 |
| 代码复杂度 | ~50 行 | ~450 行 |

## v0 证明了什么

**复杂能力从简单规则中涌现：**

1. **一个工具足够** — Bash 是通往一切的入口
2. **递归 = 层级** — 自我调用实现子代理
3. **进程 = 隔离** — 操作系统提供上下文分离
4. **提示词 = 约束** — 指令塑造行为

核心模式从未改变：

```python
while True:
    response = model(messages, tools)
    if response.stop_reason != "tool_use":
        return response.text
    results = execute(response.tool_calls)
    messages.append(results)
```

其他一切——待办、子代理、权限——都是围绕这个循环的精化。

---

**Bash 就是一切。**

[← 返回 README](../README_zh.md)
