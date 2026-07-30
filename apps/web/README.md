# Amadeus Web

React MVP for the versioned Amadeus session API. The interface provides live-session navigation, history hydration, SSE streaming, tool activity, approvals, cancellation, context usage, and responsive desktop/mobile layouts.

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

The default API address is `http://127.0.0.1:3000`. Override it when necessary:

```bash
VITE_AMADEUS_API_URL=http://localhost:8080 npm run dev
```

## UI-only demo

The mock server exercises message submission, reasoning, tool execution, streaming text, completion, token usage, session creation, and cancellation without an LLM credential:

```bash
npm run mock-api
npm run dev
```

## Verification

```bash
npm run lint
npm run build
npm audit --omit=dev
```

The web app is intended for trusted local use. The Amadeus HTTP server currently has unrestricted CORS and no built-in authentication; see `docs/HTTP_API.md` before any remote deployment.
