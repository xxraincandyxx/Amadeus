# v0: Bashがすべて

**究極の簡素化：約50行、1ツール、完全なエージェント機能。**

v1、v2、v3を構築した後、ある疑問が浮かびます：エージェントの*本質*とは何か？

v0は逆方向に進むことでこれに答えます—コアだけが残るまですべてを削ぎ落とします。

## コアの洞察

Unix哲学：すべてはファイル、すべてはパイプできる。Bashはこの世界への入り口です：

| 必要なこと | Bashコマンド |
|----------|--------------|
| ファイルを読む | `cat`, `head`, `grep` |
| ファイルに書く | `echo '...' > file` |
| 検索 | `find`, `grep`, `rg` |
| 実行 | `python`, `npm`, `make` |
| **サブエージェント** | `python v0_bash_agent.py "task"` |

最後の行が重要な洞察です：**bash経由で自分自身を呼び出すことでサブエージェントを実装**。Taskツールも、Agent Registryも不要—ただの再帰です。

## 完全なコード

```python
#!/usr/bin/env python
from anthropic import Anthropic
import subprocess, sys, os

client = Anthropic(api_key="your-key", base_url="...")
TOOL = [{
    "name": "bash",
    "description": """Execute shell command. Patterns:
- Read: cat/grep/find/ls
- Write: echo '...' > file
- Subagent: python v0_bash_agent.py 'task description'""",
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
        print(chat(sys.argv[1]))  # Subagent mode
    else:
        h = []
        while (q := input(">> ")) not in ("q", ""):
            print(chat(q, h))
```

これが完全なエージェントです。約50行。

## サブエージェントの仕組み

```
メインエージェント
  └─ bash: python v0_bash_agent.py "analyze architecture"
       └─ サブエージェント（分離されたプロセス、新しい履歴）
            ├─ bash: find . -name "*.py"
            ├─ bash: cat src/main.py
            └─ stdoutで要約を返す
```

**プロセス分離 = コンテキスト分離**
- 子プロセスは独自の `history=[]` を持つ
- 親はstdoutをツール結果としてキャプチャ
- 再帰呼び出しで無制限のネストが可能

## v0が犠牲にするもの

| 機能 | v0 | v3 |
|------|----|-----|
| エージェントタイプ | なし | explore/code/plan |
| ツールフィルタリング | なし | ホワイトリスト |
| 進捗表示 | 通常のstdout | インライン更新 |
| コードの複雑さ | 約50行 | 約450行 |

## v0が証明すること

**複雑な能力はシンプルなルールから生まれる：**

1. **1つのツールで十分** — Bashはすべてへの入り口
2. **再帰 = 階層** — 自己呼び出しでサブエージェントを実装
3. **プロセス = 分離** — OSがコンテキスト分離を提供
4. **プロンプト = 制約** — 指示が振る舞いを形作る

コアパターンは決して変わらない：

```python
while True:
    response = model(messages, tools)
    if response.stop_reason != "tool_use":
        return response.text
    results = execute(response.tool_calls)
    messages.append(results)
```

他のすべて—Todo、サブエージェント、権限—はこのループの周りの改良です。

---

**Bashがすべて。**

[← READMEに戻る](../README_ja.md) | [v1 →](./v1-モデルがエージェント.md)
