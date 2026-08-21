// @amadeus-header
// summary: Renders the visual editor for composing agents from core runtime components.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: AgentDesignerWorkspace
// uses:
// - module: apps/web/src/agentArchitecture.js
// - library: @xyflow/react
// - library: Phosphor Icons
// invariants:
// - Canvas bindings configure an Agent Runtime and never represent task execution order.
// - Architecture edits persist locally as schema-versioned manifests.
// side_effects:
// - Reads and writes browser local storage.
// - Imports and downloads agent architecture JSON files.
// tests:
// - cmd: npm test
// - cmd: npm run build
// @end-amadeus-header

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  addEdge,
  applyEdgeChanges,
  applyNodeChanges,
  Background,
  BackgroundVariant,
  Controls,
  Handle,
  MiniMap,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import {
  ArrowsClockwise,
  BracketsCurly,
  ChatText,
  CheckCircle,
  Cpu,
  Database,
  DownloadSimple,
  FlowArrow,
  Gauge,
  GitBranch,
  MagnifyingGlass,
  Plus,
  Pulse,
  Robot,
  ShieldCheck,
  Trash,
  UploadSimple,
  WarningCircle,
  Wrench,
} from "@phosphor-icons/react";

import {
  AGENT_ARCHITECTURE_SCHEMA_VERSION,
  AGENT_COMPONENT_TYPES,
  architectureBuilderSteps,
  architectureForExport,
  componentDefinition,
  createAgentArchitecture,
  createArchitectureComponent,
  createArchitectureLibrary,
  normalizeAgentArchitecture,
  parseArchitectureFile,
  validateAgentArchitecture,
} from "./agentArchitecture";

const STORAGE_KEY = "amadeus.agentArchitectureLibrary.v1";
const componentIcons = {
  runtime: Robot,
  model: Cpu,
  prompt: ChatText,
  tools: Wrench,
  policy: ShieldCheck,
  memory: Database,
  rag: MagnifyingGlass,
  delegation: GitBranch,
  hooks: BracketsCurly,
  compaction: ArrowsClockwise,
  telemetry: Pulse,
};

function loadLibrary() {
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY));
    if (stored?.schemaVersion !== AGENT_ARCHITECTURE_SCHEMA_VERSION || !Array.isArray(stored.architectures) || !stored.architectures.length) {
      return createArchitectureLibrary();
    }
    const architectures = stored.architectures.map(normalizeAgentArchitecture);
    const activeArchitectureId = architectures.some(({ id }) => id === stored.activeArchitectureId)
      ? stored.activeArchitectureId
      : architectures[0].id;
    return { schemaVersion: AGENT_ARCHITECTURE_SCHEMA_VERSION, activeArchitectureId, architectures };
  } catch {
    return createArchitectureLibrary();
  }
}

function componentDetail(data, t) {
  if (data.kind === "model") return `${data.provider} / ${data.model}`;
  if (data.kind === "prompt" || data.kind === "tools") return data.profile;
  if (data.kind === "policy") return data.mode;
  if (data.kind === "memory") return data.providers;
  if (data.kind === "rag") return data.embeddingModel;
  if (data.kind === "delegation") return t("Depth {depth}", { depth: data.maxDepth });
  if (data.kind === "hooks") return data.enabled ? data.sandbox : t("Disabled");
  if (data.kind === "compaction") return t("{percent}% threshold", { percent: data.thresholdPercent });
  if (data.kind === "telemetry") return data.telemetry || data.llmTrace ? t("Recording enabled") : t("Disabled");
  return data.runtime;
}

function AgentComponent({ data, selected }) {
  const Icon = componentIcons[data.kind] || Robot;
  const runtime = data.kind === "runtime";
  return (
    <article className={`workflow-node architecture-component kind-${data.kind} ${selected ? "selected" : ""}`}>
      {runtime && <Handle type="target" position={Position.Left} />}
      <div className="workflow-node-icon"><Icon aria-hidden="true" /></div>
      <div className="workflow-node-copy">
        <span>{data.groupLabel}</span>
        <strong>{data.label}</strong>
        <small>{componentDetail(data, data.t) || data.emptyDetail}</small>
      </div>
      {!runtime && <Handle type="source" position={Position.Right} />}
    </article>
  );
}

const nodeTypes = { agentComponent: AgentComponent };

