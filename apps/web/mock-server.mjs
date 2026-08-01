// @amadeus-header
// summary: Provides deterministic local API responses for web and native interface testing.
// layer: test
// status: test-only
// feature_flags: none
// provides:
// - cmd: npm run mock-api
// uses:
// - protocol: Amadeus REST and SSE APIs
// invariants:
// - Mock responses exercise reasoning, tools, compaction, streaming, and Markdown rendering.
// side_effects:
// - Opens a local HTTP listener.
// tests:
// - cmd: npm run mock-api
// @end-amadeus-header

import http from "node:http";
import { randomUUID } from "node:crypto";

const sessions = [{ id: randomUUID(), name: "Build the React MVP", profile: "default", status: "idle", parent_session_id: null }];
const histories = new Map([[sessions[0].id, [
  { role: "user", content: [{ type: "text", text: "Inspect the Amadeus HTTP APIs and build a polished React MVP." }] },
  { role: "assistant", content: [{ type: "text", text: "I mapped the session protocol and prepared the workspace. The app now has live sessions, streaming events, tool activity, approvals, history hydration, and cancellation." }] },
]]]);
const listeners = new Map();
const host = process.env.AMADEUS_MOCK_HOST || "127.0.0.1";
const port = Number(process.env.AMADEUS_MOCK_PORT || 3000);

function json(response, status, payload) {
  response.writeHead(status, { "Content-Type": "application/json", "Access-Control-Allow-Origin": "*", "Access-Control-Allow-Headers": "Content-Type", "Access-Control-Allow-Methods": "GET,POST,PUT,DELETE,OPTIONS" });
  response.end(JSON.stringify(payload));
}

function emit(sessionId, event, payload) {
  for (const response of listeners.get(sessionId) || []) response.write(`event: ${event}\ndata: ${JSON.stringify(payload)}\n\n`);
}

function readBody(request) {
  return new Promise((resolve) => {
    let body = "";
    request.on("data", (chunk) => { body += chunk; });
    request.on("end", () => resolve(body ? JSON.parse(body) : {}));
  });
}

const server = http.createServer(async (request, response) => {
  if (request.method === "OPTIONS") return json(response, 204, {});
  const url = new URL(request.url, `http://${host}:${port}`);
  const parts = url.pathname.split("/").filter(Boolean);
  if (url.pathname === "/health") return json(response, 200, { status: "ok", version: "mock" });
  if (url.pathname === "/config") return json(response, 200, { model: "mock-agent", context_window_size: 32768, prompt: { active_profile: "default", section_count: 4 }, tools: { active_profile: "default" } });
  if (url.pathname === "/tools/catalog") return json(response, 200, { tools: [{ name: "bash", level: "core", permission_mode: "workspace_write" }, { name: "read_file", level: "core", permission_mode: "read_only" }, { name: "web_fetch", level: "extended", permission_mode: "network" }] });
  if (url.pathname === "/v1/sessions" && request.method === "GET") return json(response, 200, { sessions, active_session_id: sessions[0]?.id || null });
  if (url.pathname === "/v1/sessions" && request.method === "POST") {
    const body = await readBody(request);
    const session = { id: randomUUID(), name: body.name || "New session", profile: body.profile || "default", status: "idle", parent_session_id: null };
    sessions.push(session); histories.set(session.id, []); return json(response, 201, session);
  }
  if (parts[0] === "v1" && parts[1] === "sessions") {
    const sessionId = parts[2];
    const session = sessions.find((item) => item.id === sessionId);
    if (!session) return json(response, 404, { error: "SessionNotFound", message: "Session not found" });
    if (parts.length === 3 && request.method === "GET") return json(response, 200, session);
    if (parts.length === 3 && request.method === "DELETE") { session.status = "closed"; return json(response, 200, { success: true }); }
    if (parts[3] === "history") return json(response, 200, { messages: histories.get(sessionId) || [], total: histories.get(sessionId)?.length || 0 });
    if (parts[3] === "approvals") return json(response, 200, []);
    if (parts[3] === "compact" && request.method === "POST") {
      const history = histories.get(sessionId) || [];
      const originalCount = history.length;
      const compactedCount = Math.min(originalCount, 1);
      const originalTokens = originalCount * 120;
      const newTokens = compactedCount * 120;
      if (originalCount > 1) histories.set(sessionId, history.slice(-1));
      emit(sessionId, "compaction", { original_count: originalCount, compacted_count: compactedCount, tokens_saved: originalTokens - newTokens, messages_summarized: originalCount - compactedCount });
      return json(response, 200, { original_count: originalCount, compacted_count: compactedCount, original_tokens: originalTokens, new_tokens: newTokens, tokens_saved: originalTokens - newTokens, messages_summarized: originalCount - compactedCount, status: originalCount > 1 ? "compressed" : "noop" });
    }
    if (parts[3] === "cancel") { session.status = "idle"; emit(sessionId, "session_state", session); return json(response, 200, { success: true }); }
    if (parts[3] === "events") {
      response.writeHead(200, { "Content-Type": "text/event-stream", "Cache-Control": "no-cache", "Connection": "keep-alive", "Access-Control-Allow-Origin": "*" });
      const current = listeners.get(sessionId) || new Set(); current.add(response); listeners.set(sessionId, current);
      response.write(`event: session_state\ndata: ${JSON.stringify(session)}\n\n`);
      request.on("close", () => current.delete(response)); return;
    }
    if (parts[3] === "messages" && request.method === "POST") {
      const body = await readBody(request); const history = histories.get(sessionId) || [];
      history.push({ role: "user", content: [{ type: "text", text: body.content }] }); session.status = "running";
      json(response, 202, { accepted: true, session_id: sessionId }); emit(sessionId, "session_state", session);
      setTimeout(() => emit(sessionId, "thinking", { delta: "I’ll inspect the relevant files and verify the implementation path." }), 120);
      setTimeout(() => emit(sessionId, "tool_start", { id: "tool-demo", name: "bash", command: "cargo check --features full", parent_id: null }), 500);
      setTimeout(() => emit(sessionId, "tool_output", { id: "tool-demo", delta: "Checking amadeus v0.1.0\n", parent_id: null }), 900);
      setTimeout(() => emit(sessionId, "tool_done", { id: "tool-demo", name: "bash", output: "Finished dev profile", is_error: false, parent_id: null }), 1300);
      const answer = "## Verification complete\n\nThe React workspace now renders **GitHub-flavored Markdown** safely.\n\n- Streaming content updates in place\n- Tables remain horizontally scrollable\n- Fenced code includes a copy action\n\n| Capability | Status |\n| --- | --- |\n| Reasoning disclosure | Available |\n| Markdown rendering | Available |\n\n```rust\ncargo check --features full\n```";
      const answerChunks = [answer.slice(0, 72), answer.slice(72, 188), answer.slice(188)];
      answerChunks.forEach((content, index) => {
        setTimeout(() => emit(sessionId, "text", { content }), 1500 + index * 120);
      });
      setTimeout(() => { history.push({ role: "assistant", content: [{ type: "text", text: answer }] }); session.status = "completed"; emit(sessionId, "token_usage", { input_tokens: 1834, output_tokens: 211, total_tokens: 2045, context_percent: 4 }); emit(sessionId, "done", { stop_reason: "end_turn", result: { text: answer, tool_calls: [] } }); emit(sessionId, "session_state", session); }, 1900);
      return;
    }
  }
  json(response, 404, { error: "NotFound", message: "Route not found" });
});

server.listen(port, host, () => console.log(`Mock Amadeus API listening on http://${host}:${port}`));
