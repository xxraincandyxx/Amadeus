// @amadeus-header
// summary: Defines persisted control-flow graphs and presets for designing Amadeus agents.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - const: AGENT_ARCHITECTURE_PRESETS
// - const: AGENT_NODE_TYPES
// - fn: createAgentArchitecture
// - fn: createArchitectureNode
// - fn: createArchitectureLibrary
// - fn: createToolProfile
// - fn: loadArchitectureLibrary
// - fn: parseArchitectureFile
// - fn: validateAgentArchitecture
// - fn: architectureForExport
// uses:
// - format: Amadeus agent architecture manifest version 2
// invariants:
// - Graph edges represent agent control transitions, including loops and branches.
// - Runtime support is reported separately from structural graph validity.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

export const AGENT_ARCHITECTURE_SCHEMA_VERSION = 2;
export const AGENT_ARCHITECTURE_STORAGE_KEY = "amadeus.agentArchitectureLibrary.v2";

export const AGENT_TOOL_PERMISSION_MODES = ["read-only", "workspace-write", "danger-full-access"];

export const AGENT_ARCHITECTURE_PRESETS = [
  { id: "react", label: "ReAct", description: "Reason, act with tools, observe results, and repeat until complete.", runtimeStatus: "production" },
  { id: "plan-execute", label: "Plan and execute", description: "Create a plan, execute one step at a time, then synthesize the result.", runtimeStatus: "production" },
  { id: "reflection", label: "Reflection", description: "Draft, critique, and revise until the quality gate passes.", runtimeStatus: "production" },
  { id: "supervisor-team", label: "Supervisor team", description: "Route work to specialists, review their output, and decide whether to continue.", runtimeStatus: "production" },
];

export const AGENT_NODE_TYPES = [
  { type: "input", label: "Input", description: "Initializes typed workflow state.", group: "Flow", fields: [] },
  { type: "observe", label: "Observe", description: "Reads messages, tool results, and memory.", group: "Reasoning", fields: [{ key: "source", label: "Observation source", defaultValue: "conversation + tools" }] },
  { type: "reason", label: "Reason", description: "Invokes the model to choose the next transition.", group: "Reasoning", fields: [{ key: "instruction", label: "Reasoning instruction", defaultValue: "Choose the next action or finish.", multiline: true }] },
  { type: "act", label: "Act", description: "Executes the selected tool under policy.", group: "Effects", fields: [{ key: "toolProfile", label: "Tool profile", defaultValue: "default" }] },
  { type: "plan", label: "Plan", description: "Produces or revises a structured task plan.", group: "Reasoning", fields: [{ key: "instruction", label: "Planning instruction", defaultValue: "Create an ordered plan with verifiable steps.", multiline: true }] },
  { type: "execute", label: "Execute step", description: "Runs the current plan step using model and tools.", group: "Effects", fields: [{ key: "toolProfile", label: "Tool profile", defaultValue: "default" }] },
  { type: "route", label: "Route", description: "Selects a declared transition from workflow state.", group: "Control", fields: [{ key: "condition", label: "Routing condition", defaultValue: "state.next" }] },
  { type: "critique", label: "Critique", description: "Evaluates output against explicit criteria.", group: "Reasoning", fields: [{ key: "criteria", label: "Review criteria", defaultValue: "correctness, completeness, clarity", multiline: true }] },
  { type: "revise", label: "Revise", description: "Improves state using critique feedback.", group: "Reasoning", fields: [{ key: "instruction", label: "Revision instruction", defaultValue: "Address every actionable critique.", multiline: true }] },
  { type: "delegate", label: "Delegate", description: "Creates or selects a specialist agent session.", group: "Multi-agent", fields: [{ key: "capability", label: "Required capability", defaultValue: "specialist" }] },
  { type: "review", label: "Review", description: "Checks delegated output before accepting it.", group: "Multi-agent", fields: [{ key: "criteria", label: "Acceptance criteria", defaultValue: "task complete and verified", multiline: true }] },
  { type: "approval", label: "Approval gate", description: "Suspends the run for an external decision.", group: "Control", fields: [{ key: "reason", label: "Suspension reason", defaultValue: "Approval required" }] },
  { type: "synthesize", label: "Synthesize", description: "Combines accumulated state into a final answer.", group: "Reasoning", fields: [{ key: "instruction", label: "Synthesis instruction", defaultValue: "Produce the final response from verified state.", multiline: true }] },
  { type: "output", label: "Complete", description: "Ends the run with final typed state.", group: "Flow", fields: [] },
];

