// @amadeus-header
// summary: Verifies agent architecture manifests, typed bindings, and core composition validation.
// layer: test
// status: test-only
// feature_flags: none
// provides:
// - cmd: npm test
// uses:
// - module: apps/web/src/agentArchitecture.js
// invariants:
// - Valid manifests contain one model bound to one runtime.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

import assert from "node:assert/strict";
import test from "node:test";

import {
  architectureBuilderSteps,
  architectureForExport,
  createAgentArchitecture,
  createArchitectureComponent,
  parseArchitectureFile,
  validateAgentArchitecture,
} from "./agentArchitecture.js";

test("default agent architecture maps to a valid AgentBuilder composition", () => {
  const architecture = createAgentArchitecture("Reviewer");
  const validation = validateAgentArchitecture(architecture);
  assert.equal(validation.isValid, true);
  assert.deepEqual(validation.warnings, []);
  assert.deepEqual(architectureBuilderSteps(architecture), [
    "AgentBuilder::new(client, config)",
    "AgentBuilder::with_prompt_profile",
    "AgentBuilder::with_default_tools / with_tool_profile",
    "AgentBuilder::with_policy",
    "AgentBuilder::with_subagent_delegate / with_subagent_depth",
    "AgentBuilder::build",
  ]);
});

test("blank architecture requires a model binding", () => {
  const architecture = createAgentArchitecture("Blank", { blank: true });
  const validation = validateAgentArchitecture(architecture);
  assert.equal(validation.isValid, false);
  assert.deepEqual(validation.errors, [{ code: "model_count", count: 0 }]);
});

test("validation rejects duplicate singleton components and execution-style edges", () => {
  const architecture = createAgentArchitecture("Invalid");
  const secondPrompt = createArchitectureComponent("prompt", { x: 0, y: 0 }, "prompt-2");
  architecture.components.push(secondPrompt);
  architecture.bindings.push({ id: "invalid", source: architecture.components[0].id, target: secondPrompt.id, label: "then" });
  const validation = validateAgentArchitecture(architecture);
  assert.equal(validation.isValid, false);
  assert.ok(validation.errors.some(({ code }) => code === "duplicate_component_kind"));
  assert.ok(validation.errors.some(({ code }) => code === "invalid_binding"));
});

test("manifest import is distinct from task workflow JSON", () => {
  assert.throws(() => parseArchitectureFile(JSON.stringify({ schemaVersion: 1, kind: "workflow" })), /Unsupported agent architecture schema/);
  assert.throws(() => parseArchitectureFile("{"), /valid JSON/);
});

test("export strips editor-only component properties", () => {
  const architecture = createAgentArchitecture("Portable");
  architecture.components[0].selected = true;
  architecture.components[0].width = 240;
  const exported = architectureForExport(architecture);
  assert.equal(exported.components[0].selected, undefined);
  assert.equal(exported.components[0].width, undefined);
  assert.equal(exported.kind, "agent-architecture");
});
