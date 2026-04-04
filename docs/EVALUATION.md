# Amadeus vs claw-code-parity Evaluation

*Generated: 2026-04-04*

## Project Overview

| | **Amadeus** | **claw-code-parity (Reference)** |
|---|---|---|
| **Language** | Rust | Rust + Python |
| **Purpose** | AI Agent SDK | Claude Code CLI implementation |
| **Scale** | ~1 crate, modular | 9 Rust crates, monorepo |
| **LOC** | ~5-10k estimated | 48,599 Rust LOC |
| **Tests** | Integration tests | 2,568 test LOC with mock harness |
| **Status** | Active development | Production-grade, 292 commits |

---

## Architecture Comparison

| Feature | **Amadeus** | **claw-code (Reference)** |
|---|---|---|
| **Agent Loop** | ReAct pattern in `loop_agent.rs` | Similar in `runtime/src/` |
| **Multi-Agent** | Supervisor + Workers | TaskRegistry, Team, Cron |
| **Tool System** | ToolRegistry with 8 built-in | 40 tool specs with stubs |
| **Policy/Approval** | Policy mod (Auto/Ask/Strict) | PermissionEnforcer (read-only, workspace-write, danger) |
| **Streaming** | Yes via `StreamEvent` | SSE support in `runtime/src/sse.rs` |
| **MCP** | Basic adapter | Full lifecycle bridge (`mcp_tool_bridge.rs`) |
| **LSP** | Not implemented | Full LSP client registry |
| **Session/History** | JSON/gzip logging | Session management |
| **Context Compaction** | `compaction.rs` | `compact.rs` + `summary_compression.rs` |

---

## Key Behavioral Gaps (Amadeus Missing)

| Feature | **Amadeus** | **claw-code** |
|---|---|---|
| **Bash validation** | Simple substring blocking | 9 validation submodules (path, sed, mode, destructive, etc.) |
| **File tool safety** | Basic path checks | Binary detection, size limits, symlink escape, canonical path |
| **Permission enforcement** | Generic Auto/Ask/Strict | Per-tool `required_permission` gating |
| **Task/Team/Cron registries** | Not implemented | Full in-memory registries |
| **MCP lifecycle** | Basic adapter | Full lifecycle bridge with auth, disconnect |
| **LSP client** | Not implemented | Diagnostics, hover, completion, symbols |
| **Mock parity harness** | None | `mock-anthropic-service` + `mock_parity_harness.rs` |
| **Config hierarchy** | Has bugs (boolean override) | User > Project > Local precedence |
| **Plugin system** | Skills only | Full PluginManager with install/enable/disable |
| **Output truncation** | Ad-hoc | Consistent truncation notice |

---

## What Amadeus Does Well

| | **Advantage over claw-code** |
|---|---|
| **Simplicity** | Single crate vs 9-crate workspace |
| **Provider abstraction** | Clean `LLMClient` trait, easy to swap Anthropic ↔ OpenAI |
| **Feature flags** | Better granularity (`tui`, `api`, `mesh`, etc.) |
| **Code size** | Smaller, easier to understand |
| **Generic Agent** | `Agent<C>` is generic over any LLM client |

---

## Maturity Gap

| Area | **Amadeus** | **claw-code** |
|---|---|---|
| **Tool surface** | ~8 tools | 40 tools |
| **Test coverage** | Basic integration tests | Mock harness + 10 scenarios + 19 captured requests |
| **Safety** | Simple blocking | 9 bash validation submodules + PermissionEnforcer |
| **Persistence** | File-based session logs | TaskRegistry, Team, Cron in-memory |
| **CI** | Not evident | `.github/workflows/rust-ci.yml` |
| **Docs** | CLAUDE.md + README | PARITY.md with commit-level evidence |

---

## Bottom Line

**Amadeus** is a clean, lightweight Rust SDK with good abstractions but **significantly less mature** than claw-code-parity. The reference has:

- **6x more Rust LOC** (48k vs ~8k)
- **5x more tool specs** (40 vs 8)
- **Full safety systems** (bash validation 9-submodule, permission enforcement)
- **Production test harness** with mock service and scripted scenarios
- **Full MCP/LSP/Task/Team/Cron registries**

Amadeus is architecturally sound but lacks the runtime hardening, safety depth, and test coverage of the reference. The 9-task parity improvement plan in `docs/plans/2026-04-03-claw-code-parity-improvements.md` is the right roadmap to close these gaps.

---

## Reference Files

- `refs/claw-code-parity/PARITY.md` - Main parity status document
- `refs/claw-code-parity/rust/PARITY.md` - Rust-specific parity details
- `refs/claw-code-parity/rust/mock_parity_scenarios.json` - Scenario manifest