function diagnosticText(item, t) {
  const definition = item.kind ? componentDefinition(item.kind) : null;
  const messages = {
    name_required: t("Agent name is required."),
    duplicate_component_id: t("Component identifiers must be unique."),
    runtime_count: t("Add exactly one Agent Runtime component."),
    model_count: t("Add exactly one Model Provider component."),
    duplicate_component_kind: t("Only one {type} component can be used.", { type: t(definition?.label || item.kind) }),
    dangling_binding: t("{count} bindings reference missing components.", { count: item.count }),
    invalid_binding: t("{count} bindings do not configure the Agent Runtime.", { count: item.count }),
    model_binding_required: t("Bind the Model Provider to the Agent Runtime."),
    unbound_components: t("{count} components are not bound to the Agent Runtime.", { count: item.count }),
  };
  return messages[item.code] || item.code;
}

function AgentDesigner({ t }) {
  const [library, setLibrary] = useState(loadLibrary);
  const [selectedComponentId, setSelectedComponentId] = useState(null);
  const [selectedBindingId, setSelectedBindingId] = useState(null);
  const [saveState, setSaveState] = useState("saved");
  const [importError, setImportError] = useState("");
  const fileInputRef = useRef(null);
  const canvasRef = useRef(null);
  const { screenToFlowPosition, fitView } = useReactFlow();

  const architecture = useMemo(
    () => library.architectures.find(({ id }) => id === library.activeArchitectureId) || library.architectures[0],
    [library],
  );
  const validation = useMemo(() => validateAgentArchitecture(architecture), [architecture]);
  const selectedComponent = architecture.components.find(({ id }) => id === selectedComponentId) || null;
  const selectedBinding = architecture.bindings.find(({ id }) => id === selectedBindingId) || null;
  const selectedDefinition = selectedComponent ? componentDefinition(selectedComponent.data.kind) : null;
  const builderSteps = useMemo(() => architectureBuilderSteps(architecture), [architecture]);
  const existingKinds = useMemo(() => new Set(architecture.components.map(({ data }) => data.kind)), [architecture.components]);

  const decoratedComponents = useMemo(() => architecture.components.map((component) => {
    const definition = componentDefinition(component.data.kind);
    return {
      ...component,
      data: {
        ...component.data,
        groupLabel: t(definition?.group || "Component"),
        emptyDetail: t("Not configured"),
        t,
      },
    };
  }), [architecture.components, t]);

  useEffect(() => {
    setSaveState("saving");
    const timer = window.setTimeout(() => {
      localStorage.setItem(STORAGE_KEY, JSON.stringify({
        ...library,
        architectures: library.architectures.map(architectureForExport),
      }));
      setSaveState("saved");
    }, 180);
    return () => window.clearTimeout(timer);
  }, [library]);

  const replaceArchitecture = useCallback((nextArchitecture) => {
    setLibrary((current) => ({
      ...current,
      architectures: current.architectures.map((item) => item.id === current.activeArchitectureId ? nextArchitecture : item),
    }));
  }, []);

  const updateArchitecture = useCallback((updater) => {
    replaceArchitecture(typeof updater === "function" ? updater(architecture) : updater);
  }, [architecture, replaceArchitecture]);

  const onNodesChange = useCallback((changes) => {
    updateArchitecture({ ...architecture, components: applyNodeChanges(changes, architecture.components) });
  }, [architecture, updateArchitecture]);

  const onEdgesChange = useCallback((changes) => {
    updateArchitecture({ ...architecture, bindings: applyEdgeChanges(changes, architecture.bindings) });
  }, [architecture, updateArchitecture]);

  const validConnection = useCallback(({ source, target }) => {
    const sourceKind = architecture.components.find(({ id }) => id === source)?.data.kind;
    const targetKind = architecture.components.find(({ id }) => id === target)?.data.kind;
    return Boolean(sourceKind && targetKind === "runtime" && sourceKind !== "runtime"
      && !architecture.bindings.some((binding) => binding.source === source && binding.target === target));
  }, [architecture]);

  const onConnect = useCallback((connection) => {
    if (!validConnection(connection)) return;
    updateArchitecture({
      ...architecture,
      bindings: addEdge({ ...connection, id: `binding-${Date.now().toString(36)}`, label: "configures" }, architecture.bindings),
    });
  }, [architecture, updateArchitecture, validConnection]);

  const addComponent = useCallback((type, position) => {
    if (existingKinds.has(type)) return;
    const component = createArchitectureComponent(type, position || { x: 180, y: 120 });
    updateArchitecture({ ...architecture, components: [...architecture.components, component] });
    setSelectedComponentId(component.id);
    setSelectedBindingId(null);
    window.requestAnimationFrame(() => fitView({ padding: 0.2, duration: 220, maxZoom: 1 }));
  }, [architecture, existingKinds, fitView, updateArchitecture]);

  const onDrop = useCallback((event) => {
    event.preventDefault();
    const type = event.dataTransfer.getData("application/amadeus-agent-component");
    if (!type || existingKinds.has(type)) return;
    addComponent(type, screenToFlowPosition({ x: event.clientX, y: event.clientY }));
  }, [addComponent, existingKinds, screenToFlowPosition]);

  const updateComponentData = useCallback((key, value) => {
    if (!selectedComponent) return;
    updateArchitecture({
      ...architecture,
      components: architecture.components.map((component) => component.id === selectedComponent.id
        ? { ...component, data: { ...component.data, [key]: value } }
        : component),
    });
  }, [architecture, selectedComponent, updateArchitecture]);

  const removeSelection = useCallback(() => {
    if (selectedComponent?.data.kind === "runtime") return;
    updateArchitecture({
      ...architecture,
      components: selectedComponent ? architecture.components.filter(({ id }) => id !== selectedComponent.id) : architecture.components,
      bindings: architecture.bindings.filter((binding) => (
        binding.id !== selectedBinding?.id
        && binding.source !== selectedComponent?.id
        && binding.target !== selectedComponent?.id
      )),
    });
    setSelectedComponentId(null);
    setSelectedBindingId(null);
  }, [architecture, selectedBinding, selectedComponent, updateArchitecture]);

  const createNewArchitecture = useCallback(() => {
    const created = createAgentArchitecture(t("Untitled agent"), { blank: true });
    setLibrary((current) => ({ ...current, activeArchitectureId: created.id, architectures: [...current.architectures, created] }));
    setSelectedComponentId(null);
    setSelectedBindingId(null);
    window.requestAnimationFrame(() => fitView({ padding: 0.3, duration: 220 }));
  }, [fitView, t]);

  const deleteArchitecture = useCallback(() => {
    if (library.architectures.length <= 1) return;
    const remaining = library.architectures.filter(({ id }) => id !== architecture.id);
    setLibrary({ ...library, activeArchitectureId: remaining[0].id, architectures: remaining });
    setSelectedComponentId(null);
    setSelectedBindingId(null);
  }, [architecture.id, library]);

  const exportArchitecture = useCallback(() => {
    const contents = JSON.stringify(architectureForExport(architecture), null, 2);
    const url = URL.createObjectURL(new Blob([contents], { type: "application/json" }));
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `${architecture.name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "agent"}.amadeus-agent.json`;
    anchor.click();
    URL.revokeObjectURL(url);
  }, [architecture]);

  const importArchitecture = useCallback(async (event) => {
    const [file] = event.target.files || [];
    event.target.value = "";
    if (!file) return;
    try {
      const imported = parseArchitectureFile(await file.text());
      imported.id = `${imported.id}-${Date.now().toString(36)}`;
      setLibrary((current) => ({ ...current, activeArchitectureId: imported.id, architectures: [...current.architectures, imported] }));
      setImportError("");
      window.requestAnimationFrame(() => fitView({ padding: 0.2, duration: 220 }));
    } catch (caught) {
      setImportError(caught.message.startsWith("Unsupported agent architecture schema")
        ? t("Unsupported agent architecture schema.")
        : t(caught.message));
    }
  }, [fitView, t]);

  return (
    <section className="workflow-workspace architecture-workspace" aria-labelledby="agent-designer-title">
      <header className="workflow-toolbar">
        <div className="workflow-title-control">
          <FlowArrow aria-hidden="true" />
          <select
            aria-label={t("Active agent design")}
            value={architecture.id}
            onChange={(event) => {
              setLibrary((current) => ({ ...current, activeArchitectureId: event.target.value }));
              setSelectedComponentId(null);
              setSelectedBindingId(null);
              window.requestAnimationFrame(() => fitView({ padding: 0.2, duration: 220 }));
            }}
          >
            {library.architectures.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
          </select>
          <input id="agent-designer-title" value={architecture.name} onChange={(event) => updateArchitecture({ ...architecture, name: event.target.value })} />
          <span className={`workflow-save-state ${saveState}`}><CheckCircle />{saveState === "saved" ? t("Saved locally") : t("Saving")}</span>
        </div>
        <div className="workflow-toolbar-actions">
          <button type="button" className="icon-button" title={t("New agent design")} aria-label={t("New agent design")} onClick={createNewArchitecture}><Plus /></button>
          <button type="button" className="icon-button danger-hover" title={t("Delete agent design")} aria-label={t("Delete agent design")} disabled={library.architectures.length <= 1} onClick={deleteArchitecture}><Trash /></button>
          <button type="button" className="toolbar-button" aria-label={t("Import")} title={t("Import")} onClick={() => fileInputRef.current?.click()}><UploadSimple /><span>{t("Import")}</span></button>
          <button type="button" className="toolbar-button" aria-label={t("Export manifest")} title={t("Export manifest")} onClick={exportArchitecture}><DownloadSimple /><span>{t("Export")}</span></button>
          <input ref={fileInputRef} type="file" accept="application/json,.json" hidden onChange={importArchitecture} />
        </div>
      </header>

      {importError && <div className="workflow-import-error" role="alert"><WarningCircle /><span>{importError}</span><button type="button" onClick={() => setImportError("")}>{t("Dismiss")}</button></div>}

      <div className="workflow-editor-grid">
        <aside className="workflow-palette architecture-palette">
          <div className="workflow-panel-heading"><div><strong>{t("Component library")}</strong><span>{t("Core AgentBuilder modules")}</span></div></div>
          <div className="workflow-palette-list">
            {AGENT_COMPONENT_TYPES.filter(({ type }) => type !== "runtime").map((definition) => {
              const Icon = componentIcons[definition.type];
              const disabled = existingKinds.has(definition.type);
              return (
                <button
                  key={definition.type}
                  type="button"
                  draggable={!disabled}
                  disabled={disabled}
                  onDragStart={(event) => {
                    event.dataTransfer.setData("application/amadeus-agent-component", definition.type);
                    event.dataTransfer.effectAllowed = "move";
                  }}
                  onClick={() => addComponent(definition.type)}
                >
                  <span className={`workflow-palette-icon kind-${definition.type}`}><Icon /></span>
                  <span><strong>{t(definition.label)}</strong><small>{t(definition.description)}</small></span>
                  {disabled ? <CheckCircle aria-hidden="true" /> : <Plus aria-hidden="true" />}
                </button>
              );
            })}
          </div>
          <div className="architecture-legend"><FlowArrow /><span>{t("Bindings configure the runtime. They do not define task order.")}</span></div>
        </aside>

        <div className="workflow-canvas" ref={canvasRef} onDrop={onDrop} onDragOver={(event) => { event.preventDefault(); event.dataTransfer.dropEffect = "move"; }}>
          <ReactFlow
            nodes={decoratedComponents}
            edges={architecture.bindings}
            nodeTypes={nodeTypes}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            onNodeClick={(_, node) => { setSelectedComponentId(node.id); setSelectedBindingId(null); }}
            onEdgeClick={(_, edge) => { setSelectedBindingId(edge.id); setSelectedComponentId(null); }}
            onPaneClick={() => { setSelectedComponentId(null); setSelectedBindingId(null); }}
            isValidConnection={validConnection}
            fitView
            fitViewOptions={{ padding: 0.2, maxZoom: 1 }}
            minZoom={0.25}
            maxZoom={1.8}
            defaultEdgeOptions={{ type: "smoothstep" }}
            deleteKeyCode={null}
            proOptions={{ hideAttribution: true }}
          >
            <Background variant={BackgroundVariant.Dots} gap={22} size={1} color="#3a3a3a" />
            <Controls showInteractive={false} />
            <MiniMap pannable zoomable nodeColor={(node) => node.data.kind === "runtime" ? "#ef7d32" : "#676767"} maskColor="rgba(15,15,15,.72)" />
          </ReactFlow>
          <div className={`workflow-validation-chip ${validation.isValid ? validation.warnings.length ? "warning" : "valid" : "invalid"}`}>
            {validation.isValid && !validation.warnings.length ? <CheckCircle /> : <WarningCircle />}
            <span>{validation.isValid ? validation.warnings.length ? t("Valid with {count} warnings", { count: validation.warnings.length }) : t("Architecture valid") : t("{count} validation errors", { count: validation.errors.length })}</span>
          </div>
        </div>

        <aside className="workflow-inspector">
          {selectedComponent ? (
            <>
              <div className="workflow-panel-heading"><div><strong>{t("Component inspector")}</strong><span>{t(selectedDefinition?.builder || "AgentBuilder component")}</span></div></div>
              <div className="workflow-inspector-form">
                <label>{t("Label")}<input value={selectedComponent.data.label} onChange={(event) => updateComponentData("label", event.target.value)} /></label>
                {selectedDefinition?.fields.map((field) => (
                  <label key={field.key}>{t(field.label)}
                    {field.control === "checkbox" ? (
                      <span className="architecture-checkbox"><input type="checkbox" checked={Boolean(selectedComponent.data[field.key])} onChange={(event) => updateComponentData(field.key, event.target.checked)} /><span>{selectedComponent.data[field.key] ? t("Enabled") : t("Disabled")}</span></span>
                    ) : field.multiline ? (
                      <textarea rows="4" value={selectedComponent.data[field.key] || ""} readOnly={field.readOnly} onChange={(event) => updateComponentData(field.key, event.target.value)} />
                    ) : (
                      <input inputMode={field.inputMode} value={selectedComponent.data[field.key] || ""} readOnly={field.readOnly} onChange={(event) => updateComponentData(field.key, event.target.value)} />
                    )}
                  </label>
                ))}
                <label>{t("Component ID")}<input className="mono" value={selectedComponent.id} readOnly /></label>
              </div>
              <div className="workflow-inspector-actions"><button type="button" className="danger" disabled={selectedComponent.data.kind === "runtime"} onClick={removeSelection}><Trash />{selectedComponent.data.kind === "runtime" ? t("Runtime is required") : t("Delete component")}</button></div>
            </>
          ) : selectedBinding ? (
            <>
              <div className="workflow-panel-heading"><div><strong>{t("Binding inspector")}</strong><span>{t("Component configures runtime")}</span></div></div>
              <div className="workflow-inspector-form">
                <label>{t("Binding type")}<input value={t("configures")} readOnly /></label>
                <label>{t("Source")}<input className="mono" value={selectedBinding.source} readOnly /></label>
                <label>{t("Destination")}<input className="mono" value={selectedBinding.target} readOnly /></label>
              </div>
              <div className="workflow-inspector-actions"><button type="button" className="danger" onClick={removeSelection}><Trash />{t("Delete binding")}</button></div>
            </>
          ) : (
            <>
              <div className="workflow-panel-heading"><div><strong>{t("Agent architecture")}</strong><span>{t("AgentBuilder composition")}</span></div></div>
              <div className="workflow-overview-metrics">
                <div><span>{t("Components")}</span><strong>{architecture.components.length}</strong></div>
                <div><span>{t("Bindings")}</span><strong>{architecture.bindings.length}</strong></div>
              </div>
              <label className="workflow-description-field">{t("Agent name")}<input value={architecture.name} onChange={(event) => updateArchitecture({ ...architecture, name: event.target.value })} /></label>
              <label className="workflow-description-field">{t("Description")}<textarea rows="3" value={architecture.description} placeholder={t("Describe the agent this architecture creates.")} onChange={(event) => updateArchitecture({ ...architecture, description: event.target.value })} /></label>
              <div className="workflow-diagnostics">
                <strong>{t("Validation")}</strong>
                {!validation.errors.length && !validation.warnings.length ? (
                  <div className="workflow-diagnostic valid"><CheckCircle />{t("Architecture is ready to export.")}</div>
                ) : [...validation.errors, ...validation.warnings].map((item, index) => (
                  <div className={`workflow-diagnostic ${validation.errors.includes(item) ? "error" : "warning"}`} key={`${item.code}-${index}`}><WarningCircle />{diagnosticText(item, t)}</div>
                ))}
              </div>
              <div className="architecture-build-preview">
                <strong><Gauge />{t("Core build map")}</strong>
                <ol>{builderSteps.map((step) => <li key={step}><code>{step}</code></li>)}</ol>
              </div>
              <div className="workflow-runtime-note"><FlowArrow /><div><strong>{t("Manifest export only")}</strong><span>{t("The current API cannot apply serialized agent architectures yet.")}</span></div></div>
            </>
          )}
        </aside>
      </div>
    </section>
  );
}

export function AgentDesignerWorkspace({ t }) {
  return <ReactFlowProvider><AgentDesigner t={t} /></ReactFlowProvider>;
}
