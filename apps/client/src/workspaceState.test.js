// @amadeus-header
// summary: Verifies per-user workspace identity and last-session persistence.
// layer: test
// status: active
// feature_flags: none
// provides:
// - tests: workspace state behavior
// uses:
// - module: apps/client/src/workspaceState.js
// invariants:
// - Workspaces remain isolated by endpoint and working directory.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

import assert from "node:assert/strict";
import test from "node:test";

import {
  loadWorkspaceActiveSession,
  loadWorkspaceRegistry,
  rememberWorkspace,
  saveWorkspaceActiveSession,
  workspaceFromConfig,
} from "./workspaceState.js";

function memoryStorage() {
  const values = new Map();
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  };
}

test("workspace identity is stable and uses the project directory name", () => {
  const first = workspaceFromConfig({ working_dir: "/Users/alice/Dev/amadeus/" }, "http://127.0.0.1:3000/");
  const second = workspaceFromConfig({ working_dir: "/Users/alice/Dev/amadeus" }, "http://127.0.0.1:3000");

  assert.deepEqual(first, second);
  assert.equal(first.name, "amadeus");
});

test("workspace records are isolated by local user storage", () => {
  const alice = memoryStorage();
  const bob = memoryStorage();
  const workspace = rememberWorkspace(alice, { working_dir: "/repo/amadeus" }, "http://localhost:3000", 10);

  saveWorkspaceActiveSession(alice, workspace.id, "session-alice");

  assert.equal(loadWorkspaceActiveSession(alice, workspace.id), "session-alice");
  assert.equal(loadWorkspaceActiveSession(bob, workspace.id), null);
  assert.deepEqual(loadWorkspaceRegistry(bob).workspaces, {});
});

test("remembering another workspace preserves prior workspace state", () => {
  const storage = memoryStorage();
  const first = rememberWorkspace(storage, { working_dir: "/repo/one" }, "http://localhost:3000", 10);
  saveWorkspaceActiveSession(storage, first.id, "session-one");
  const second = rememberWorkspace(storage, { working_dir: "/repo/two" }, "http://localhost:3000", 20);
  const registry = loadWorkspaceRegistry(storage);

  assert.equal(registry.activeWorkspaceId, second.id);
  assert.equal(registry.workspaces[first.id].activeSessionId, "session-one");
  assert.equal(registry.workspaces[second.id].name, "two");
});
