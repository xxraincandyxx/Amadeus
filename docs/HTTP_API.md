# Amadeus HTTP API

This document is the canonical reference for the HTTP adapter implemented by `crates/api`.

## Contract policy

The supported external interaction model is the versioned, session-oriented API under `/v1`. A session owns conversation history, one active turn, pending approvals, checkpoints, and an event stream. External applications should not model a live conversation through the older unversioned agent, stream, history, or approval resources.

Availability labels used below are:

| Label | Meaning |
|---|---|
| Stable | Versioned external contract intended for GUI and SDK integration |
| Available | Implemented but unversioned; compatibility can change before a versioned replacement exists |
| Experimental | Implemented for local or specialized workflows, without stability guarantees |
| Removed | No longer registered by the HTTP router |
| Informational | Reads persisted data but does not mutate a live session |

The current server has no authentication layer and allows every CORS origin. It is suitable for a trusted local environment. It must not be exposed to an untrusted network without an authenticating reverse proxy, origin restrictions, TLS, request limits, and workspace isolation.

## External session API

### Session model

```json
{
  "id": "d1ea92df-6bb0-43c2-8666-ae87d88c2716",
  "name": "Main Agent",
  "profile": "default",
  "status": "idle",
  "parent_session_id": null
}
```

Valid status values are `idle`, `running`, `awaiting_approval`, `completed`, `failed`, and `closed`.

### Endpoint inventory

| Method | Path | Status | Purpose |
|---|---|---|---|
| GET | `/v1/sessions` | Stable | List live sessions and the server-selected active session |
| POST | `/v1/sessions` | Stable | Create a live session |
| GET | `/v1/sessions/{id}` | Stable | Read live session metadata |
| DELETE | `/v1/sessions/{id}` | Stable | Close a session and abort its active turn |
| POST | `/v1/sessions/{id}/messages` | Stable | Start an asynchronous agent turn |
| GET | `/v1/sessions/{id}/events` | Stable | Subscribe to session events through SSE |
| GET | `/v1/sessions/{id}/history` | Stable | Retrieve the authoritative current conversation history |
| GET | `/v1/sessions/{id}/approvals` | Stable | List approval requests pending in the session |
| POST | `/v1/sessions/{id}/approvals/{approval_id}` | Stable | Resolve one pending approval |
| GET | `/v1/sessions/{id}/checkpoint` | Stable | Capture history and todo state |
| PUT | `/v1/sessions/{id}/checkpoint` | Stable | Restore history and todo state |
| POST | `/v1/sessions/{id}/cancel` | Stable | Stop the current turn and preserve the session |

### Create and list sessions

`POST /v1/sessions` accepts:

```json
{
  "name": "Research",
  "profile": "default"
}
```

`name` is optional. `profile` defaults to `default`; recognized built-in values are `default`, `debug`, `docs`, `review`, and `code_review`. Other values create a custom profile.

The response is `201 Created` with the session object. `GET /v1/sessions` returns:

```json
{
  "sessions": [],
  "active_session_id": null
}
```

The active identifier is advisory server state. A GUI should keep its selected session in client state and should not depend on a global switch operation.

### Client slash-command mapping

Slash commands are a React and macOS client interaction layer, not an HTTP protocol. They execute locally or compose existing endpoints and must never be sent to `POST /v1/sessions/{id}/messages` as model prompts.

| Client command | Implementation | API availability |
| --- | --- | --- |
| `/help` | Render the client command catalog | Local |
| `/new-agent [name]` | `POST /v1/sessions` and select the response | Stable |
| `/context` | Summarize client session state and latest `token_usage` | Local plus Stable SSE data |
| `/tools` | `GET /tools/catalog` | Available, unversioned |
| `/prompt` | `GET /config` | Available, unversioned |
| `/export [markdown\|json]` | Serialize the visible client timeline and download it | Local |
| `/settings` | Open the client connection dialog | Local |
| `/contribute` | Open the client contribution dialog | Local |
| `/cancel` | `POST /v1/sessions/{id}/cancel` when a turn is active | Stable |
| `/close` | `DELETE /v1/sessions/{id}` | Stable |

