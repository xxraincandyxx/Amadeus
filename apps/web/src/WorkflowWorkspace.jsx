// @amadeus-header
// summary: Renders the node-based editor for authoring and exporting task control-flow graphs.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: WorkflowWorkspace
// uses:
// - module: apps/web/src/workflowDefinition.js
// - library: @xyflow/react
// - library: Phosphor Icons
// invariants:
// - Task workflow edits persist locally as schema-versioned JSON.
// - Runtime execution stays unavailable until the HTTP API accepts serialized workflows.
// side_effects:
// - Reads and writes browser local storage.
// - Imports and downloads workflow JSON files.
// tests:
// - cmd: npm test
// - cmd: npm run build
// @end-amadeus-header

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ConfirmDeleteButton } from "./DeleteButton";
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
  ArrowUUpLeft,
  ArrowUUpRight,
  CheckCircle,
  DownloadSimple,
  FlowArrow,
  GitBranch,
  Lightning,
  Plus,
  Play,
  Robot,
  ShieldCheck,
  SignIn,
  SignOut,
  Trash,
  UploadSimple,
  WarningCircle,
  Wrench,
} from "@phosphor-icons/react";

import {
  createWorkflow,
  createWorkflowLibrary,
  createWorkflowNode,
  normalizeWorkflow,
  parseWorkflowFile,
  validateWorkflow,
  WORKFLOW_NODE_TYPES,
  WORKFLOW_SCHEMA_VERSION,
  workflowForExport,
} from "./workflowDefinition";

const STORAGE_KEY = "amadeus.workflowLibrary.v1";
const nodeIcons = {
  trigger: Lightning,
  agent: Robot,
  tool: Wrench,
  condition: GitBranch,
  approval: ShieldCheck,
  output: SignOut,
};

function loadLibrary() {
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY));
    if (stored?.schemaVersion !== WORKFLOW_SCHEMA_VERSION || !Array.isArray(stored.workflows) || !stored.workflows.length) {
      return createWorkflowLibrary();
    }
    const workflows = stored.workflows.map(normalizeWorkflow);
    const activeWorkflowId = workflows.some(({ id }) => id === stored.activeWorkflowId)
      ? stored.activeWorkflowId
      : workflows[0].id;
    return { schemaVersion: WORKFLOW_SCHEMA_VERSION, activeWorkflowId, workflows };
  } catch {
    return createWorkflowLibrary();
  }
}

function WorkflowNode({ data, selected }) {
  const Icon = nodeIcons[data.kind] || Robot;
  const terminal = data.kind === "output";
  const trigger = data.kind === "trigger";
  const detail = data.kind === "agent"
    ? data.role
    : data.kind === "tool"
      ? data.tool
      : data.kind === "condition"
        ? data.expression
        : data.kind === "approval"
          ? data.message
          : data.kind === "trigger"
            ? data.input
            : data.output;
  return (
    <article className={`workflow-node kind-${data.kind} ${selected ? "selected" : ""}`}>
      {!trigger && <Handle type="target" position={Position.Left} />}
      <div className="workflow-node-icon"><Icon aria-hidden="true" /></div>
      <div className="workflow-node-copy">
        <span>{data.typeLabel}</span>
        <strong>{data.label}</strong>
        <small>{detail || data.emptyDetail}</small>
      </div>
      {!terminal && <Handle type="source" position={Position.Right} />}
    </article>
  );
}

const nodeTypes = { workflowNode: WorkflowNode };

function diagnosticText(item, t) {
  const messages = {
    name_required: t("Workflow name is required."),
    node_required: t("Add at least one node."),
    entry_required: t("Choose an entry node."),
    entry_missing: t("The entry node no longer exists."),
    duplicate_node: t("Node identifiers must be unique."),
    dangling_edge: item.count === 1
      ? t("{count} connection references missing nodes.", { count: item.count })
      : t("{count} connections reference missing nodes.", { count: item.count }),
    unreachable_nodes: item.count === 1
      ? t("{count} node cannot be reached from the entry node.", { count: item.count })
      : t("{count} nodes cannot be reached from the entry node.", { count: item.count }),
    output_recommended: t("Add an Output node to return a workflow result."),
  };
  return messages[item.code] || item.code;
}

