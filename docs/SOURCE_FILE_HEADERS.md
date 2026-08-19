# Source File Headers

Hand-maintained source files use a machine-readable header to summarize their responsibility and boundary contracts. `python3 scripts/check_source_headers.py` validates the format.

## Scope

Headers are required for repository-owned Rust, Python, and shell files under `src/`, `tests/`, `examples/`, and `scripts/`, plus `verify.sh` and `count-code.sh`.

Exclude generated files, fixtures, snapshots, lockfiles, documentation, vendored code, and `refs/`.

## Format

Place the block before imports or items. A script shebang may precede it. Use `//` for Rust and `#` for Python or shell.

Every field is required and must appear in this order:

| Field | Value |
| --- | --- |
| `summary` | One sentence, at most 24 words |
| `layer` | Architectural area |
| `status` | `active`, `experimental`, `deprecated`, `test-only`, or `generated` |
| `feature_flags` | Relevant gates, or `none` |
| `provides` | Interfaces defined by the file, or `none` |
| `uses` | Boundary interfaces consumed by the file, or `none` |
| `invariants` | Non-obvious conditions edits must preserve, or `none` |
| `side_effects` | Observable effects outside local computation, or `none` |
| `tests` | Primary verification targets, or `none` |

List fields accept `none` or one item per commented line. `provides` and `uses` entries use one of these prefixes:

`module`, `type`, `trait`, `fn`, `const`, `tool`, `route`, `event`, `cmd`, `format`, `artifact`, `env`, `protocol`, `runtime`.

Include contracts that cross a file, module, package, process, or persistence boundary. Omit ordinary imports, private helpers, history, authorship, and implementation detail.

## Example

```rust
// @amadeus-header
// summary: ReAct loop for LLM turns, tool execution, approvals, and session logging.
// layer: agent
// status: active
// feature_flags:
// - full
// provides:
// - type: crate::agent::loop_agent::Agent<C>
// - event: crate::agent::events::AgentEvent
// uses:
// - trait: crate::client::LLMClient
// - runtime: tokio tasks and channels
// invariants:
// - Tool-use and tool-result history stay order-aligned.
// side_effects:
// - Sends agent events across channels.
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header
```

## Maintenance

- Add a valid header with every new in-scope file.
- When touching a file, update any field affected by interface, feature, side-effect, invariant, or verification changes.
- Keep lists short, accurate, sorted by kind and name where practical, and free of placeholders such as `TBD`, `TODO`, `etc`, or `misc`.
- Use stable, repository-qualified names. Do not turn headers into changelogs.
- Treat missing, malformed, incomplete, or stale headers as merge blockers.
