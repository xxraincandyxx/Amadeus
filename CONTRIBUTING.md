# Contributing

## Scope
Keep changes focused. If you are fixing a bug and adding a feature, split them into separate pull requests when possible.

## Workflow
1. Create a topic branch from `main`, for example `fix/streaming-buffer` or `feat/api-health`.
2. Make the smallest coherent change that solves the problem.
3. Add or update tests for user-visible behavior, policy changes, or concurrency fixes.
4. Run local checks before opening a PR:

```bash
cargo fmt --all
cargo clippy --all-features -- -D warnings
cargo test --features full
./verify.sh
```

Use narrower commands while iterating, such as `cargo test --test tool_approval_test --features full`.

## Code Expectations
- Follow Rust 2021 idioms and keep modules narrow in responsibility.
- Prefer `Result`-based error handling over panics in production code.
- Avoid unrelated refactors in the same PR.
- If you add a new feature gate, document it in `README.md` and keep `verify.sh` aligned with the supported matrix.
- For in-scope source files, maintain the required file header defined in `docs/SOURCE_FILE_HEADERS.md`.
- Treat header updates as part of the implementation, not optional documentation cleanup.

## Tests and Docs
Bug fixes should come with a regression test when practical. Update `README.md`, `docs/`, examples, or inline help when behavior changes.

If a change affects an in-scope source file’s responsibilities, interfaces, invariants, side effects, or primary verification path, update that file’s header in the same PR.

## Pull Requests
Include:
- a short problem statement,
- the implementation approach,
- test coverage or verification notes,
- screenshots or terminal captures for TUI/API behavior changes,
- linked issues or follow-up work, if any.

Recent history follows Conventional Commit style with scopes, such as `feat(ui): ...` and `fix(agent): ...`. Matching that format keeps history readable.