The client catalog intentionally omits core commands whose semantics depend on a TUI viewport, terminal process, or an unexposed orchestration operation. Adding such a command to an external client requires a suitable API contract first. A GUI should reject unknown commands locally and advertise only commands it can fully execute.

### Submit a message

`POST /v1/sessions/{id}/messages` accepts:

```json
{
  "content": "Explain the project architecture"
}
```

The server returns `202 Accepted` after starting the turn:

```json
{
  "accepted": true,
  "session_id": "..."
}
```

The response is not the agent result. Subscribe to the session event stream before submitting the message, then consume events until `done` or `error`. A session rejects a new message while its status is `running` or `awaiting_approval`.

### Event stream

`GET /v1/sessions/{id}/events` uses `text/event-stream`. Event names and payload availability are:

| Event | Status | Payload |
|---|---|---|
| `session_state` | Stable | Complete session metadata |
| `text` | Stable | `{ "content": string }` |
| `thinking` | Stable | `{ "delta": string }` |
| `thinking_complete` | Stable | `{ "thinking": string }` |
| `tool_start` | Stable | Tool id, name, optional command and parent id |
| `tool_input` | Stable | Incremental tool input |
| `tool_output` | Stable | Incremental tool output |
| `tool_progress` | Stable | Progress message and optional percentage |
| `tool_done` | Stable | Tool result, error flag, and parent id |
| `approval_request` | Stable | Approval id, tool, action/reason, and input |
| `subagent_requested` | Stable | Child request id, prompt, and depth |
| `subagent_session` | Stable | Parent/request metadata and child session |
| `token_usage` | Stable | Input, output, total tokens, and context percentage |
| `compaction` | Stable | Message counts and estimated tokens saved |
| `session_saved` | Available | Server-side session-log path |
| `done` | Stable | Final `RunResult` containing text and tool calls |
| `error` | Stable | `{ "message": string }` |

The event channel is live and bounded. It does not currently persist event IDs or replay missed deltas. After reconnect or page refresh, clients must call the history and session metadata endpoints to reconcile authoritative state. A client should establish the SSE subscription before submitting a message.

Text and reasoning events are emitted at provider-delta granularity. OpenAI-compatible providers that use genuine chunked SSE preserve their native timing. If a compatibility gateway labels a fully buffered response as SSE and supplies `Content-Length`, Amadeus progressively replays the contained deltas at a short cadence so external clients still receive incremental updates instead of one final burst. This improves rendering behavior but cannot reduce the gateway's time to first byte.

Reasoning availability is provider-dependent. OpenAI-compatible responses can expose `delta.reasoning_content`, `delta.reasoning`, `delta.analysis`, or textual entries under `delta.reasoning_details`; Anthropic-compatible responses can expose their thinking delta type. Amadeus normalizes those formats into `thinking` events. The React client also recognizes streamed `<think>...</think>` sections embedded in text and removes them from the final answer before display.

Clients must not infer private reasoning from ordinary untagged `text` events. When a response contains no supported reasoning channel, the React client reports Reasoning unavailable for that turn. The configured `gemma-4-26b-a4b-it-fp8-25603` deployment currently emits answer `content` only in direct probes, so it may show that availability state unless the gateway begins emitting a supported reasoning field or tagged section.

`text` event content can contain Markdown. The protocol transports text without prescribing a renderer. The Amadeus React client renders GitHub-flavored Markdown while streaming and after history hydration, escapes raw HTML by default, and opens rendered links with opener isolation. External clients should choose and document their own Markdown and sanitization policy.

### History

`GET /v1/sessions/{id}/history` returns the core serialized message representation:

```json
{
  "messages": [],
  "total": 0
}
```

This response is authoritative for refresh and reconnect. Content blocks can contain text, thinking, tool-use, and tool-result structures; clients must ignore unknown block fields for forward compatibility.

### Approvals

`GET /v1/sessions/{id}/approvals` returns complete pending approval requests. Resolve one with:

```http
POST /v1/sessions/{id}/approvals/{approval_id}
Content-Type: application/json

{ "decision": "approve" }
```

Allowed decisions are `approve`, `always_approve`, and `deny`. Approval identifiers are scoped to their session; clients must retain both identifiers from the event.

### Cancellation and close

