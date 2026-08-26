// @amadeus-header
// summary: Defines the persisted graph schema and validation rules for user-authored task workflows.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - const: WORKFLOW_NODE_TYPES
// - fn: createWorkflow
// - fn: createWorkflowNode
// - fn: createWorkflowLibrary
// - fn: parseWorkflowFile
// - fn: validateWorkflow
// uses:
// - format: Amadeus workflow JSON schema version 1
// invariants:
// - Persisted task workflow graphs contain only serializable node and edge data.
// - Imported graphs are normalized before they enter editor state.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

export const WORKFLOW_SCHEMA_VERSION = 1;

export const WORKFLOW_NODE_TYPES = [
  { type: "trigger", label: "Trigger", description: "Receives the workflow input." },
  { type: "agent", label: "Agent", description: "Runs an agent role with instructions." },
  { type: "tool", label: "Tool", description: "Invokes a configured runtime tool." },
  { type: "condition", label: "Condition", description: "Routes state using an expression." },
  { type: "approval", label: "Approval", description: "Pauses for a human decision." },
  { type: "output", label: "Output", description: "Returns the final workflow result." },
];

const NODE_TYPE_SET = new Set(WORKFLOW_NODE_TYPES.map(({ type }) => type));
let nextLocalId = 0;

function uniqueId(prefix) {
  nextLocalId += 1;
  return `${prefix}-${Date.now().toString(36)}-${nextLocalId.toString(36)}`;
}

function text(value, fallback = "") {
  return typeof value === "string" ? value : fallback;
}

function normalizedPosition(position) {
  return {
    x: Number.isFinite(position?.x) ? position.x : 0,
    y: Number.isFinite(position?.y) ? position.y : 0,
  };
}

export function createWorkflowNode(type, position = { x: 0, y: 0 }, id = uniqueId("node")) {
  const normalizedType = NODE_TYPE_SET.has(type) ? type : "agent";
  const definition = WORKFLOW_NODE_TYPES.find((candidate) => candidate.type === normalizedType);
  const defaults = {
    trigger: { input: "request" },
    agent: { role: "specialist", instructions: "" },
    tool: { tool: "" },
    condition: { expression: "state.ready == true" },
    approval: { message: "Review the current result before continuing." },
    output: { output: "state.result" },
  };
  return {
    id,
    type: "workflowNode",
    position: normalizedPosition(position),
    data: { kind: normalizedType, label: definition?.label || "Agent", ...defaults[normalizedType] },
  };
}

export function createWorkflow(name = "Untitled workflow", options = {}) {
  const id = options.id || uniqueId("workflow");
  if (options.blank) {
    const trigger = createWorkflowNode("trigger", { x: 80, y: 180 }, `${id}-trigger`);
    return {
      schemaVersion: WORKFLOW_SCHEMA_VERSION,
      id,
      name,
      description: "",
      entryNodeId: trigger.id,
      nodes: [trigger],
      edges: [],
    };
  }

  const trigger = createWorkflowNode("trigger", { x: 60, y: 90 }, `${id}-trigger`);
  const agent = createWorkflowNode("agent", { x: 340, y: 90 }, `${id}-agent`);
  const approval = createWorkflowNode("approval", { x: 340, y: 290 }, `${id}-approval`);
  const output = createWorkflowNode("output", { x: 620, y: 290 }, `${id}-output`);
  agent.data = { ...agent.data, label: "Implement task", role: "coding specialist", instructions: "Complete the request and report the result." };
  approval.data = { ...approval.data, label: "Review result" };
  return {
    schemaVersion: WORKFLOW_SCHEMA_VERSION,
    id,
    name,
    description: "",
    entryNodeId: trigger.id,
    nodes: [trigger, agent, approval, output],
    edges: [
      { id: `${id}-edge-1`, source: trigger.id, target: agent.id, label: "request" },
      { id: `${id}-edge-2`, source: agent.id, target: approval.id, label: "result" },
      { id: `${id}-edge-3`, source: approval.id, target: output.id, label: "approved" },
    ],
  };
}

