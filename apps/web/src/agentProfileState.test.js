// @amadeus-header
// summary: Verifies workspace-scoped persistence for customized agent profiles.
// layer: test
// status: test-only
// feature_flags: none
// provides:
// - tests: agent profile state behavior
// uses:
// - module: apps/web/src/agentProfileState.js
// invariants:
// - Agent profiles remain isolated by workspace and session.
// - Invalid and duplicated profile values are normalized.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

import assert from "node:assert/strict";
import test from "node:test";

import { defaultAgentProfile, loadAgentProfile, saveAgentProfile } from "./agentProfileState.js";

function memoryStorage() {
  const values = new Map();
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  };
}

test("creates an empty character profile from session identity", () => {
  assert.deepEqual(defaultAgentProfile({ id: "agent-1", name: "Planner" }), {
    displayName: "Planner",
    role: "",
    bio: "",
    likes: [],
    dislikes: [],
    avatarDataUrl: null,
  });
});

test("stores profiles independently by workspace and session", () => {
  const storage = memoryStorage();
  const first = { id: "agent-1", name: "Planner" };
  const second = { id: "agent-2", name: "Reviewer" };
  saveAgentProfile(storage, "workspace-a", first, { displayName: "Alice", likes: ["Concise plans"] });
  saveAgentProfile(storage, "workspace-a", second, { displayName: "Bob" });

  assert.equal(loadAgentProfile(storage, "workspace-a", first).displayName, "Alice");
  assert.equal(loadAgentProfile(storage, "workspace-a", second).displayName, "Bob");
  assert.equal(loadAgentProfile(storage, "workspace-b", first).displayName, "Planner");
});

test("normalizes tags and rejects unsupported avatar data", () => {
  const storage = memoryStorage();
  const session = { id: "agent-1", name: "Planner" };
  const profile = saveAgentProfile(storage, "workspace-a", session, {
    displayName: "  Planner Prime  ",
    likes: ["Tests", "tests", "  Clear plans  ", ""],
    dislikes: "surprises",
    avatarDataUrl: "data:image/svg+xml;base64,PHN2Zy8+",
  });

  assert.equal(profile.displayName, "Planner Prime");
  assert.deepEqual(profile.likes, ["Tests", "Clear plans"]);
  assert.deepEqual(profile.dislikes, []);
  assert.equal(profile.avatarDataUrl, null);
});
