// @amadeus-header
// summary: Verifies agent architecture presets, graph loops, imports, and runtime support reporting.
// layer: test
// status: test-only
// feature_flags: none
// provides:
// - cmd: npm test
// uses:
// - module: apps/web/src/agentArchitecture.js
// invariants:
// - Presets are valid control-flow graphs and only ReAct reports production support.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

import assert from "node:assert/strict";
import test from "node:test";
import { AGENT_ARCHITECTURE_PRESETS, architectureForExport, architectureRuntimeStatus, createAgentArchitecture, createArchitectureLibrary, parseArchitectureFile, validateAgentArchitecture } from "./agentArchitecture.js";

test("every architecture preset is a valid control-flow graph", () => {
  const library = createArchitectureLibrary();
  assert.equal(library.architectures.length, AGENT_ARCHITECTURE_PRESETS.length);
  library.architectures.forEach((architecture) => assert.equal(validateAgentArchitecture(architecture).isValid, true, architecture.preset));
});

test("ReAct contains an act-observe loop and is production runnable", () => {
  const architecture = createAgentArchitecture("ReAct", { preset: "react", id: "react" });
  const act = architecture.nodes.find(({ data }) => data.kind === "act");
  const observe = architecture.nodes.find(({ data }) => data.kind === "observe");
  assert.ok(architecture.edges.some(({ source, target }) => source === act.id && target === observe.id));
  assert.equal(architectureRuntimeStatus(architecture), "production");
});

test("plan and execute loops over execution steps but remains planned", () => {
  const architecture = createAgentArchitecture("Planner", { preset: "plan-execute", id: "planner" });
  const execute = architecture.nodes.find(({ data }) => data.kind === "execute");
  assert.ok(architecture.edges.some(({ target, label }) => target === execute.id && label === "more steps"));
  assert.equal(architectureRuntimeStatus(architecture), "planned");
});

test("validation reports missing entry, output, and dangling edges", () => {
  const architecture = createAgentArchitecture("Broken", { preset: "react", id: "broken" });
  architecture.entryNodeId = "missing";
  architecture.nodes = architecture.nodes.filter(({ data }) => data.kind !== "output");
  architecture.edges.push({ id: "bad", source: "missing", target: "also-missing", label: "next" });
  const validation = validateAgentArchitecture(architecture);
  assert.equal(validation.isValid, false);
  assert.ok(validation.errors.some(({ code }) => code === "entry_missing"));
  assert.ok(validation.errors.some(({ code }) => code === "output_required"));
  assert.ok(validation.errors.some(({ code }) => code === "dangling_edge"));
});

test("manifest import rejects the obsolete resource-binding schema", () => {
  assert.throws(() => parseArchitectureFile(JSON.stringify({ schemaVersion: 1, kind: "agent-architecture" })), /Unsupported agent architecture schema/);
  assert.throws(() => parseArchitectureFile("{"), /valid JSON/);
});

test("export strips editor-only node properties", () => {
  const architecture = createAgentArchitecture("Portable", { preset: "reflection" });
  architecture.nodes[0].selected = true;
  architecture.nodes[0].width = 240;
  const exported = architectureForExport(architecture);
  assert.equal(exported.nodes[0].selected, undefined);
  assert.equal(exported.nodes[0].width, undefined);
  assert.equal(exported.schemaVersion, 2);
});
