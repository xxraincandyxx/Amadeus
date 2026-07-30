# Contributing to Amadeus

Amadeus is a Rust agent SDK with terminal, HTTP, web, and native desktop interfaces. Contributions should improve the shared runtime without allowing its product surfaces to drift apart.

## Before changing code

1. Create a focused topic branch from `master`, such as `fix/streaming-buffer` or `feat/api-health`.
2. Read `AGENTS.md`, `docs/ARCHITECTURE.md`, and the documentation for the area you will change.
3. Use GitNexus impact analysis before modifying an existing symbol. Resolve every direct caller and warn before proceeding when the reported risk is high or critical.
4. Keep changes scoped. Do not combine runtime refactors, API changes, and visual redesigns in one commit unless they are inseparable.
5. Check the relevant feature flags. Development and repository-wide verification use `--features full` because this crate has no default features. Document new feature gates in `README.md` and keep `verify.sh` aligned.

## Development setup

Install Rust, Node.js, and the macOS Command Line Tools. Configure an Amadeus provider in `.amadeus/settings.json` when testing the real agent runtime; never commit credentials or local settings.

```bash
cargo check --features full

cd apps/web
npm install
npm run lint
npm run build
```

The web and desktop clients can be developed without an LLM credential by running the mock API:

```bash
cd apps/web
npm run mock-api
```

See `docs/MACOS_APP.md` for native development and packaging.

## Change standards

### Rust and agent core

- Preserve the generic `Agent<C: LLMClient>` and `Tool` contracts unless an API migration is explicitly planned.
- Use `crate::error::Result<T>` and avoid `unwrap()` or `expect()` outside tests.
- Review the source-file header whenever an in-scope source file is touched. New source files must follow `docs/SOURCE_FILE_HEADERS.md`.
- Add deterministic tests with the mock LLM or HTTP mocking surfaces.

### HTTP API

- Treat `/v1/sessions/*` as the external client contract.
- Document stability, authentication assumptions, events, and error behavior in `docs/HTTP_API.md`.
- Add compatibility tests before removing or changing an available endpoint.
- Do not expose an unauthenticated server directly to an untrusted network.

### Web and desktop interfaces

- Follow `docs/WEB_DESIGN_SYSTEM.md` before adding components or changing tokens.
- Use Phosphor icons and the existing sparkle mark. Do not add a second icon family.
- Implement loading, empty, offline, error, success, disabled, focus, and reduced-motion behavior where relevant.
- Verify both a desktop viewport and a 390 × 844 mobile viewport.
- Keep the React app browser-compatible. Native-only behavior must be gated behind the Tauri runtime.
- Keep native capabilities minimal. A frontend feature does not automatically justify filesystem, shell, or process permissions.

## Verification

Run the smallest relevant checks while iterating, then run the full set before submitting a cross-cutting change.

```bash
cargo fmt --all -- --check
cargo check --features full
cargo clippy --all-features --all-targets -- -D warnings
cargo test --features full
python3 scripts/check_source_headers.py

cd apps/web
npm run lint
npm run build
npm audit --omit=dev
cargo check --manifest-path src-tauri/Cargo.toml
npm run desktop:build
```

Run `./verify.sh` when the complete repository suite is appropriate. Before each commit, stage only its intended files, run `npx gitnexus detect-changes --scope staged`, and check `git diff --cached --check`.

Use narrower commands while iterating, such as `cargo test --test tool_approval_test --features full`.

## Commits and pull requests

Prefer small Conventional Commit messages such as:

```text
feat(api): add session checkpoint endpoint
feat(web): expose runtime connection settings
fix(desktop): preserve titlebar spacing on small windows
docs: define interface contribution workflow
```

Each pull request should explain the user-visible outcome, the affected interfaces, security or compatibility implications, and the verification performed. Include screenshots for visual changes and state whether the native app, browser app, or both were checked.
