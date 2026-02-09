# Learn Claude Code - Bashがあれば、エージェントは作れる

[![Python 3.10+](https://img.shields.io/badge/python-3.10+-blue.svg)](https://www.python.org/downloads/)
[![Tests](https://github.com/shareAI-lab/learn-claude-code/actions/workflows/test.yml/badge.svg)](https://github.com/shareAI-lab/learn-claude-code/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](./LICENSE)

> **免責事項**: これは [shareAI Lab](https://github.com/shareAI-lab) による独立した教育プロジェクトです。Anthropic社とは無関係であり、同社からの承認やスポンサーを受けていません。「Claude Code」はAnthropic社の商標です。

**ゼロからAIエージェントの仕組みを学ぶ。**

[English](./README.md) | [中文](./README_zh.md)

---

## なぜこのリポジトリを作ったのか？

このリポジトリは、Claude Code への敬意から生まれました。私たちは **Claude Code を世界最高のAIコーディングエージェント** だと考えています。当初、行動観察と推測によってその設計をリバースエンジニアリングしようとしました。しかし、公開した分析には不正確な情報、根拠のない推測、技術的な誤りが含まれていました。Claude Code チームと、誤った情報を信じてしまった方々に深くお詫び申し上げます。

過去6ヶ月間、実際のエージェントシステムを構築し反復する中で、**「真のAIエージェントとは何か」** についての理解が根本的に変わりました。その知見を皆さんと共有したいと思います。以前の推測的なコンテンツはすべて削除し、オリジナルの教材に置き換えました。

---

> **[Kode CLI](https://github.com/shareAI-lab/Kode)**、**Claude Code**、**Cursor**、および [Agent Skills Spec](https://agentskills.io/specification) をサポートするすべてのエージェントで動作します。

<img height="400" alt="demo" src="https://github.com/user-attachments/assets/0e1e31f8-064f-4908-92ce-121e2eb8d453" />

## 学べること

このチュートリアルを完了すると、以下を理解できます：

- **エージェントループ** - すべてのAIコーディングエージェントの背後にある驚くほどシンプルなパターン
- **ツール設計** - AIモデルに現実世界と対話する能力を与える方法
- **明示的な計画** - 制約を使ってAIの動作を予測可能にする
- **コンテキスト管理** - サブエージェントの分離によりエージェントのメモリをクリーンに保つ
- **知識注入** - 再学習なしでドメイン専門知識をオンデマンドで読み込む

## 学習パス

```
ここから始める
    |
    v
[v0: Bash Agent] -----> 「1つのツールで十分」
    |                    16-50行
    v
[v1: Basic Agent] ----> 「完全なエージェントパターン」
    |                    4ツール、約200行
    v
[v2: Todo Agent] -----> 「計画を明示化する」
    |                    +TodoManager、約300行
    v
[v3: Subagent] -------> 「分割統治」
    |                    +Taskツール、約450行
    v
[v4: Skills Agent] ---> 「オンデマンドのドメイン専門性」
                         +Skillツール、約550行
```

**おすすめの学習方法：**
1. まずv0を読んで実行 - コアループを理解する
2. v0とv1を比較 - ツールがどう進化するか見る
3. v2で計画パターンを学ぶ
4. v3で複雑なタスク分解を探求する
5. v4で拡張可能なエージェント構築をマスターする

## クイックスタート

```bash
# リポジトリをクローン
git clone https://github.com/shareAI-lab/learn-claude-code
cd learn-claude-code

# 依存関係をインストール
pip install -r requirements.txt

# API キーを設定
cp .env.example .env
# .env を編集して ANTHROPIC_API_KEY を入力

# 任意のバージョンを実行
python v0_bash_agent.py      # 最小限（ここから始めよう！）
python v1_basic_agent.py     # コアエージェントループ
python v2_todo_agent.py      # + Todo計画
python v3_subagent.py        # + サブエージェント
python v4_skills_agent.py    # + Skills
```

## コアパターン

すべてのコーディングエージェントは、このループにすぎない：

```python
while True:
    response = model(messages, tools)
    if response.stop_reason != "tool_use":
        return response.text
    results = execute(response.tool_calls)
    messages.append(results)
```

これだけです。モデルは完了するまでツールを呼び出し続けます。他のすべては改良にすぎません。

## バージョン比較

| バージョン | 行数 | ツール | コア追加 | 重要な洞察 |
|------------|------|--------|----------|------------|
| [v0](./v0_bash_agent.py) | ~50 | bash | 再帰的サブエージェント | 1つのツールで十分 |
| [v1](./v1_basic_agent.py) | ~200 | bash, read, write, edit | コアループ | モデルがエージェント |
| [v2](./v2_todo_agent.py) | ~300 | +TodoWrite | 明示的計画 | 制約が複雑さを可能にする |
| [v3](./v3_subagent.py) | ~450 | +Task | コンテキスト分離 | クリーンなコンテキスト = より良い結果 |
| [v4](./v4_skills_agent.py) | ~550 | +Skill | 知識読み込み | 再学習なしの専門性 |

## ファイル構造

```
learn-claude-code/
├── v0_bash_agent.py       # ~50行: 1ツール、再帰的サブエージェント
├── v0_bash_agent_mini.py  # ~16行: 極限圧縮
├── v1_basic_agent.py      # ~200行: 4ツール、コアループ
├── v2_todo_agent.py       # ~300行: + TodoManager
├── v3_subagent.py         # ~450行: + Taskツール、エージェントレジストリ
├── v4_skills_agent.py     # ~550行: + Skillツール、SkillLoader
├── skills/                # サンプルSkills（pdf, code-review, mcp-builder, agent-builder）
├── docs/                  # 技術ドキュメント（EN + ZH + JA）
├── articles/              # ブログ形式の記事（ZH）
└── tests/                 # ユニットテストと統合テスト
```

## ドキュメント

### 技術チュートリアル (docs/)

- [v0: Bashがすべて](./docs/v0-Bashがすべて.md)
- [v1: モデルがエージェント](./docs/v1-モデルがエージェント.md)
- [v2: 構造化プランニング](./docs/v2-構造化プランニング.md)
- [v3: サブエージェント機構](./docs/v3-サブエージェント.md)
- [v4: スキル機構](./docs/v4-スキル機構.md)

### 記事

[articles/](./articles/) でブログ形式の解説を参照してください（中国語）。

## Skillsシステムの使用

### 含まれているサンプルSkills

| Skill | 用途 |
|-------|------|
| [agent-builder](./skills/agent-builder/) | メタスキル：エージェントの作り方 |
| [code-review](./skills/code-review/) | 体系的なコードレビュー手法 |
| [pdf](./skills/pdf/) | PDF操作パターン |
| [mcp-builder](./skills/mcp-builder/) | MCPサーバー開発 |

### 新しいエージェントのスキャフォールド

```bash
# agent-builder skillを使って新しいプロジェクトを作成
python skills/agent-builder/scripts/init_agent.py my-agent

# 複雑さのレベルを指定
python skills/agent-builder/scripts/init_agent.py my-agent --level 0  # 最小限
python skills/agent-builder/scripts/init_agent.py my-agent --level 1  # 4ツール
```

### 本番環境用Skillsのインストール

```bash
# Kode CLI（推奨）
kode plugins install https://github.com/shareAI-lab/shareAI-skills

# Claude Code
claude plugins install https://github.com/shareAI-lab/shareAI-skills
```

## 設定

```bash
# .env ファイルのオプション
ANTHROPIC_API_KEY=sk-ant-xxx      # 必須：あなたのAPIキー
ANTHROPIC_BASE_URL=https://...    # 任意：APIプロキシ用
MODEL_ID=claude-sonnet-4-5-20250929  # 任意：モデル選択
```

## 関連プロジェクト

| リポジトリ | 説明 |
|------------|------|
| [Kode](https://github.com/shareAI-lab/Kode) | 本番対応のオープンソースエージェントCLI |
| [shareAI-skills](https://github.com/shareAI-lab/shareAI-skills) | 本番用Skillsコレクション |
| [Agent Skills Spec](https://agentskills.io/specification) | 公式仕様 |

## 設計思想

> **モデルが80%、コードは20%。**

KodeやClaude Codeのような現代のエージェントが機能するのは、巧妙なエンジニアリングのためではなく、モデルがエージェントとして訓練されているからです。私たちの仕事は、モデルにツールを与えて、邪魔をしないことです。

## コントリビュート

コントリビューションを歓迎します！お気軽にissueやpull requestを送ってください。

- `skills/` に新しいサンプルSkillsを追加
- `docs/` のドキュメントを改善
- [Issues](https://github.com/shareAI-lab/learn-claude-code/issues) でバグ報告や機能提案

## ライセンス

MIT

---

**モデルがエージェント。これがすべての秘密。**

[@baicai003](https://x.com/baicai003) | [shareAI Lab](https://github.com/shareAI-lab)
