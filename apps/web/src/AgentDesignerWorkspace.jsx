// @amadeus-header
// summary: Renders a visual editor for agent reasoning architectures and control transitions.
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
// - Edges represent declared workflow transitions, including loops and branches.
// - Only production-backed presets offer agent session creation.
// side_effects:
// - Imports and downloads agent architecture JSON files.
// tests:
// - cmd: npm test
// - cmd: npm run build
// @end-amadeus-header

import { useCallback, useMemo, useRef, useState } from "react";
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
  ArrowClockwise,
  ArrowRight,
  Brain,
  CheckCircle,
  DownloadSimple,
  FlowArrow,
  GitBranch,
  ListChecks,
  MagnifyingGlass,
  Plus,
  Robot,
  ShieldCheck,
  SignIn,
  SignOut,
  Sparkle,
  Trash,
  UploadSimple,
  UsersThree,
  WarningCircle,
  Wrench,
} from "@phosphor-icons/react";

import {
  AGENT_ARCHITECTURE_PRESETS,
  AGENT_NODE_TYPES,
  architectureForExport,
  architectureRuntimeStatus,
  createAgentArchitecture,
  createArchitectureNode,
  nodeDefinition,
  parseArchitectureFile,
  presetDefinition,
  validateAgentArchitecture,
} from "./agentArchitecture";

const nodeIcons = {
  input: SignIn,
  observe: MagnifyingGlass,
  reason: Brain,
  act: Wrench,
  plan: ListChecks,
  execute: Wrench,
  route: GitBranch,
  critique: ShieldCheck,
  revise: ArrowClockwise,
  delegate: UsersThree,
  review: ShieldCheck,
  approval: ShieldCheck,
  synthesize: Sparkle,
  output: SignOut,
};

const presetIcons = { react: Brain, "plan-execute": ListChecks, reflection: ArrowClockwise, "supervisor-team": UsersThree };

function ArchitectureNode({ data, selected }) {
  const Icon = nodeIcons[data.kind] || Brain;
  const isInput = data.kind === "input";
  const isOutput = data.kind === "output";
  const detail = data.instruction || data.criteria || data.condition || data.toolProfile || data.source || data.capability || data.reason || data.emptyDetail;
  return (
    <article className={`workflow-node architecture-node kind-${data.kind} ${selected ? "selected" : ""}`}>
      {!isInput && <Handle type="target" position={Position.Left} />}
      <div className="workflow-node-icon"><Icon aria-hidden="true" /></div>
      <div className="workflow-node-copy">
        <span>{data.groupLabel}</span>
        <strong>{data.label}</strong>
        <small>{detail}</small>
      </div>
      {!isOutput && <Handle type="source" position={Position.Right} />}
    </article>
  );
}

const nodeTypes = { architectureNode: ArchitectureNode };

function diagnosticText(item, t) {
  const messages = {
    name_required: t("Agent name is required."),
    node_required: t("Add at least one architecture node."),
    duplicate_node_id: t("Node identifiers must be unique."),
    entry_required: t("Choose an architecture entry node."),
    entry_missing: t("The architecture entry node no longer exists."),
    dangling_edge: t("{count} transitions reference missing nodes.", { count: item.count }),
    output_required: t("Add a Complete node to end the run."),
    unreachable_nodes: t("{count} nodes cannot be reached from the entry node.", { count: item.count }),
  };
  return messages[item.code] || item.code;
}

