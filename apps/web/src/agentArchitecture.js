// @amadeus-header
// summary: Defines the persisted component graph used to design Amadeus agent architectures.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - const: AGENT_COMPONENT_TYPES
// - fn: createAgentArchitecture
// - fn: createArchitectureComponent
// - fn: createArchitectureLibrary
// - fn: parseArchitectureFile
// - fn: validateAgentArchitecture
// - fn: architectureForExport
// uses:
// - format: Amadeus agent architecture manifest version 1
// invariants:
// - Component graphs model AgentBuilder composition, not task execution order.
// - Every valid architecture binds exactly one model to exactly one agent runtime.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

export const AGENT_ARCHITECTURE_SCHEMA_VERSION = 1;

export const AGENT_COMPONENT_TYPES = [
  {
    type: "runtime",
    label: "Agent Runtime",
    description: "Owns the reasoning loop and assembled capabilities.",
    group: "Core",
    singleton: true,
    builder: "AgentBuilder::build",
    fields: [
      { key: "runtime", label: "Runtime", defaultValue: "react-loop", readOnly: true },
    ],
  },
  {
    type: "model",
    label: "Model Provider",
    description: "Supplies the LLM client and model configuration.",
    group: "Core",
    singleton: true,
    builder: "AgentBuilder::new(client, config)",
    fields: [
      { key: "provider", label: "Provider", defaultValue: "anthropic" },
      { key: "model", label: "Model", defaultValue: "claude-sonnet" },
      { key: "maxOutputTokens", label: "Max output tokens", defaultValue: "8192", inputMode: "numeric" },
    ],
  },
  {
    type: "prompt",
    label: "Prompt Profile",
    description: "Selects system prompt sections and project context.",
    group: "Context",
    singleton: true,
    builder: "AgentBuilder::with_prompt_profile",
    fields: [
      { key: "profile", label: "Profile", defaultValue: "default" },
      { key: "mergeMode", label: "Merge mode", defaultValue: "append" },
      { key: "projectContext", label: "Include project context", defaultValue: true, control: "checkbox" },
    ],
  },
  {
    type: "tools",
    label: "Tool Profile",
    description: "Selects tool packs, aliases, MCP, and control-plane tools.",
    group: "Capabilities",
    singleton: true,
    builder: "AgentBuilder::with_default_tools / with_tool_profile",
    fields: [
      { key: "profile", label: "Profile", defaultValue: "default" },
      { key: "includeMcp", label: "Include MCP tools", defaultValue: true, control: "checkbox" },
      { key: "includeControlPlane", label: "Include control plane", defaultValue: true, control: "checkbox" },
    ],
  },
  {
    type: "policy",
    label: "Permission Policy",
    description: "Controls approval and filesystem access boundaries.",
    group: "Controls",
    singleton: true,
    builder: "AgentBuilder::with_policy",
    fields: [
      { key: "mode", label: "Permission mode", defaultValue: "workspace-write" },
      { key: "rules", label: "Policy rules", defaultValue: "", multiline: true },
    ],
  },
  {
    type: "memory",
    label: "Memory Registry",
    description: "Injects dynamic context and exposes memory tools.",
    group: "Context",
    singleton: true,
    builder: "AgentBuilder::with_memory_registry",
    fields: [
      { key: "providers", label: "Memory providers", defaultValue: "project" },
    ],
  },
  {
    type: "rag",
    label: "RAG Search",
    description: "Adds semantic document retrieval as an agent tool.",
    group: "Context",
    singleton: true,
    builder: "AgentBuilder::with_rag",
    fields: [
      { key: "embeddingModel", label: "Embedding model", defaultValue: "" },
      { key: "topK", label: "Top K", defaultValue: "5", inputMode: "numeric" },
    ],
  },
  {
    type: "delegation",
    label: "Sub-agent Delegation",
    description: "Enables delegated sessions within a bounded depth.",
    group: "Capabilities",
    singleton: true,
    builder: "AgentBuilder::with_subagent_delegate / with_subagent_depth",
    fields: [
      { key: "maxDepth", label: "Maximum depth", defaultValue: "2", inputMode: "numeric" },
      { key: "toolProfile", label: "Sub-agent tool profile", defaultValue: "subagent" },
    ],
  },
  {
    type: "hooks",
    label: "Lifecycle Hooks",
    description: "Runs configured hooks around agent and tool events.",
    group: "Controls",
    singleton: true,
    builder: "AgentBuilder::with_hooks",
    fields: [
      { key: "enabled", label: "Enabled", defaultValue: true, control: "checkbox" },
      { key: "sandbox", label: "Hook sandbox", defaultValue: "inherit" },
      { key: "files", label: "Hook files", defaultValue: "", multiline: true },
    ],
  },
  {
    type: "compaction",
    label: "Context Compaction",
    description: "Defines when and how conversation history is compacted.",
    group: "Controls",
    singleton: true,
    builder: "AgentBuilder::with_compaction_trigger",
    fields: [
      { key: "thresholdPercent", label: "Threshold percent", defaultValue: "75", inputMode: "numeric" },
      { key: "preserveRecent", label: "Preserve recent messages", defaultValue: "6", inputMode: "numeric" },
    ],
  },
  {
    type: "telemetry",
    label: "Telemetry & Trace",
    description: "Records runtime events and optional LLM request traces.",
    group: "Observability",
    singleton: true,
    builder: "AgentBuilder::with_telemetry / with_llm_trace",
    fields: [
      { key: "telemetry", label: "Record telemetry", defaultValue: false, control: "checkbox" },
      { key: "llmTrace", label: "Record LLM trace", defaultValue: false, control: "checkbox" },
      { key: "jsonlPath", label: "JSONL path", defaultValue: "" },
    ],
  },
];