const NODE_TYPE_SET = new Set(AGENT_NODE_TYPES.map(({ type }) => type));
let nextLocalId = 0;

function uniqueId(prefix) {
  nextLocalId += 1;
  return `${prefix}-${Date.now().toString(36)}-${nextLocalId.toString(36)}`;
}

function text(value, fallback = "") { return typeof value === "string" ? value : fallback; }

export function parsePositiveInt(value, fallback) {
  const parsed = Number(value);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : fallback;
}

function normalizedPosition(position) {
  return { x: Number.isFinite(position?.x) ? position.x : 0, y: Number.isFinite(position?.y) ? position.y : 0 };
}

function serializableData(data = {}) {
  return Object.fromEntries(Object.entries(data).filter(([, value]) => typeof value === "string" || typeof value === "boolean" || Number.isFinite(value)));
}

function uniqueStrings(value) {
  return Array.isArray(value) ? [...new Set(value.filter((item) => typeof item === "string" && item.trim()).map((item) => item.trim()))] : [];
}

export function createToolProfile(value = {}) {
  const permissionMode = AGENT_TOOL_PERMISSION_MODES.includes(value.modelPermissionMode) ? value.modelPermissionMode : "danger-full-access";
  return {
    name: text(value.name, "default"),
    selectionMode: value.selectionMode === "selected" ? "selected" : "all",
    enabledTools: uniqueStrings(value.enabledTools),
    disabledTools: uniqueStrings(value.disabledTools),
    allowAliases: typeof value.allowAliases === "boolean" ? value.allowAliases : true,
    includeMcp: typeof value.includeMcp === "boolean" ? value.includeMcp : true,
    includeControlPlane: typeof value.includeControlPlane === "boolean" ? value.includeControlPlane : true,
    modelPermissionMode: permissionMode,
  };
}

export function nodeDefinition(type) { return AGENT_NODE_TYPES.find((candidate) => candidate.type === type); }
export function presetDefinition(preset) { return AGENT_ARCHITECTURE_PRESETS.find(({ id }) => id === preset) || AGENT_ARCHITECTURE_PRESETS[0]; }

export function createArchitectureNode(type, position = { x: 0, y: 0 }, id = uniqueId("node"), label) {
  const normalizedType = NODE_TYPE_SET.has(type) ? type : "reason";
  const definition = nodeDefinition(normalizedType);
  const defaults = Object.fromEntries(definition.fields.map((field) => [field.key, field.defaultValue]));
  return { id, type: "architectureNode", position: normalizedPosition(position), data: { kind: normalizedType, label: label || definition.label, description: "", ...defaults } };
}

function transition(id, source, target, label = "next") { return { id, source, target, label }; }

