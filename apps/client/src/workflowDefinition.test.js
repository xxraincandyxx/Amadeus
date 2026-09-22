// @amadeus-header
// summary: Verifies task workflow graph creation, import normalization, and validation behavior.
// layer: test
// status: test-only
// feature_flags: none
// provides:
// - cmd: npm test
// uses:
// - module: apps/client/src/workflowDefinition.js
// invariants:
// - Invalid or disconnected graph definitions produce stable diagnostics.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

import assert from "node:assert/strict";
import test from "node:test";

import {
  createWorkflow,
  normalizeWorkflow,
  parseWorkflowFile,
  validateWorkflow,
  workflowForExport,
} from "./workflowDefinition.js";

test("default workflow is connected and valid", () => {
  const workflow = createWorkflow("Review");
  const validation = validateWorkflow(workflow);
  assert.equal(validation.isValid, true);
  assert.deepEqual(validation.warnings, []);
});

test("blank workflow reports recommended output and remains valid", () => {
  const workflow = createWorkflow("Blank", { blank: true, id: "blank" });
  const validation = validateWorkflow(workflow);
  assert.equal(validation.isValid, true);
  assert.deepEqual(validation.warnings, [{ code: "output_recommended" }]);
});

test("validation reports dangling edges and unreachable nodes", () => {
  const workflow = createWorkflow("Broken");
  workflow.edges = [{ id: "bad", source: workflow.entryNodeId, target: "missing", label: "" }];
  const validation = validateWorkflow(workflow);
  assert.equal(validation.isValid, false);
  assert.deepEqual(validation.errors, [{ code: "dangling_edge", count: 1 }]);
  assert.equal(validation.warnings[0].code, "unreachable_nodes");
});

test("import rejects invalid JSON and unsupported schema versions", () => {
  assert.throws(() => parseWorkflowFile("{"), /valid JSON/);
  assert.throws(() => parseWorkflowFile(JSON.stringify({ schemaVersion: 8 })), /Unsupported workflow schema/);
});

test("export normalization strips editor-only node properties", () => {
  const workflow = createWorkflow("Portable");
  workflow.nodes[0].selected = true;
  workflow.nodes[0].width = 240;
  const exported = workflowForExport(workflow);
  assert.equal(exported.nodes[0].selected, undefined);
  assert.equal(exported.nodes[0].width, undefined);
  assert.equal(normalizeWorkflow(exported).name, "Portable");
});