const COMPONENT_TYPE_SET = new Set(AGENT_COMPONENT_TYPES.map(({ type }) => type));
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

function serializableData(data = {}) {
  return Object.fromEntries(Object.entries(data).filter(([, value]) => (
    typeof value === "string" || typeof value === "boolean" || Number.isFinite(value)
  )));
}

export function componentDefinition(type) {
  return AGENT_COMPONENT_TYPES.find((candidate) => candidate.type === type);
}

export function createArchitectureComponent(type, position = { x: 0, y: 0 }, id = uniqueId("component")) {
  const normalizedType = COMPONENT_TYPE_SET.has(type) ? type : "prompt";
  const definition = componentDefinition(normalizedType);
  const defaults = Object.fromEntries(definition.fields.map((field) => [field.key, field.defaultValue]));
  return {
    id,
    type: "agentComponent",
    position: normalizedPosition(position),
    data: {
      kind: normalizedType,
      label: definition.label,
      description: "",
      ...defaults,
    },
  };
}

function binding(id, source, target) {
  return { id, source, target, label: "configures" };
}

export function createAgentArchitecture(name = "Untitled agent", options = {}) {
  const id = options.id || uniqueId("architecture");
  const runtime = createArchitectureComponent("runtime", { x: 640, y: 220 }, `${id}-runtime`);
  if (options.blank) {
    return {
      schemaVersion: AGENT_ARCHITECTURE_SCHEMA_VERSION,
      kind: "agent-architecture",
      id,
      name,
      description: "",
      components: [runtime],
      bindings: [],
    };
  }

  const model = createArchitectureComponent("model", { x: 80, y: 70 }, `${id}-model`);
  const prompt = createArchitectureComponent("prompt", { x: 80, y: 220 }, `${id}-prompt`);
  const tools = createArchitectureComponent("tools", { x: 350, y: 70 }, `${id}-tools`);
  const policy = createArchitectureComponent("policy", { x: 350, y: 220 }, `${id}-policy`);
  const delegation = createArchitectureComponent("delegation", { x: 350, y: 370 }, `${id}-delegation`);
  const components = [model, prompt, tools, policy, delegation, runtime];
  return {
    schemaVersion: AGENT_ARCHITECTURE_SCHEMA_VERSION,
    kind: "agent-architecture",
    id,
    name,
    description: "",
    components,
    bindings: components
      .filter(({ data }) => data.kind !== "runtime")
      .map((component, index) => binding(`${id}-binding-${index + 1}`, component.id, runtime.id)),
  };
}

export function createArchitectureLibrary() {
  const architecture = createAgentArchitecture("General purpose agent");
  return {
    schemaVersion: AGENT_ARCHITECTURE_SCHEMA_VERSION,
    activeArchitectureId: architecture.id,
    architectures: [architecture],
  };
}

function normalizeComponent(component, index) {
  if (!component || typeof component !== "object") return null;
  const kind = COMPONENT_TYPE_SET.has(component.data?.kind) ? component.data.kind : "prompt";
  const normalized = createArchitectureComponent(kind, component.position, text(component.id, `component-${index + 1}`));
  return {
    ...normalized,
    data: {
      ...normalized.data,
      ...serializableData(component.data),
      kind,
      label: text(component.data?.label, normalized.data.label),
      description: text(component.data?.description),
    },
  };
}