function graphForPreset(id, preset) {
  const node = (type, x, y, suffix, label) => createArchitectureNode(type, { x, y }, `${id}-${suffix}`, label);
  if (preset === "plan-execute") {
    const nodes = [node("input", 40, 180, "input"), node("plan", 280, 180, "plan"), node("execute", 520, 80, "execute"), node("route", 760, 80, "check", "Plan complete?"), node("synthesize", 760, 280, "synthesize"), node("output", 1000, 280, "complete")];
    return { entryNodeId: nodes[0].id, nodes, edges: [transition(`${id}-e1`, nodes[0].id, nodes[1].id, "request"), transition(`${id}-e2`, nodes[1].id, nodes[2].id, "plan ready"), transition(`${id}-e3`, nodes[2].id, nodes[3].id, "step result"), transition(`${id}-e4`, nodes[3].id, nodes[2].id, "more steps"), transition(`${id}-e5`, nodes[3].id, nodes[4].id, "done"), transition(`${id}-e6`, nodes[4].id, nodes[5].id, "final")] };
  }
  if (preset === "reflection") {
    const nodes = [node("input", 40, 180, "input"), node("reason", 280, 180, "draft", "Draft"), node("critique", 520, 80, "critique"), node("route", 760, 80, "quality", "Quality gate"), node("revise", 520, 300, "revise"), node("output", 1000, 80, "complete")];
    return { entryNodeId: nodes[0].id, nodes, edges: [transition(`${id}-e1`, nodes[0].id, nodes[1].id), transition(`${id}-e2`, nodes[1].id, nodes[2].id, "draft"), transition(`${id}-e3`, nodes[2].id, nodes[3].id, "critique"), transition(`${id}-e4`, nodes[3].id, nodes[4].id, "revise"), transition(`${id}-e5`, nodes[4].id, nodes[2].id, "review again"), transition(`${id}-e6`, nodes[3].id, nodes[5].id, "accepted")] };
  }
  if (preset === "supervisor-team") {
    const nodes = [node("input", 40, 180, "input"), node("route", 280, 180, "supervisor", "Supervisor"), node("delegate", 520, 60, "researcher", "Researcher"), node("delegate", 520, 180, "implementer", "Implementer"), node("delegate", 520, 300, "verifier", "Verifier"), node("review", 760, 180, "review"), node("route", 1000, 180, "decision", "Accept result?"), node("synthesize", 1000, 360, "synthesize"), node("output", 1240, 360, "complete")];
    return { entryNodeId: nodes[0].id, nodes, edges: [transition(`${id}-e1`, nodes[0].id, nodes[1].id), transition(`${id}-e2`, nodes[1].id, nodes[2].id, "research"), transition(`${id}-e3`, nodes[1].id, nodes[3].id, "implement"), transition(`${id}-e4`, nodes[1].id, nodes[4].id, "verify"), transition(`${id}-e5`, nodes[2].id, nodes[5].id), transition(`${id}-e6`, nodes[3].id, nodes[5].id), transition(`${id}-e7`, nodes[4].id, nodes[5].id), transition(`${id}-e8`, nodes[5].id, nodes[6].id), transition(`${id}-e9`, nodes[6].id, nodes[1].id, "delegate again"), transition(`${id}-e10`, nodes[6].id, nodes[7].id, "accepted"), transition(`${id}-e11`, nodes[7].id, nodes[8].id)] };
  }
  const nodes = [node("input", 40, 180, "input"), node("observe", 280, 180, "observe"), node("reason", 520, 180, "reason"), node("route", 760, 180, "decision", "Action or answer?"), node("act", 760, 380, "act"), node("output", 1000, 80, "complete")];
  return { entryNodeId: nodes[0].id, nodes, edges: [transition(`${id}-e1`, nodes[0].id, nodes[1].id, "request"), transition(`${id}-e2`, nodes[1].id, nodes[2].id, "context"), transition(`${id}-e3`, nodes[2].id, nodes[3].id, "decision"), transition(`${id}-e4`, nodes[3].id, nodes[4].id, "tool call"), transition(`${id}-e5`, nodes[4].id, nodes[1].id, "tool result"), transition(`${id}-e6`, nodes[3].id, nodes[5].id, "final answer")] };
}

export function createAgentArchitecture(name, options = {}) {
  const preset = presetDefinition(options.preset || "react");
  const id = options.id || uniqueId("architecture");
  const graph = options.blank ? { entryNodeId: "", nodes: [], edges: [] } : graphForPreset(id, preset.id);
  return { schemaVersion: AGENT_ARCHITECTURE_SCHEMA_VERSION, kind: "agent-architecture", id, name: name || preset.label, description: preset.description, preset: preset.id, runtimeStatus: preset.runtimeStatus, maxTransitions: 1024, toolProfile: createToolProfile(options.toolProfile), ...graph };
}

export function createArchitectureLibrary() {
  const architectures = AGENT_ARCHITECTURE_PRESETS.map((preset) => createAgentArchitecture(preset.label, { preset: preset.id, id: `preset-${preset.id}` }));
  return { schemaVersion: AGENT_ARCHITECTURE_SCHEMA_VERSION, activeArchitectureId: architectures[0].id, architectures };
}

export function loadArchitectureLibrary(storage) {
  try {
    const stored = JSON.parse(storage?.getItem(AGENT_ARCHITECTURE_STORAGE_KEY));
    if (stored?.schemaVersion !== AGENT_ARCHITECTURE_SCHEMA_VERSION || !Array.isArray(stored.architectures) || !stored.architectures.length) return createArchitectureLibrary();
    const architectures = stored.architectures.map(normalizeAgentArchitecture);
    const activeArchitectureId = architectures.some(({ id }) => id === stored.activeArchitectureId) ? stored.activeArchitectureId : architectures[0].id;
    return { schemaVersion: AGENT_ARCHITECTURE_SCHEMA_VERSION, activeArchitectureId, architectures };
  } catch {
    return createArchitectureLibrary();
  }
}

