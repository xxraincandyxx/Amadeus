# Amadeus Web

React and Tauri client for the versioned Amadeus session API. The interface provides live-session navigation, history hydration, SSE streaming, tool activity, approvals, cancellation, runtime connection settings, context usage, and responsive desktop/mobile layouts.

When a provider exposes a distinct reasoning stream, live thinking appears in an expanded inline disclosure. Completed reasoning collapses automatically and remains available through explicit Show and Hide controls. OpenAI-compatible reasoning fields and streamed `<think>...</think>` sections are separated from the final answer. When a model supplies neither format, the timeline displays an explicit Reasoning unavailable status instead of silently omitting the disclosure. Ordinary untagged answer content is never guessed or reclassified as reasoning.

Assistant answers render safe GitHub-flavored Markdown, including headings, emphasis, links, lists, task lists, blockquotes, tables, inline code, and fenced code blocks with copy actions. Raw HTML is escaped by default. Markdown is rendered during streaming as well as after history hydration.

## Run with the Amadeus server

From the repository root:

```bash
cargo run --features full -- --server 3000
```

In another terminal:

```bash
cd apps/web
npm install
npm run dev
```

Open `http://127.0.0.1:5173`.

The default API address is `http://127.0.0.1:3000`. Set a development default when necessary:

```bash
VITE_AMADEUS_API_URL=http://localhost:8080 npm run dev
```

The gear button opens runtime Connection settings. The saved endpoint overrides the build-time default, is stored under `amadeus.apiUrl`, and can be tested or reset without rebuilding the client.

## UI-only demo

The mock server exercises message submission, reasoning, tool execution, streaming GitHub-flavored Markdown, completion, token usage, session creation, and cancellation without an LLM credential:

```bash
npm run mock-api
npm run dev
```

If port 3000 is already in use, select another IPv4 port and configure the client through Connection settings:

```bash
AMADEUS_MOCK_PORT=3100 npm run mock-api
```

The mock server binds to `127.0.0.1` by default. Override `AMADEUS_MOCK_HOST` only when a different local interface is required.

## Native macOS app

The same workspace runs in a Tauri 2 shell with native window controls:

```bash
npm run desktop:dev
npm run desktop:build
```

The release bundle is generated at `src-tauri/target/release/bundle/macos/Amadeus.app`. It contains the native HTTP client and a supervised Amadeus server sidecar, so a separately started server is not required. Provider settings and credentials remain external to the bundle. See [`../../docs/MACOS_APP.md`](../../docs/MACOS_APP.md) for architecture, configuration, signing, packaging, and security details.

Use `npm run desktop:icon` after changing `src-tauri/app-icon.svg`.

## Verification

```bash
npm test
npm run lint
npm run build
npm audit --omit=dev
cargo check --manifest-path src-tauri/Cargo.toml
npm run desktop:build
```

The web app is intended for trusted local use. The Amadeus HTTP server currently has unrestricted CORS and no built-in authentication; see `docs/HTTP_API.md` before any remote deployment.

Visual changes must follow [`../../docs/WEB_DESIGN_SYSTEM.md`](../../docs/WEB_DESIGN_SYSTEM.md) and the repository workflow in [`../../CONTRIBUTING.md`](../../CONTRIBUTING.md).