export function createWorkflowLibrary() {
  const workflow = createWorkflow("Coding review");
  return { schemaVersion: WORKFLOW_SCHEMA_VERSION, activeWorkflowId: workflow.id, workflows: [workflow] };
}

function normalizeNode(node, index) {
  if (!node || typeof node !== "object") return null;
  const kind = NODE_TYPE_SET.has(node.data?.kind) ? node.data.kind : "agent";
  const normalized = createWorkflowNode(kind, node.position, text(node.id, `node-${index + 1}`));
  return {
    ...normalized,
    data: {
      ...normalized.data,
      ...Object.fromEntries(Object.entries(node.data || {}).filter(([, value]) => typeof value === "string")),
      kind,
      label: text(node.data?.label, normalized.data.label),
    },
  };
}

function normalizeEdge(edge, index) {
  if (!edge || typeof edge !== "object") return null;
  return {
    id: text(edge.id, `edge-${index + 1}`),
    source: text(edge.source),
    target: text(edge.target),
    label: text(edge.label),
  };
}

export function normalizeWorkflow(value) {
  if (!value || typeof value !== "object") throw new Error("Workflow file must contain a JSON object.");
  const nodes = Array.isArray(value.nodes) ? value.nodes.map(normalizeNode).filter(Boolean) : [];
  const edges = Array.isArray(value.edges) ? value.edges.map(normalizeEdge).filter(Boolean) : [];
  return {
    schemaVersion: WORKFLOW_SCHEMA_VERSION,
    id: text(value.id, uniqueId("workflow")),
    name: text(value.name, "Imported workflow"),
    description: text(value.description),
    entryNodeId: text(value.entryNodeId, nodes[0]?.id || ""),
    nodes,
    edges,
  };
}

export function parseWorkflowFile(contents) {
  let parsed;
  try {
    parsed = JSON.parse(contents);
  } catch {
    throw new Error("Workflow file is not valid JSON.");
  }
  if (parsed.schemaVersion !== WORKFLOW_SCHEMA_VERSION) {
    throw new Error(`Unsupported workflow schema version: ${parsed.schemaVersion ?? "missing"}.`);
  }
  return normalizeWorkflow(parsed);
}

export function validateWorkflow(workflow) {
  const errors = [];
  const warnings = [];
  const nodeIds = workflow.nodes.map((node) => node.id);
  const idSet = new Set(nodeIds);

  if (!workflow.name.trim()) errors.push({ code: "name_required" });
  if (!workflow.nodes.length) errors.push({ code: "node_required" });
  if (!workflow.entryNodeId) errors.push({ code: "entry_required" });
  else if (!idSet.has(workflow.entryNodeId)) errors.push({ code: "entry_missing" });
  if (idSet.size !== nodeIds.length) errors.push({ code: "duplicate_node" });

  const danglingEdges = workflow.edges.filter((edge) => !idSet.has(edge.source) || !idSet.has(edge.target));
  if (danglingEdges.length) errors.push({ code: "dangling_edge", count: danglingEdges.length });

  if (idSet.has(workflow.entryNodeId)) {
    const reachable = new Set([workflow.entryNodeId]);
    const queue = [workflow.entryNodeId];
    while (queue.length) {
      const source = queue.shift();
      workflow.edges.filter((edge) => edge.source === source).forEach((edge) => {
        if (idSet.has(edge.target) && !reachable.has(edge.target)) {
          reachable.add(edge.target);
          queue.push(edge.target);
        }
      });
    }
    const unreachable = workflow.nodes.length - reachable.size;
    if (unreachable) warnings.push({ code: "unreachable_nodes", count: unreachable });
  }

  const outputNodes = workflow.nodes.filter((node) => node.data.kind === "output");
  if (!outputNodes.length) warnings.push({ code: "output_recommended" });
  return { isValid: errors.length === 0, errors, warnings };
}

export function workflowForExport(workflow) {
  return normalizeWorkflow(workflow);
}