function AgentDesigner({ t, library, onLibraryChange, onUseArchitecture, onOpenAgentWorkspace }) {
  const [selectedNodeId, setSelectedNodeId] = useState(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState(null);
  const [importError, setImportError] = useState("");
  const fileInputRef = useRef(null);
  const { screenToFlowPosition, fitView } = useReactFlow();
  const architecture = useMemo(() => library.architectures.find(({ id }) => id === library.activeArchitectureId) || library.architectures[0], [library]);
  const validation = useMemo(() => validateAgentArchitecture(architecture), [architecture]);
  const runtimeStatus = architectureRuntimeStatus(architecture);
  const selectedNode = architecture.nodes.find(({ id }) => id === selectedNodeId) || null;
  const selectedEdge = architecture.edges.find(({ id }) => id === selectedEdgeId) || null;
  const selectedDefinition = selectedNode ? nodeDefinition(selectedNode.data.kind) : null;
  const preset = presetDefinition(architecture.preset);

  const decoratedNodes = useMemo(() => architecture.nodes.map((node) => {
    const definition = nodeDefinition(node.data.kind);
    return { ...node, data: { ...node.data, groupLabel: t(definition?.group || "Node"), emptyDetail: t("No node configuration"), isEntry: node.id === architecture.entryNodeId } };
  }), [architecture.entryNodeId, architecture.nodes, t]);

  const replaceArchitecture = useCallback((nextArchitecture) => {
    onLibraryChange({ ...library, architectures: library.architectures.map((item) => item.id === library.activeArchitectureId ? nextArchitecture : item) });
  }, [library, onLibraryChange]);

  const updateArchitecture = useCallback((updater) => replaceArchitecture(typeof updater === "function" ? updater(architecture) : updater), [architecture, replaceArchitecture]);
  const selectArchitecture = useCallback((id) => {
    onLibraryChange({ ...library, activeArchitectureId: id });
    setSelectedNodeId(null);
    setSelectedEdgeId(null);
    window.requestAnimationFrame(() => fitView({ padding: 0.2, duration: 220, maxZoom: 1 }));
  }, [fitView, library, onLibraryChange]);

  const onNodesChange = useCallback((changes) => updateArchitecture({ ...architecture, nodes: applyNodeChanges(changes, architecture.nodes) }), [architecture, updateArchitecture]);
  const onEdgesChange = useCallback((changes) => updateArchitecture({ ...architecture, edges: applyEdgeChanges(changes, architecture.edges) }), [architecture, updateArchitecture]);
  const validConnection = useCallback(({ source, target }) => source !== target && !architecture.edges.some((edge) => edge.source === source && edge.target === target), [architecture.edges]);
  const onConnect = useCallback((connection) => {
    if (!validConnection(connection)) return;
    updateArchitecture({ ...architecture, edges: addEdge({ ...connection, id: `transition-${Date.now().toString(36)}`, label: "next" }, architecture.edges) });
  }, [architecture, updateArchitecture, validConnection]);

  const addNode = useCallback((type, position) => {
    const node = createArchitectureNode(type, position || { x: 180 + architecture.nodes.length * 18, y: 120 + architecture.nodes.length * 14 });
    updateArchitecture({ ...architecture, entryNodeId: architecture.entryNodeId || node.id, nodes: [...architecture.nodes, node] });
    setSelectedNodeId(node.id);
    setSelectedEdgeId(null);
  }, [architecture, updateArchitecture]);

  const onDrop = useCallback((event) => {
    event.preventDefault();
    const type = event.dataTransfer.getData("application/amadeus-architecture-node");
    if (type) addNode(type, screenToFlowPosition({ x: event.clientX, y: event.clientY }));
  }, [addNode, screenToFlowPosition]);

  const updateNodeData = useCallback((key, value) => {
    if (!selectedNode) return;
    updateArchitecture({ ...architecture, nodes: architecture.nodes.map((node) => node.id === selectedNode.id ? { ...node, data: { ...node.data, [key]: value } } : node) });
  }, [architecture, selectedNode, updateArchitecture]);

  const removeSelection = useCallback(() => {
    updateArchitecture({
      ...architecture,
      entryNodeId: selectedNode?.id === architecture.entryNodeId ? "" : architecture.entryNodeId,
      nodes: selectedNode ? architecture.nodes.filter(({ id }) => id !== selectedNode.id) : architecture.nodes,
      edges: architecture.edges.filter((edge) => edge.id !== selectedEdge?.id && edge.source !== selectedNode?.id && edge.target !== selectedNode?.id),
    });
    setSelectedNodeId(null);
    setSelectedEdgeId(null);
  }, [architecture, selectedEdge, selectedNode, updateArchitecture]);

  const createFromPreset = useCallback((presetId = architecture.preset) => {
    const definition = presetDefinition(presetId);
    const created = createAgentArchitecture(`${definition.label} copy`, { preset: presetId });
    onLibraryChange({ ...library, activeArchitectureId: created.id, architectures: [...library.architectures, created] });
    window.requestAnimationFrame(() => fitView({ padding: 0.2, duration: 220, maxZoom: 1 }));
  }, [architecture.preset, fitView, library, onLibraryChange]);

  const deleteArchitecture = useCallback(() => {
    if (library.architectures.length <= AGENT_ARCHITECTURE_PRESETS.length) return;
    const remaining = library.architectures.filter(({ id }) => id !== architecture.id);
    onLibraryChange({ ...library, activeArchitectureId: remaining[0].id, architectures: remaining });
  }, [architecture.id, library, onLibraryChange]);

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
      onLibraryChange({ ...library, activeArchitectureId: imported.id, architectures: [...library.architectures, imported] });
      setImportError("");
    } catch (caught) {
      setImportError(caught.message.startsWith("Unsupported agent architecture schema") ? t("Unsupported agent architecture schema.") : t(caught.message));
    }
  }, [library, onLibraryChange, t]);

  return (
    <section className="workflow-workspace architecture-workspace" aria-labelledby="agent-designer-title">
      <header className="workflow-toolbar">
        <div className="workflow-title-control">
          <FlowArrow aria-hidden="true" />
          <select aria-label={t("Active agent design")} value={architecture.id} onChange={(event) => selectArchitecture(event.target.value)}>
            {library.architectures.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
          </select>
          <input id="agent-designer-title" value={architecture.name} onChange={(event) => updateArchitecture({ ...architecture, name: event.target.value })} />
          <span className={`architecture-runtime-status ${runtimeStatus}`}><i />{runtimeStatus === "production" ? t("Runnable ReAct") : runtimeStatus === "planned" ? t("Runtime planned") : t("Invalid design")}</span>
        </div>
        <div className="workflow-toolbar-actions">
          <button type="button" className="icon-button" title={t("Duplicate from this pattern")} aria-label={t("Duplicate from this pattern")} onClick={() => createFromPreset()}><Plus /></button>
          <button type="button" className="icon-button danger-hover" title={t("Delete agent design")} aria-label={t("Delete agent design")} disabled={library.architectures.length <= AGENT_ARCHITECTURE_PRESETS.length} onClick={deleteArchitecture}><Trash /></button>
          <button type="button" className="toolbar-button" onClick={() => fileInputRef.current?.click()}><UploadSimple /><span>{t("Import")}</span></button>
          <button type="button" className="toolbar-button" onClick={exportArchitecture}><DownloadSimple /><span>{t("Export")}</span></button>
          <button type="button" className="workflow-use-button" disabled={runtimeStatus !== "production"} title={runtimeStatus === "production" ? t("Create a session with the production ReAct runtime") : t("This architecture preset is not executable yet")} onClick={() => onUseArchitecture(architecture.id)}><Robot />{t("Create agent")}</button>
          <input ref={fileInputRef} type="file" accept="application/json,.json" hidden onChange={importArchitecture} />
        </div>
      </header>
      {importError && <div className="workflow-import-error" role="alert"><WarningCircle /><span>{importError}</span><button type="button" onClick={() => setImportError("")}>{t("Dismiss")}</button></div>}

      <div className="workflow-editor-grid">
        <aside className="workflow-palette architecture-palette">
          <div className="workflow-panel-heading"><div><strong>{t("Architecture patterns")}</strong><span>{t("Control algorithms, not task flows")}</span></div></div>
          <div className="architecture-pattern-list">
            {AGENT_ARCHITECTURE_PRESETS.map((item) => {
              const Icon = presetIcons[item.id];
              const design = library.architectures.find(({ preset: designPreset, id }) => designPreset === item.id && id === `preset-${item.id}`);
              return <button key={item.id} className={architecture.id === design?.id ? "active" : ""} onClick={() => design && selectArchitecture(design.id)}><Icon /><span><strong>{t(item.label)}</strong><small>{t(item.description)}</small></span><i className={item.runtimeStatus} title={item.runtimeStatus === "production" ? t("Runnable now") : t("Runtime planned")} /></button>;
            })}
          </div>
          <div className="workflow-panel-heading architecture-node-heading"><div><strong>{t("Runtime nodes")}</strong><span>{t("Drag or click to add")}</span></div></div>
          <div className="workflow-palette-list architecture-node-library">
            {AGENT_NODE_TYPES.map((definition) => {
              const Icon = nodeIcons[definition.type];
              return <button key={definition.type} type="button" draggable onDragStart={(event) => { event.dataTransfer.setData("application/amadeus-architecture-node", definition.type); event.dataTransfer.effectAllowed = "move"; }} onClick={() => addNode(definition.type)}><span className={`workflow-palette-icon kind-${definition.type}`}><Icon /></span><span><strong>{t(definition.label)}</strong><small>{t(definition.description)}</small></span><Plus /></button>;
            })}
          </div>
        </aside>

        <div className="workflow-canvas" onDrop={onDrop} onDragOver={(event) => { event.preventDefault(); event.dataTransfer.dropEffect = "move"; }}>
          <ReactFlow nodes={decoratedNodes} edges={architecture.edges} nodeTypes={nodeTypes} onNodesChange={onNodesChange} onEdgesChange={onEdgesChange} onConnect={onConnect} onNodeClick={(_, node) => { setSelectedNodeId(node.id); setSelectedEdgeId(null); }} onEdgeClick={(_, edge) => { setSelectedEdgeId(edge.id); setSelectedNodeId(null); }} onPaneClick={() => { setSelectedNodeId(null); setSelectedEdgeId(null); }} isValidConnection={validConnection} fitView fitViewOptions={{ padding: 0.16, maxZoom: 1 }} minZoom={0.2} maxZoom={1.8} defaultEdgeOptions={{ type: "smoothstep" }} deleteKeyCode={null} proOptions={{ hideAttribution: true }}>
            <Background variant={BackgroundVariant.Dots} gap={22} size={1} color="#3a3a3a" />
            <Controls showInteractive={false} />
            <MiniMap pannable zoomable nodeColor={(node) => node.id === architecture.entryNodeId ? "#ef7d32" : node.data.kind === "output" ? "#55c97a" : "#676767"} maskColor="rgba(15,15,15,.72)" />
          </ReactFlow>
          <div className={`workflow-validation-chip ${validation.isValid ? validation.warnings.length ? "warning" : "valid" : "invalid"}`}>{validation.isValid && !validation.warnings.length ? <CheckCircle /> : <WarningCircle />}<span>{validation.isValid ? validation.warnings.length ? t("Valid with {count} warnings", { count: validation.warnings.length }) : t("Architecture valid") : t("{count} validation errors", { count: validation.errors.length })}</span></div>
        </div>

        <aside className="workflow-inspector">
          {selectedNode ? <>
            <div className="workflow-panel-heading"><div><strong>{t("Node inspector")}</strong><span>{t(selectedDefinition?.description || "Architecture node")}</span></div></div>
            <div className="workflow-inspector-form">
              <label>{t("Label")}<input value={selectedNode.data.label} onChange={(event) => updateNodeData("label", event.target.value)} /></label>
              {selectedDefinition?.fields.map((field) => <label key={field.key}>{t(field.label)}{field.multiline ? <textarea rows="5" value={selectedNode.data[field.key] || ""} onChange={(event) => updateNodeData(field.key, event.target.value)} /> : <input value={selectedNode.data[field.key] || ""} onChange={(event) => updateNodeData(field.key, event.target.value)} />}</label>)}
              <label>{t("Node ID")}<input className="mono" value={selectedNode.id} readOnly /></label>
            </div>
            <div className="workflow-inspector-actions"><button className={selectedNode.id === architecture.entryNodeId ? "entry active" : "entry"} onClick={() => updateArchitecture({ ...architecture, entryNodeId: selectedNode.id })}><SignIn />{selectedNode.id === architecture.entryNodeId ? t("Entry node") : t("Set as entry")}</button><button className="danger" onClick={removeSelection}><Trash />{t("Delete node")}</button></div>
          </> : selectedEdge ? <>
            <div className="workflow-panel-heading"><div><strong>{t("Transition inspector")}</strong><span>{t("Declared workflow destination")}</span></div></div>
            <div className="workflow-inspector-form"><label>{t("Transition label")}<input value={selectedEdge.label || ""} onChange={(event) => updateArchitecture({ ...architecture, edges: architecture.edges.map((edge) => edge.id === selectedEdge.id ? { ...edge, label: event.target.value } : edge) })} /></label><label>{t("Source")}<input className="mono" value={selectedEdge.source} readOnly /></label><label>{t("Destination")}<input className="mono" value={selectedEdge.target} readOnly /></label></div>
            <div className="workflow-inspector-actions"><button className="danger" onClick={removeSelection}><Trash />{t("Delete transition")}</button></div>
          </> : <>
            <div className="workflow-panel-heading"><div><strong>{t(preset.label)}</strong><span>{t("Agent control architecture")}</span></div></div>
            <div className="architecture-runtime-summary"><div className={`architecture-runtime-status ${runtimeStatus}`}><i />{runtimeStatus === "production" ? t("Production runtime") : t("Runtime migration pending")}</div><p>{t(preset.description)}</p></div>
            <div className="workflow-overview-metrics"><div><span>{t("Nodes")}</span><strong>{architecture.nodes.length}</strong></div><div><span>{t("Transitions")}</span><strong>{architecture.edges.length}</strong></div></div>
            <label className="workflow-description-field">{t("Agent name")}<input value={architecture.name} onChange={(event) => updateArchitecture({ ...architecture, name: event.target.value })} /></label>
            <label className="workflow-description-field">{t("Description")}<textarea rows="3" value={architecture.description} onChange={(event) => updateArchitecture({ ...architecture, description: event.target.value })} /></label>
            <label className="workflow-description-field">{t("Maximum transitions")}<input inputMode="numeric" value={architecture.maxTransitions} onChange={(event) => updateArchitecture({ ...architecture, maxTransitions: Math.max(1, Number.parseInt(event.target.value, 10) || 1) })} /></label>
            <div className="workflow-diagnostics"><strong>{t("Validation")}</strong>{!validation.errors.length && !validation.warnings.length ? <div className="workflow-diagnostic valid"><CheckCircle />{t("Architecture graph is structurally valid.")}</div> : [...validation.errors, ...validation.warnings].map((item, index) => <div className={`workflow-diagnostic ${validation.errors.includes(item) ? "error" : "warning"}`} key={`${item.code}-${index}`}><WarningCircle />{diagnosticText(item, t)}</div>)}</div>
            <div className="workflow-runtime-note"><Robot /><div><strong>{runtimeStatus === "production" ? t("Use from Agent workspace") : t("Design available, execution pending")}</strong><span>{runtimeStatus === "production" ? t("Creates a session with the production ReAct loop. Visual graph edits are not applied by the backend yet.") : t("The typed runtime exists, but this preset's built-in nodes and API bridge are not implemented yet.")}</span>{runtimeStatus === "production" && <button onClick={() => onUseArchitecture(architecture.id)}>{t("Create ReAct agent")}<ArrowRight /></button>}<button className="text-only" onClick={onOpenAgentWorkspace}>{t("Open Agent workspace")}</button></div></div>
          </>}
        </aside>
      </div>
    </section>
  );
}

export function AgentDesignerWorkspace(props) {
  return <ReactFlowProvider><AgentDesigner {...props} /></ReactFlowProvider>;
}