function WorkflowEditor({ t, themeColor }) {
  const [library, setLibrary] = useState(loadLibrary);
  const [selectedNodeId, setSelectedNodeId] = useState(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState(null);
  const [saveState, setSaveState] = useState("saved");
  const [importError, setImportError] = useState("");
  const fileInputRef = useRef(null);
  const canvasRef = useRef(null);
  const historyRef = useRef({ past: [], future: [] });
  const { screenToFlowPosition, fitView } = useReactFlow();

  const workflow = useMemo(
    () => library.workflows.find(({ id }) => id === library.activeWorkflowId) || library.workflows[0],
    [library],
  );
  const validation = useMemo(() => validateWorkflow(workflow), [workflow]);
  const selectedNode = workflow.nodes.find(({ id }) => id === selectedNodeId) || null;
  const selectedEdge = workflow.edges.find(({ id }) => id === selectedEdgeId) || null;

  const decoratedNodes = useMemo(() => workflow.nodes.map((node) => {
    const definition = WORKFLOW_NODE_TYPES.find(({ type }) => type === node.data.kind);
    return {
      ...node,
      data: {
        ...node.data,
        typeLabel: t(definition?.label || "Agent"),
        emptyDetail: t("Not configured"),
      },
    };
  }), [t, workflow.nodes]);

  useEffect(() => {
    setSaveState("saving");
    const timer = window.setTimeout(() => {
      const portable = {
        ...library,
        workflows: library.workflows.map(workflowForExport),
      };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(portable));
      setSaveState("saved");
    }, 300);
    return () => window.clearTimeout(timer);
  }, [library]);

  const replaceWorkflow = useCallback((nextWorkflow) => {
    setLibrary((current) => ({
      ...current,
      workflows: current.workflows.map((item) => item.id === current.activeWorkflowId ? nextWorkflow : item),
    }));
  }, []);

  const recordHistory = useCallback(() => {
    historyRef.current = {
      past: [...historyRef.current.past.slice(-39), workflowForExport(workflow)],
      future: [],
    };
  }, [workflow]);

  const restoreSnapshot = useCallback((direction) => {
    const source = historyRef.current[direction];
    if (!source.length) return;
    const snapshot = source[source.length - 1];
    const destination = direction === "past" ? "future" : "past";
    historyRef.current = {
      ...historyRef.current,
      [direction]: source.slice(0, -1),
      [destination]: [...historyRef.current[destination], workflowForExport(workflow)],
    };
    replaceWorkflow(snapshot);
    setSelectedNodeId(null);
    setSelectedEdgeId(null);
  }, [replaceWorkflow, workflow]);

  const updateWorkflow = useCallback((updater, record = true) => {
    if (record) recordHistory();
    replaceWorkflow(typeof updater === "function" ? updater(workflow) : updater);
  }, [recordHistory, replaceWorkflow, workflow]);

  const onNodesChange = useCallback((changes) => {
    if (changes.some(({ type }) => type === "remove")) recordHistory();
    replaceWorkflow({ ...workflow, nodes: applyNodeChanges(changes, workflow.nodes) });
  }, [recordHistory, replaceWorkflow, workflow]);

  const onEdgesChange = useCallback((changes) => {
    if (changes.some(({ type }) => type === "remove")) recordHistory();
    replaceWorkflow({ ...workflow, edges: applyEdgeChanges(changes, workflow.edges) });
  }, [recordHistory, replaceWorkflow, workflow]);

  const onConnect = useCallback((connection) => {
    recordHistory();
    const edge = { ...connection, id: `edge-${Date.now().toString(36)}`, label: "" };
    replaceWorkflow({ ...workflow, edges: addEdge(edge, workflow.edges) });
  }, [recordHistory, replaceWorkflow, workflow]);

  const addNode = useCallback((kind, position) => {
    recordHistory();
    const fallback = { x: 180 + workflow.nodes.length * 24, y: 120 + workflow.nodes.length * 18 };
    const node = createWorkflowNode(kind, position || fallback);
    replaceWorkflow({ ...workflow, nodes: [...workflow.nodes, node] });
    setSelectedNodeId(node.id);
    setSelectedEdgeId(null);
  }, [recordHistory, replaceWorkflow, workflow]);

  const addNodeAtCenter = useCallback((kind) => {
    const bounds = canvasRef.current?.getBoundingClientRect();
    const position = bounds
      ? screenToFlowPosition({ x: bounds.left + bounds.width / 2, y: bounds.top + bounds.height / 2 })
      : undefined;
    addNode(kind, position);
  }, [addNode, screenToFlowPosition]);

  const onDrop = useCallback((event) => {
    event.preventDefault();
    const kind = event.dataTransfer.getData("application/amadeus-workflow-node");
    if (!kind) return;
    addNode(kind, screenToFlowPosition({ x: event.clientX, y: event.clientY }));
  }, [addNode, screenToFlowPosition]);

  const updateNodeData = useCallback((key, value) => {
    if (!selectedNode) return;
    updateWorkflow({
      ...workflow,
      nodes: workflow.nodes.map((node) => node.id === selectedNode.id
        ? { ...node, data: { ...node.data, [key]: value } }
        : node),
    });
  }, [selectedNode, updateWorkflow, workflow]);

  const updateEdgeLabel = useCallback((value) => {
    if (!selectedEdge) return;
    updateWorkflow({
      ...workflow,
      edges: workflow.edges.map((edge) => edge.id === selectedEdge.id ? { ...edge, label: value } : edge),
    });
  }, [selectedEdge, updateWorkflow, workflow]);

  const removeSelection = useCallback(() => {
    if (!selectedNode && !selectedEdge) return;
    updateWorkflow({
      ...workflow,
      nodes: selectedNode ? workflow.nodes.filter(({ id }) => id !== selectedNode.id) : workflow.nodes,
      edges: workflow.edges.filter((edge) => (
        selectedEdge ? edge.id !== selectedEdge.id : edge.source !== selectedNode.id && edge.target !== selectedNode.id
      )),
      entryNodeId: selectedNode?.id === workflow.entryNodeId ? "" : workflow.entryNodeId,
    });
    setSelectedNodeId(null);
    setSelectedEdgeId(null);
  }, [selectedEdge, selectedNode, updateWorkflow, workflow]);

  const createNewWorkflow = useCallback(() => {
    const created = createWorkflow(t("Untitled workflow"), { blank: true });
    setLibrary((current) => ({ ...current, activeWorkflowId: created.id, workflows: [...current.workflows, created] }));
    historyRef.current = { past: [], future: [] };
    setSelectedNodeId(created.entryNodeId);
    setSelectedEdgeId(null);
    requestAnimationFrame(() => fitView({ duration: 250, padding: 0.3 }));
  }, [fitView, t]);

  const deleteWorkflow = useCallback(() => {
    if (library.workflows.length <= 1) return;
    const remaining = library.workflows.filter(({ id }) => id !== workflow.id);
    setLibrary({ ...library, activeWorkflowId: remaining[0].id, workflows: remaining });
    historyRef.current = { past: [], future: [] };
    setSelectedNodeId(null);
    setSelectedEdgeId(null);
  }, [library, workflow.id]);

  const exportWorkflow = useCallback(() => {
    const contents = JSON.stringify(workflowForExport(workflow), null, 2);
    const url = URL.createObjectURL(new Blob([contents], { type: "application/json" }));
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `${workflow.name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "workflow"}.amadeus-workflow.json`;
    anchor.click();
    URL.revokeObjectURL(url);
  }, [workflow]);

  const importWorkflow = useCallback(async (event) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    try {
      const imported = parseWorkflowFile(await file.text());
      imported.id = `${imported.id}-import-${Date.now().toString(36)}`;
      setLibrary((current) => ({ ...current, activeWorkflowId: imported.id, workflows: [...current.workflows, imported] }));
      setImportError("");
      setSelectedNodeId(null);
      setSelectedEdgeId(null);
      requestAnimationFrame(() => fitView({ duration: 250, padding: 0.25 }));
    } catch (caught) {
      setImportError(caught.message.startsWith("Unsupported workflow schema version")
        ? t("Unsupported workflow schema version.")
        : t(caught.message));
    }
  }, [fitView, t]);

  const inspectorField = selectedNode ? {
    trigger: { key: "input", label: t("Input key"), multiline: false },
    agent: { key: "instructions", label: t("Instructions"), multiline: true },
    tool: { key: "tool", label: t("Tool name"), multiline: false },
    condition: { key: "expression", label: t("Routing expression"), multiline: true },
    approval: { key: "message", label: t("Approval message"), multiline: true },
    output: { key: "output", label: t("Output mapping"), multiline: true },
  }[selectedNode.data.kind] : null;

  return (
    <section className="workflow-workspace" aria-labelledby="workflow-editor-title">
      <header className="workflow-toolbar">
        <div className="workflow-title-control">
          <FlowArrow aria-hidden="true" />
          <select
            aria-label={t("Active workflow")}
            value={workflow.id}
            onChange={(event) => {
              setLibrary((current) => ({ ...current, activeWorkflowId: event.target.value }));
              historyRef.current = { past: [], future: [] };
              setSelectedNodeId(null);
              setSelectedEdgeId(null);
              requestAnimationFrame(() => fitView({ duration: 250, padding: 0.25 }));
            }}
          >
            {library.workflows.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
          </select>
          <input
            id="workflow-editor-title"
            aria-label={t("Workflow name")}
            value={workflow.name}
            onChange={(event) => updateWorkflow({ ...workflow, name: event.target.value })}
          />
          <span className={`workflow-save-state ${saveState}`}><CheckCircle />{saveState === "saved" ? t("Saved locally") : t("Saving")}</span>
        </div>
        <div className="workflow-toolbar-actions">
          <button type="button" className="icon-button workflow-history-button" title={t("Undo")} aria-label={t("Undo")} disabled={!historyRef.current.past.length} onClick={() => restoreSnapshot("past")}><ArrowUUpLeft /></button>
          <button type="button" className="icon-button workflow-history-button" title={t("Redo")} aria-label={t("Redo")} disabled={!historyRef.current.future.length} onClick={() => restoreSnapshot("future")}><ArrowUUpRight /></button>
          <span className="workflow-toolbar-divider" />
          <button type="button" className="icon-button" title={t("New workflow")} aria-label={t("New workflow")} onClick={createNewWorkflow}><Plus /></button>
          <ConfirmDeleteButton className="icon-button danger-hover" title={t("Delete workflow")} confirmLabel={t("Confirm delete")} disabled={library.workflows.length <= 1} onConfirm={deleteWorkflow} />
          <button type="button" className="toolbar-button" aria-label={t("Import")} title={t("Import")} onClick={() => fileInputRef.current?.click()}><UploadSimple /><span>{t("Import")}</span></button>
          <button type="button" className="toolbar-button" aria-label={t("Export")} title={t("Export")} onClick={exportWorkflow}><DownloadSimple /><span>{t("Export")}</span></button>
          <button type="button" className="workflow-run-button" disabled title={t("Workflow execution API is not available yet.")}><Play weight="fill" />{t("Run")}</button>
          <input ref={fileInputRef} type="file" accept="application/json,.json" hidden onChange={importWorkflow} />
        </div>
      </header>

      {importError && <div className="workflow-import-error" role="alert"><WarningCircle /><span>{importError}</span><button type="button" onClick={() => setImportError("")}>{t("Dismiss")}</button></div>}

      <div className="workflow-editor-grid">
        <aside className="workflow-palette">
          <div className="workflow-panel-heading">
            <div><strong>{t("Node library")}</strong><span>{t("Drag or click to add")}</span></div>
          </div>
          <div className="workflow-palette-list">
            {WORKFLOW_NODE_TYPES.map((definition) => {
              const Icon = nodeIcons[definition.type];
              return (
                <button
                  key={definition.type}
                  type="button"
                  draggable
                  onDragStart={(event) => {
                    event.dataTransfer.setData("application/amadeus-workflow-node", definition.type);
                    event.dataTransfer.effectAllowed = "move";
                  }}
                  onClick={() => addNodeAtCenter(definition.type)}
                >
                  <span className={`workflow-palette-icon kind-${definition.type}`}><Icon /></span>
                  <span><strong>{t(definition.label)}</strong><small>{t(definition.description)}</small></span>
                  <Plus aria-hidden="true" />
                </button>
              );
            })}
          </div>
          <div className="workflow-library-actions">
            <button type="button" onClick={createNewWorkflow}><Plus />{t("New workflow")}</button>
            <ConfirmDeleteButton className="danger" label={t("Delete workflow")} confirmLabel={t("Confirm delete")} disabled={library.workflows.length <= 1} onConfirm={deleteWorkflow} />
          </div>
        </aside>

        <div
          className="workflow-canvas"
          ref={canvasRef}
          onDrop={onDrop}
          onDragOver={(event) => { event.preventDefault(); event.dataTransfer.dropEffect = "move"; }}
        >
          <ReactFlow
            nodes={decoratedNodes}
            edges={workflow.edges}
            nodeTypes={nodeTypes}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            onNodeDragStart={recordHistory}
            onNodeClick={(_, node) => { setSelectedNodeId(node.id); setSelectedEdgeId(null); }}
            onEdgeClick={(_, edge) => { setSelectedEdgeId(edge.id); setSelectedNodeId(null); }}
            onPaneClick={() => { setSelectedNodeId(null); setSelectedEdgeId(null); }}
            isValidConnection={({ source, target }) => source !== target}
            fitView
            fitViewOptions={{ padding: 0.2, maxZoom: 1 }}
            minZoom={0.25}
            maxZoom={1.8}
            defaultEdgeOptions={{ type: "smoothstep", animated: false }}
            deleteKeyCode={null}
            proOptions={{ hideAttribution: true }}
          >
            <Background variant={BackgroundVariant.Dots} gap={22} size={1} color="#3a3a3a" />
            <Controls showInteractive={false} />
            <MiniMap pannable zoomable nodeColor={(node) => node.id === workflow.entryNodeId ? themeColor : "#676767"} maskColor="rgba(15,15,15,.72)" />
          </ReactFlow>
          <div className={`workflow-validation-chip ${validation.isValid ? validation.warnings.length ? "warning" : "valid" : "invalid"}`}>
            {validation.isValid ? validation.warnings.length ? <WarningCircle /> : <CheckCircle /> : <WarningCircle />}
            <span>{validation.isValid ? validation.warnings.length === 1 ? t("Valid with {count} warning", { count: validation.warnings.length }) : validation.warnings.length ? t("Valid with {count} warnings", { count: validation.warnings.length }) : t("Workflow valid") : validation.errors.length === 1 ? t("{count} validation error", { count: validation.errors.length }) : t("{count} validation errors", { count: validation.errors.length })}</span>
          </div>
        </div>

        <aside className="workflow-inspector">
          {selectedNode ? (
            <>
              <div className="workflow-panel-heading"><div><strong>{t("Node inspector")}</strong><span>{t("{type} node", { type: t(WORKFLOW_NODE_TYPES.find(({ type }) => type === selectedNode.data.kind)?.label || "Agent") })}</span></div></div>
              <div className="workflow-inspector-form">
                <label>{t("Label")}<input value={selectedNode.data.label} onChange={(event) => updateNodeData("label", event.target.value)} /></label>
                {selectedNode.data.kind === "agent" && <label>{t("Role")}<input value={selectedNode.data.role || ""} onChange={(event) => updateNodeData("role", event.target.value)} /></label>}
                <label>{inspectorField.label}{inspectorField.multiline
                  ? <textarea rows="6" value={selectedNode.data[inspectorField.key] || ""} onChange={(event) => updateNodeData(inspectorField.key, event.target.value)} />
                  : <input value={selectedNode.data[inspectorField.key] || ""} onChange={(event) => updateNodeData(inspectorField.key, event.target.value)} />}
                </label>
                <label>{t("Node ID")}<input className="mono" value={selectedNode.id} readOnly /></label>
              </div>
              <div className="workflow-inspector-actions">
                <button type="button" className={workflow.entryNodeId === selectedNode.id ? "entry active" : "entry"} onClick={() => updateWorkflow({ ...workflow, entryNodeId: selectedNode.id })}><SignIn />{workflow.entryNodeId === selectedNode.id ? t("Entry node") : t("Set as entry")}</button>
                <button type="button" className="danger" onClick={removeSelection}><Trash />{t("Delete node")}</button>
              </div>
            </>
          ) : selectedEdge ? (
            <>
              <div className="workflow-panel-heading"><div><strong>{t("Connection inspector")}</strong><span>{t("Transition between nodes")}</span></div></div>
              <div className="workflow-inspector-form">
                <label>{t("Transition label")}<input value={selectedEdge.label || ""} onChange={(event) => updateEdgeLabel(event.target.value)} placeholder={t("Optional state or outcome")} /></label>
                <label>{t("Source")}<input className="mono" value={selectedEdge.source} readOnly /></label>
                <label>{t("Destination")}<input className="mono" value={selectedEdge.target} readOnly /></label>
              </div>
              <div className="workflow-inspector-actions"><button type="button" className="danger" onClick={removeSelection}><Trash />{t("Delete connection")}</button></div>
            </>
          ) : (
            <>
              <div className="workflow-panel-heading"><div><strong>{t("Workflow overview")}</strong><span>{t("Select a node to configure it")}</span></div></div>
              <div className="workflow-overview-metrics">
                <div><span>{t("Nodes")}</span><strong>{workflow.nodes.length}</strong></div>
                <div><span>{t("Connections")}</span><strong>{workflow.edges.length}</strong></div>
              </div>
              <label className="workflow-description-field">{t("Workflow name")}<input value={workflow.name} onChange={(event) => updateWorkflow({ ...workflow, name: event.target.value })} /></label>
              <label className="workflow-description-field">{t("Description")}<textarea rows="4" value={workflow.description} placeholder={t("Describe when this workflow should be used.")} onChange={(event) => updateWorkflow({ ...workflow, description: event.target.value })} /></label>
              <div className="workflow-diagnostics">
                <strong>{t("Validation")}</strong>
                {!validation.errors.length && !validation.warnings.length ? (
                  <div className="workflow-diagnostic valid"><CheckCircle />{t("Graph is ready to export.")}</div>
                ) : [...validation.errors, ...validation.warnings].map((item, index) => (
                  <div className={`workflow-diagnostic ${validation.errors.includes(item) ? "error" : "warning"}`} key={`${item.code}-${index}`}>
                    <WarningCircle />{diagnosticText(item, t)}
                  </div>
                ))}
              </div>
              <div className="workflow-runtime-note"><Play /><div><strong>{t("Execution bridge pending")}</strong><span>{t("You can design, validate, save, import, and export workflows. Running them requires a future serialized workflow API.")}</span></div></div>
            </>
          )}
        </aside>
      </div>
    </section>
  );
}

export function WorkflowWorkspace({ t, themeColor }) {
  return <ReactFlowProvider><WorkflowEditor t={t} themeColor={themeColor} /></ReactFlowProvider>;
}