function normalizeNode(value, index) {
  if (!value || typeof value !== "object") return null;
  const kind = NODE_TYPE_SET.has(value.data?.kind) ? value.data.kind : "reason";
  const normalized = createArchitectureNode(kind, value.position, text(value.id, `node-${index + 1}`));
  return { ...normalized, data: { ...normalized.data, ...serializableData(value.data), kind, label: text(value.data?.label, normalized.data.label), description: text(value.data?.description) } };
}

function normalizeEdge(value, index) {
  if (!value || typeof value !== "object") return null;
  return { id: text(value.id, `edge-${index + 1}`), source: text(value.source), target: text(value.target), label: text(value.label, "next") };
}

export function normalizeAgentArchitecture(value) {
  if (!value || typeof value !== "object") throw new Error("Architecture file must contain a JSON object.");
  const preset = presetDefinition(value.preset);
  const nodes = Array.isArray(value.nodes) ? value.nodes.map(normalizeNode).filter(Boolean) : [];
  return { schemaVersion: AGENT_ARCHITECTURE_SCHEMA_VERSION, kind: "agent-architecture", id: text(value.id, uniqueId("architecture")), name: text(value.name, "Imported agent"), description: text(value.description, preset.description), preset: preset.id, runtimeStatus: preset.runtimeStatus, maxTransitions: parsePositiveInt(value.maxTransitions, 1024), toolProfile: createToolProfile(value.toolProfile), entryNodeId: text(value.entryNodeId, nodes[0]?.id || ""), nodes, edges: Array.isArray(value.edges) ? value.edges.map(normalizeEdge).filter(Boolean) : [] };
}

export function parseArchitectureFile(contents) {
  let parsed;
  try { parsed = JSON.parse(contents); } catch { throw new Error("Architecture file is not valid JSON."); }
  if (parsed.schemaVersion !== AGENT_ARCHITECTURE_SCHEMA_VERSION || parsed.kind !== "agent-architecture") throw new Error(`Unsupported agent architecture schema: ${parsed.schemaVersion ?? "missing"}.`);
  return normalizeAgentArchitecture(parsed);
}

export function validateAgentArchitecture(architecture) {
  const errors = [];
  const warnings = [];
  const nodeIds = architecture.nodes.map(({ id }) => id);
  const idSet = new Set(nodeIds);
  if (!architecture.name.trim()) errors.push({ code: "name_required" });
  if (!architecture.nodes.length) errors.push({ code: "node_required" });
  if (idSet.size !== nodeIds.length) errors.push({ code: "duplicate_node_id" });
  if (!architecture.entryNodeId) errors.push({ code: "entry_required" });
  else if (!idSet.has(architecture.entryNodeId)) errors.push({ code: "entry_missing" });
  const dangling = architecture.edges.filter(({ source, target }) => !idSet.has(source) || !idSet.has(target));
  if (dangling.length) errors.push({ code: "dangling_edge", count: dangling.length });
  if (!architecture.nodes.some(({ data }) => data.kind === "output")) errors.push({ code: "output_required" });
  if (idSet.has(architecture.entryNodeId)) {
    const reachable = new Set([architecture.entryNodeId]);
    const queue = [architecture.entryNodeId];
    while (queue.length) {
      const source = queue.shift();
      architecture.edges.filter((edge) => edge.source === source).forEach(({ target }) => {
        if (idSet.has(target) && !reachable.has(target)) { reachable.add(target); queue.push(target); }
      });
    }
    const unreachable = architecture.nodes.length - reachable.size;
    if (unreachable) warnings.push({ code: "unreachable_nodes", count: unreachable });
  }
  return { isValid: errors.length === 0, errors, warnings };
}

export function architectureRuntimeStatus(architecture) {
  if (!validateAgentArchitecture(architecture).isValid) return "invalid";
  return "production";
}

export function architectureForExport(architecture) { return normalizeAgentArchitecture(architecture); }
