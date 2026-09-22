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
// - Maximum transition limits are positive integers; other input falls back without corruption.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

import assert from "node:assert/strict";
import test from "node:test";
import { AGENT_ARCHITECTURE_PRESETS, architectureForExport, architectureRuntimeStatus, createAgentArchitecture, createArchitectureLibrary, createToolProfile, parseArchitectureFile, parsePositiveInt, resetArchitectureLayout, validateAgentArchitecture } from "./agentArchitecture.js";

test("parsePositiveInt accepts whole positive numbers and rejects everything else", () => {
  assert.equal(parsePositiveInt("1024", 7), 1024);
  assert.equal(parsePositiveInt("1e3", 7), 1000);
  assert.equal(parsePositiveInt(" 42 ", 7), 42);
  assert.equal(parsePositiveInt("abc", 7), 7);
  assert.equal(parsePositiveInt("1e3x", 7), 7);
  assert.equal(parsePositiveInt("3.7", 7), 7);
  assert.equal(parsePositiveInt("0", 7), 7);
  assert.equal(parsePositiveInt("-5", 7), 7);
  assert.equal(parsePositiveInt("", 7), 7);
  assert.equal(parsePositiveInt(undefined, 7), 7);
});

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

test("plan and execute loops over execution steps and is runnable", () => {
  const architecture = createAgentArchitecture("Planner", { preset: "plan-execute", id: "planner" });
  const execute = architecture.nodes.find(({ data }) => data.kind === "execute");
  assert.ok(architecture.edges.some(({ target, label }) => target === execute.id && label === "more steps"));
  assert.equal(architectureRuntimeStatus(architecture), "production");
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

test("tool profiles normalize selections and runtime policy", () => {
  const profile = createToolProfile({
    name: "restricted",
    selectionMode: "selected",
    enabledTools: ["read_file", "read_file", ""],
    disabledTools: ["bash"],
    allowAliases: false,
    includeMcp: false,
    includeControlPlane: false,
    modelPermissionMode: "read-only",
  });
  assert.deepEqual(profile.enabledTools, ["read_file"]);
  assert.equal(profile.selectionMode, "selected");
  assert.deepEqual(profile.disabledTools, ["bash"]);
  assert.equal(profile.allowAliases, false);
  assert.equal(profile.includeMcp, false);
  assert.equal(profile.modelPermissionMode, "read-only");
});

test("architecture export retains editable tool configuration", () => {
  const architecture = createAgentArchitecture("Restricted", { preset: "react" });
  architecture.toolProfile.enabledTools = ["read_file", "grep"];
  architecture.toolProfile.selectionMode = "selected";
  architecture.toolProfile.includeMcp = false;
  const exported = architectureForExport(architecture);
  assert.deepEqual(exported.toolProfile.enabledTools, ["read_file", "grep"]);
  assert.equal(exported.toolProfile.selectionMode, "selected");
  assert.equal(exported.toolProfile.includeMcp, false);
});

test("an explicit tool allowlist can intentionally be empty", () => {
  const profile = createToolProfile({ selectionMode: "selected", enabledTools: [] });
  assert.equal(profile.selectionMode, "selected");
  assert.deepEqual(profile.enabledTools, []);
});

test("reset layout restores preset positions and preserves the graph", () => {
  const architecture = createAgentArchitecture("ReAct", { preset: "react" });
  const dragged = { ...architecture, nodes: architecture.nodes.map((node, index) => ({ ...node, position: { x: index * 500, y: index * 300 } })) };
  const reset = resetArchitectureLayout(dragged);
  assert.deepEqual(reset.nodes.map(({ position }) => position), architecture.nodes.map(({ position }) => position));
  assert.deepEqual(reset.edges, dragged.edges);
  assert.deepEqual(reset.nodes.map(({ id, data }) => ({ id, label: data.label })), dragged.nodes.map(({ id, data }) => ({ id, label: data.label })));
});
