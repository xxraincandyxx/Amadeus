// @amadeus-header
// summary: Verifies multi-agent hierarchy, summaries, and live session updates.
// layer: test
// status: test-only
// feature_flags: none
// provides:
// - cmd: npm test
// uses:
// - module: apps/client/src/agentSessions.js
// invariants:
// - Nested, orphaned, and cyclic sessions remain visible exactly once.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

import assert from "node:assert/strict";
import test from "node:test";

import { agentSessionRows, agentSessionSummary, sessionRelations, upsertSession } from "./agentSessions.js";

const sessions = [
  { id: "root", name: "Coordinator", status: "running", parent_session_id: null },
  { id: "child", name: "API review", status: "completed", parent_session_id: "root" },
  { id: "grandchild", name: "Test audit", status: "awaiting_approval", parent_session_id: "child" },
  { id: "orphan", name: "Detached", status: "failed", parent_session_id: "missing" },
];

test("agent rows preserve parent-first hierarchy and retain orphans", () => {
  const rows = agentSessionRows(sessions);

  assert.deepEqual(rows.map((row) => [row.session.id, row.depth]), [
    ["root", 0],
    ["child", 1],
    ["grandchild", 2],
    ["orphan", 0],
  ]);
  assert.equal(rows[0].childCount, 1);
  assert.equal(rows[1].parent.id, "root");
});

test("agent rows render cyclic references exactly once", () => {
  const rows = agentSessionRows([
    { id: "a", parent_session_id: "b", status: "idle" },
    { id: "b", parent_session_id: "a", status: "idle" },
  ]);

  assert.deepEqual(rows.map((row) => row.session.id), ["a", "b"]);
});

test("agent summary separates delegated and operational states", () => {
  assert.deepEqual(agentSessionSummary(sessions), {
    total: 4,
    delegated: 3,
    running: 1,
    awaitingApproval: 1,
    failed: 1,
    completed: 1,
  });
});

test("session relations expose parent, direct children, and depth", () => {
  const relations = sessionRelations(sessions, "child");

  assert.equal(relations.parent.id, "root");
  assert.deepEqual(relations.children.map((session) => session.id), ["grandchild"]);
  assert.equal(relations.depth, 1);
});

test("upsert inserts new children and merges status updates", () => {
  const inserted = upsertSession(sessions, {
    id: "new-child",
    name: "Docs",
    status: "running",
    parent_session_id: "root",
  });
  const updated = upsertSession(inserted, { id: "new-child", status: "completed" });

  assert.equal(updated.length, sessions.length + 1);
  assert.deepEqual(updated.at(-1), {
    id: "new-child",
    name: "Docs",
    status: "completed",
    parent_session_id: "root",
  });
});