`POST /v1/sessions/{id}/cancel` aborts only the active turn, clears pending approvals, and returns the session to `idle`. History accumulated before cancellation remains available.

`DELETE /v1/sessions/{id}` aborts active work and changes the session status to `closed`. A closed session rejects future messages.

### Checkpoints

`GET /v1/sessions/{id}/checkpoint` returns:

```json
{
  "history": [],
  "todos": []
}
```

Send the same representation to `PUT /v1/sessions/{id}/checkpoint`. Checkpoints are intended for rewind, local persistence, and conversation branching. Restore is only valid for a known live session.

## Unversioned endpoint inventory

The following endpoints remain registered but are not part of the stable external-session contract.

| Method | Path | Status | Notes |
|---|---|---|---|
| GET | `/health` | Available | Liveness and version information |
| POST | `/chat` | Experimental | Stateless orchestra task; no persistent conversation |
| POST | `/execute` | Experimental | Direct bash execution; high-risk and unsuitable for public exposure |
| POST | `/tasks` | Experimental | Stateless multi-agent task dispatch |
| GET | `/sessions` | Informational | Lists persisted session-log archives, not live sessions |
| GET | `/sessions/{id}` | Informational | Reads a persisted archive |
| POST | `/sessions/{id}/restore` | Informational | Validates an archive and reports its size; does not restore a live session |
| GET/PATCH | `/config` | Available | Process configuration inspection/update |
| GET | `/skills` | Available | Skill metadata |
| POST | `/summarize` | Available | Standalone summarization utility |
| GET/PATCH | `/compaction/config` | Available | Compaction configuration |
| GET | `/compaction/triggers` | Available | Trigger inventory |
| GET | `/prompts/sections` | Available | Prompt-section metadata |
| POST | `/prompts/build` | Available | Render a system prompt |
| GET | `/memory/providers` | Available | Memory provider inventory |
| GET/POST | `/memory/entries` | Available | Read or write memory entries |
| DELETE | `/memory/entries/{key}` | Available | Delete a memory entry |
| GET | `/tools/catalog` | Available | Tool metadata |
| POST | `/rag/ingest` | Available | Ingest text or a server-local path |
| POST | `/rag/query` | Available | Semantic search |
| GET | `/rag/documents` | Available | RAG document inventory |
| DELETE | `/rag/documents/{id}` | Available | Delete a RAG document |

The following historical routes are removed from router registration:

| Historical route | Status | Replacement |
|---|---|---|
| `GET /stream?message=...` | Removed | Subscribe to `/v1/sessions/{id}/events`, then POST a message |
| `GET /history` | Removed | `GET /v1/sessions/{id}/history` |
| `/agents/*` | Removed | `/v1/sessions/*` |
| `GET /approvals` | Removed | `GET /v1/sessions/{id}/approvals` |
| `POST /approvals/{id}` | Removed | `POST /v1/sessions/{id}/approvals/{approval_id}` |

## Errors

JSON errors use:

```json
{
  "error": "SessionNotFound",
  "message": "Session not found"
}
```

Common status codes are `201` for session creation, `202` for accepted turns, `400` for invalid session state, `404` for unknown sessions, and `422` for invalid message or approval input.

## React integration sequence

1. Call `POST /v1/sessions` or select an item returned by `GET /v1/sessions`.
2. Hydrate the view with `GET /v1/sessions/{id}/history`.
3. Open `GET /v1/sessions/{id}/events` through `EventSource` or an SSE client.
4. Submit text with `POST /v1/sessions/{id}/messages`.
5. Append deltas and tool activity from SSE.
6. Resolve `approval_request` events through the session-scoped approval endpoint.
7. On disconnect, reopen SSE and refresh metadata plus history.
8. Use `cancel` for a Stop button and `DELETE` only for permanent close.

## Known limitations

- No authentication or authorization middleware is included.
- CORS is currently unrestricted.
- SSE events have no durable replay cursor.
- Only text message submission is externally modeled; uploads and multimodal attachments are not available.
- Live sessions are process-local and are not reconstructed automatically after server restart.
- Orchestra topology and queued worker lifecycle do not yet have a stable versioned external contract.
- OpenAPI and generated TypeScript bindings are not yet published.
