# Welcome to the Amadeus community!

We welcome contributors of all backgrounds and experience levels. A community with diverse perspectives builds software with lasting impact.

[🤝 How you can help](#how-you-can-help) · [⚙️ Setting up your environment](#setting-up-your-environment) · [📝 Submitting issues and pull requests](#submitting-issues-and-pull-requests) · [🔀 Pull requests](#pull-requests) · [❤️ Code of conduct](#code-of-conduct)

## How you can help

- Report bugs with clear, reproducible steps
- Improve documentation to make the project more accessible
- Triage issues by providing feedback, testing, and validation
- Propose and implement enhancements of all sizes
- Share the agents, skills, and workflows you build on Amadeus

## Setting up your environment

Amadeus is a Rust workspace with a web/desktop client in `apps/client`. See the
[Quickstart](README.md#quickstart) for prerequisites and
[DEVELOPMENT.md](DEVELOPMENT.md) for architecture-oriented setup. In short:

```bash
cargo check --features full

cd apps/client
npm install
npm run lint
npm run build
```

The crate has no default features — development and repository-wide
verification always use `--features full`.

The web and desktop clients can be developed without an LLM credential by
running the mock API:

```bash
cd apps/client
npm run mock-api
```

Configure a real provider in `.amadeus/settings.json` when testing the agent
runtime, and never commit credentials or local settings. See
[docs/MACOS_APP.md](docs/MACOS_APP.md) for native development and packaging.

## Submitting issues and pull requests

We maintain high standards to ensure quality across the project.

Your contributions are evaluated on:

- Technical quality and correctness
- Adherence to existing conventions and architectural patterns
- Demonstrated understanding of the implementation and its implications
- Clarity of communication
- Maintainability

You may use productivity tools, including AI-assisted coding, to help you work
more efficiently. However, you remain fully accountable for all submitted code,
issues, pull requests, and comments. AI-generated content must meet the same
standards as human-written contributions. Review, validate, and, as needed,
refine or rearrange AI-assisted work so the final contribution reflects your
human creativity, understanding, and control. Use your voice and expression
when producing written materials. Misuse of AI tools in your contributions and
conversations may be considered a violation of our [Code of Conduct](#code-of-conduct).

### Issues

Before submitting an issue:

- Search [existing issues](../../issues) to avoid duplicates.
- Write a clear, concise title and a detailed description. Ensure all information is accurate and fully understood by you.
- Include precise steps to reproduce the issue, expected behavior, and actual behavior. Verify these steps yourself.
- Specify your environment: OS version, Amadeus version, the client surface (desktop app, browser, TUI, HTTP API, Python SDK), the provider and model, and any relevant configuration.
- Explain the severity and impact of the problem.
- For security issues, see [SECURITY.md](SECURITY.md). Do **not** open a public GitHub issue.

#### Proposing features

We welcome feature requests. Describe the proposed changes needed and why they
matter. Please do not start with a pull request. Use the issue template that
best matches your needs.

## Pull requests

⚠️ **Important:** We recommend an issue for larger changes to the codebase.

Pull requests represent proposed solutions or enhancements. When submitting a
pull request, verify it meets these expectations:

- They should match an open issue but may not always.
- Address the stated problem or feature request completely and effectively. You must fully understand and validate your proposed solution.
- Solutions must be thorough, handle edge cases, and integrate cleanly.
- Include comprehensive tests that validate your changes and prevent regressions.
- If performance may be impacted, run benchmarks for both the main branch and the pull request, and include scripts and reproduction steps.
- Create a focused topic branch from `master`, such as `fix/streaming-buffer` or `feat/api-health`, and keep the change scoped. Do not combine runtime refactors, API changes, and visual redesigns in one pull request unless they are inseparable.

### Area standards

#### Rust and agent core

- Preserve the generic `Agent<C: LLMClient>` and `Tool` contracts unless an API migration is explicitly planned.
- Use `crate::error::Result<T>` and avoid `unwrap()` or `expect()` outside tests.
- Review the source-file header whenever an in-scope source file is touched. New source files must follow [docs/SOURCE_FILE_HEADERS.md](docs/SOURCE_FILE_HEADERS.md).
- Add deterministic tests with the mock LLM or HTTP mocking surfaces.

#### HTTP API

- Treat `/v1/sessions/*` as the external client contract.
- Document stability, authentication assumptions, events, and error behavior in [docs/HTTP_API.md](docs/HTTP_API.md).
- Add compatibility tests before removing or changing an available endpoint.
- Do not expose an unauthenticated server directly to an untrusted network.

#### Web and desktop interfaces

- Follow [docs/WEB_DESIGN_SYSTEM.md](docs/WEB_DESIGN_SYSTEM.md) before adding components or changing tokens.
- Use Phosphor icons and the existing sparkle mark. Do not add a second icon family.
- Implement loading, empty, offline, error, success, disabled, focus, and reduced-motion behavior where relevant.
- Verify both a desktop viewport and a 390 × 844 mobile viewport.
- Keep the React app browser-compatible. Native-only behavior must be gated behind the Tauri runtime.
- Keep native capabilities minimal. A frontend feature does not automatically justify filesystem, shell, or process permissions.

### Pull request style guide and format

Follow our coding style and formatting to maintain a consistent, readable codebase.

- Write comprehensive and clear documentation. Concisely explain the "why" behind complex decisions.
- Write clear, concise, and descriptive commit messages. Prefer small Conventional Commit messages such as:

  ```text
  feat(api): add session checkpoint endpoint
  feat(web): expose runtime connection settings
  fix(desktop): preserve titlebar spacing on small windows
  docs: define interface contribution workflow
  ```

- Run `cargo fmt --all` and `cargo clippy --all-features -- -D warnings` before submitting.
- Read [AGENTS.md](AGENTS.md), [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), and the documentation for the area you change.
- Use GitNexus impact analysis before modifying an existing symbol, and run `detect_changes` before committing, as described in [AGENTS.md](AGENTS.md).
- Document new feature gates in [README.md](README.md) and keep `verify.sh` aligned.

### Testing

All contributions require thorough testing.

**Local testing:** Ensure all existing tests pass before submitting:

```bash
cargo test --features full

cd apps/client
npm run lint
npm run test
```

**Automated tests:** New features and bug fixes require corresponding automated
tests that validate the intended behavior and prevent regressions.

**Full verification:** Run `./verify.sh` — the CI-parity gate — when the change
touches shared infrastructure:

```bash
./verify.sh
```

Use narrower commands while iterating, such as
`cargo test --test tool_approval_test --features full`.

### Running CI

CI runs automatically on every pull request. Before each commit, stage only the
intended files and check `git diff --cached --check`.

## Code of conduct

We are committed to fostering a community where different experiences and
perspectives come together to create and collaborate. Good collaboration
depends on honest feedback and respect for the time and effort every
contributor brings to this project. [Please review our Code of
Conduct](CODE_OF_CONDUCT.md); all community members are expected to adhere to
it.