function normalizeBinding(value, index) {
  if (!value || typeof value !== "object") return null;
  return {
    id: text(value.id, `binding-${index + 1}`),
    source: text(value.source),
    target: text(value.target),
    label: "configures",
  };
}

export function normalizeAgentArchitecture(value) {
  if (!value || typeof value !== "object") throw new Error("Architecture file must contain a JSON object.");
  return {
    schemaVersion: AGENT_ARCHITECTURE_SCHEMA_VERSION,
    kind: "agent-architecture",
    id: text(value.id, uniqueId("architecture")),
    name: text(value.name, "Imported agent"),
    description: text(value.description),
    components: Array.isArray(value.components) ? value.components.map(normalizeComponent).filter(Boolean) : [],
    bindings: Array.isArray(value.bindings) ? value.bindings.map(normalizeBinding).filter(Boolean) : [],
  };
}

export function parseArchitectureFile(contents) {
  let parsed;
  try {
    parsed = JSON.parse(contents);
  } catch {
    throw new Error("Architecture file is not valid JSON.");
  }
  if (parsed.schemaVersion !== AGENT_ARCHITECTURE_SCHEMA_VERSION || parsed.kind !== "agent-architecture") {
    throw new Error(`Unsupported agent architecture schema: ${parsed.schemaVersion ?? "missing"}.`);
  }
  return normalizeAgentArchitecture(parsed);
}

export function validateAgentArchitecture(architecture) {
  const errors = [];
  const warnings = [];
  const componentIds = architecture.components.map(({ id }) => id);
  const idSet = new Set(componentIds);
  const componentsByKind = new Map();
  architecture.components.forEach((component) => {
    const existing = componentsByKind.get(component.data.kind) || [];
    componentsByKind.set(component.data.kind, [...existing, component]);
  });

  if (!architecture.name.trim()) errors.push({ code: "name_required" });
  if (idSet.size !== componentIds.length) errors.push({ code: "duplicate_component_id" });
  const runtimes = componentsByKind.get("runtime") || [];
  const models = componentsByKind.get("model") || [];
  if (runtimes.length !== 1) errors.push({ code: "runtime_count", count: runtimes.length });
  if (models.length !== 1) errors.push({ code: "model_count", count: models.length });

  AGENT_COMPONENT_TYPES.filter(({ singleton }) => singleton).forEach(({ type }) => {
    const count = componentsByKind.get(type)?.length || 0;
    if (count > 1) errors.push({ code: "duplicate_component_kind", kind: type, count });
  });

  const dangling = architecture.bindings.filter(({ source, target }) => !idSet.has(source) || !idSet.has(target));
  if (dangling.length) errors.push({ code: "dangling_binding", count: dangling.length });

  const componentById = new Map(architecture.components.map((component) => [component.id, component]));
  const invalid = architecture.bindings.filter(({ source, target }) => {
    const sourceKind = componentById.get(source)?.data.kind;
    const targetKind = componentById.get(target)?.data.kind;
    return sourceKind && targetKind && (sourceKind === "runtime" || targetKind !== "runtime");
  });
  if (invalid.length) errors.push({ code: "invalid_binding", count: invalid.length });

  if (runtimes.length === 1 && models.length === 1) {
    const modelBound = architecture.bindings.some(({ source, target }) => source === models[0].id && target === runtimes[0].id);
    if (!modelBound) errors.push({ code: "model_binding_required" });
  }

  const boundSources = new Set(architecture.bindings.filter(({ target }) => runtimes.some(({ id }) => id === target)).map(({ source }) => source));
  const unbound = architecture.components.filter(({ data, id }) => data.kind !== "runtime" && !boundSources.has(id));
  if (unbound.length) warnings.push({ code: "unbound_components", count: unbound.length });
  return { isValid: errors.length === 0, errors, warnings };
}

export function architectureBuilderSteps(architecture) {
  const runtime = architecture.components.find(({ data }) => data.kind === "runtime");
  if (!runtime) return [];
  const boundIds = new Set(architecture.bindings.filter(({ target }) => target === runtime.id).map(({ source }) => source));
  return architecture.components
    .filter(({ id, data }) => data.kind !== "runtime" && boundIds.has(id))
    .map(({ data }) => componentDefinition(data.kind)?.builder)
    .filter(Boolean)
    .concat("AgentBuilder::build");
}

export function architectureForExport(architecture) {
  return normalizeAgentArchitecture(architecture);
}
