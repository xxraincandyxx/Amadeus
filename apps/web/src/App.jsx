import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowCounterClockwise,
  ArrowUp,
  CaretDown,
  Check,
  Code,
  Copy,
  FolderSimple,
  GearSix,
  List,
  Plus,
  Robot,
  SidebarSimple,
  Sparkle,
  Stop,
  TerminalWindow,
  Trash,
  WarningCircle,
  X,
} from "@phosphor-icons/react";

import { api } from "./api";
import { historyToTimeline, reduceEvent } from "./sessionState";

const emptyRuntime = {
  timeline: [],
  tools: {},
  streamingText: "",
  thinking: "",
  status: "idle",
  tokenUsage: null,
  approvals: [],
};

const eventNames = [
  "session_state",
  "text",
  "thinking",
  "thinking_complete",
  "tool_start",
  "tool_input",
  "tool_output",
  "tool_progress",
  "tool_done",
  "approval_request",
  "subagent_requested",
  "subagent_session",
  "token_usage",
  "compaction",
  "done",
  "error",
  "session_saved",
];

function parseData(event) {
  try {
    return JSON.parse(event.data);
  } catch {
    return { message: event.data };
  }
}

function statusLabel(status) {
  return status === "awaiting_approval" ? "Needs approval" : status?.replaceAll("_", " ") || "idle";
}

function App() {
  const [sessions, setSessions] = useState([]);
  const [activeId, setActiveId] = useState(localStorage.getItem("amadeus.activeSession"));
  const [runtimeBySession, setRuntimeBySession] = useState({});
  const [draft, setDraft] = useState("");
  const [loading, setLoading] = useState(true);
  const [serverOnline, setServerOnline] = useState(null);
  const [error, setError] = useState("");
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [creating, setCreating] = useState(false);
  const [newSessionName, setNewSessionName] = useState("");
  const [showDetails, setShowDetails] = useState(false);
  const streamRef = useRef(null);
  const endRef = useRef(null);
  const textareaRef = useRef(null);

  const activeSession = sessions.find((session) => session.id === activeId) || null;
  const runtime = runtimeBySession[activeId] || { ...emptyRuntime, status: activeSession?.status || "idle" };
  const busy = runtime.status === "running" || runtime.status === "awaiting_approval";

  const setRuntime = useCallback((sessionId, updater) => {
    setRuntimeBySession((current) => {
      const previous = current[sessionId] || { ...emptyRuntime };
      return { ...current, [sessionId]: typeof updater === "function" ? updater(previous) : updater };
    });
  }, []);

  const refreshSessions = useCallback(async () => {
    const data = await api.listSessions();
    const available = data.sessions.filter((session) => session.status !== "closed");
    setSessions(available);
    setActiveId((current) => {
      const exists = available.some((session) => session.id === current);
      return exists ? current : data.active_session_id || available[0]?.id || null;
    });
    return available;
  }, []);

  const loadHistory = useCallback(async (sessionId) => {
    if (!sessionId) return;
    const data = await api.getHistory(sessionId);
    const approvals = await api.approvals(sessionId);
    setRuntime(sessionId, (previous) => ({
      ...previous,
      timeline: historyToTimeline(data.messages),
      approvals,
    }));
  }, [setRuntime]);

  useEffect(() => {
    let cancelled = false;
    async function bootstrap() {
      try {
        await api.health();
        if (cancelled) return;
        setServerOnline(true);
        const available = await refreshSessions();
        if (!available.length) {
          const created = await api.createSession("Main Agent", "default");
          setSessions([created]);
          setActiveId(created.id);
        }
      } catch (caught) {
        if (!cancelled) {
          setServerOnline(false);
          setError(caught.message);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    bootstrap();
    return () => { cancelled = true; };
  }, [refreshSessions]);

  useEffect(() => {
    if (!activeId) return;
    localStorage.setItem("amadeus.activeSession", activeId);
    loadHistory(activeId).catch((caught) => setError(caught.message));
    setSidebarOpen(false);
  }, [activeId, loadHistory]);

  useEffect(() => {
    streamRef.current?.close();
    if (!activeId || !serverOnline) return undefined;

    const source = new EventSource(api.eventUrl(activeId));
    streamRef.current = source;
    eventNames.forEach((eventName) => {
      source.addEventListener(eventName, (event) => {
        const payload = parseData(event);
        setRuntime(activeId, (previous) => reduceEvent(previous, eventName, payload));
        if (eventName === "session_state") {
          setSessions((current) => current.map((session) => session.id === payload.id ? payload : session));
        }
        if (eventName === "done" || eventName === "error") {
          loadHistory(activeId).catch(() => undefined);
        }
      });
    });
    source.onopen = () => setError("");
    source.onerror = () => {
      setError("Live connection interrupted. Amadeus will retry automatically.");
      loadHistory(activeId).catch(() => undefined);
    };

    return () => source.close();
  }, [activeId, loadHistory, serverOnline, setRuntime]);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [runtime.timeline, runtime.streamingText, runtime.thinking, runtime.approvals]);

  const createSession = async (event) => {
    event?.preventDefault();
    try {
      const name = newSessionName.trim() || `Session ${sessions.length + 1}`;
      const session = await api.createSession(name, "default");
      setSessions((current) => [...current, session]);
      setActiveId(session.id);
      setNewSessionName("");
      setCreating(false);
      requestAnimationFrame(() => textareaRef.current?.focus());
    } catch (caught) {
      setError(caught.message);
    }
  };

  const submit = async () => {
    const content = draft.trim();
    if (!content || !activeId || busy) return;
    setDraft("");
    setRuntime(activeId, (previous) => ({
      ...previous,
      status: "running",
      timeline: [...previous.timeline, { id: `user-${Date.now()}`, kind: "user", text: content }],
      streamingText: "",
      thinking: "",
    }));
    try {
      await api.submitMessage(activeId, content);
    } catch (caught) {
      setError(caught.message);
      setRuntime(activeId, (previous) => ({ ...previous, status: "failed" }));
    }
  };

  const cancel = async () => {
    try {
      await api.cancel(activeId);
      setRuntime(activeId, (previous) => ({ ...previous, status: "idle", streamingText: "", thinking: "", approvals: [] }));
    } catch (caught) {
      setError(caught.message);
    }
  };

  const closeSession = async () => {
    if (!activeId) return;
    try {
      await api.close(activeId);
      const remaining = sessions.filter((session) => session.id !== activeId);
      setSessions(remaining);
      setActiveId(remaining[0]?.id || null);
      if (!remaining.length) setCreating(true);
    } catch (caught) {
      setError(caught.message);
    }
  };

  const decideApproval = async (approvalId, decision) => {
    try {
      await api.approve(activeId, approvalId, decision);
      setRuntime(activeId, (previous) => ({
        ...previous,
        approvals: previous.approvals.filter((item) => item.id !== approvalId),
        status: "running",
      }));
    } catch (caught) {
      setError(caught.message);
    }
  };

  const visibleTools = useMemo(
    () => Object.values(runtime.tools).filter((tool) => tool.status === "running"),
    [runtime.tools],
  );

  if (loading) return <LoadingScreen />;

  return (
    <div className="app-shell">
      <Sidebar
        sessions={sessions}
        activeId={activeId}
        open={sidebarOpen}
        online={serverOnline}
        onSelect={setActiveId}
        onCreate={() => setCreating(true)}
        onClose={() => setSidebarOpen(false)}
      />

      <main className="workspace">
        <Header
          session={activeSession}
          status={runtime.status}
          onMenu={() => setSidebarOpen(true)}
          onDetails={() => setShowDetails((value) => !value)}
          onClose={closeSession}
        />

        {error && <ErrorBanner message={error} onDismiss={() => setError("")} />}

        <section className="conversation" aria-live="polite">
          {!activeSession ? (
            <EmptyState onCreate={() => setCreating(true)} online={serverOnline} />
          ) : (
            <div className="conversation-column">
              {!runtime.timeline.length && !runtime.streamingText && (
                <Welcome session={activeSession} />
              )}
              {runtime.timeline.map((item) => <TimelineItem key={item.id} item={item} />)}
              {runtime.thinking && <ThinkingBlock text={runtime.thinking} />}
              {visibleTools.map((tool) => <ToolCard key={tool.id} tool={tool} live />)}
              {runtime.streamingText && <AssistantMessage text={runtime.streamingText} streaming />}
              {runtime.approvals.map((approval) => (
                <ApprovalCard key={approval.id} approval={approval} onDecision={decideApproval} />
              ))}
              {busy && !runtime.streamingText && !runtime.thinking && !visibleTools.length && !runtime.approvals.length && (
                <AgentWorking />
              )}
              <div ref={endRef} />
            </div>
          )}
        </section>

        <Composer
          draft={draft}
          disabled={!activeSession || !serverOnline}
          busy={busy}
          tokenUsage={runtime.tokenUsage}
          onChange={setDraft}
          onSubmit={submit}
          onCancel={cancel}
          textareaRef={textareaRef}
        />
      </main>

      {showDetails && activeSession && (
        <DetailsPanel session={activeSession} runtime={runtime} onClose={() => setShowDetails(false)} />
      )}

      {creating && (
        <CreateDialog
          value={newSessionName}
          onChange={setNewSessionName}
          onSubmit={createSession}
          onClose={() => setCreating(false)}
        />
      )}
    </div>
  );
}

function Sidebar({ sessions, activeId, open, online, onSelect, onCreate, onClose }) {
  return (
    <>
      <button className={`sidebar-scrim ${open ? "visible" : ""}`} aria-label="Close sidebar" onClick={onClose} />
      <aside className={`sidebar ${open ? "open" : ""}`}>
        <div className="traffic-lights" aria-hidden="true"><i /><i /><i /></div>
        <div className="brand-row"><div className="brand-mark"><Sparkle weight="fill" /></div><strong>Amadeus</strong></div>
        <nav className="primary-nav" aria-label="Primary">
          <button onClick={onCreate}><Plus /><span>New session</span></button>
          <button><Robot /><span>Agents</span><span className="nav-count">{sessions.length}</span></button>
          <button><TerminalWindow /><span>Tools</span></button>
        </nav>
        <div className="section-label">Workspace</div>
        <div className="project-label"><FolderSimple /><span>amadeus</span></div>
        <div className="session-list">
          {sessions.map((session) => (
            <button
              key={session.id}
              className={`session-button ${session.id === activeId ? "active" : ""}`}
              onClick={() => onSelect(session.id)}
            >
              <span>{session.name}</span>
              <i className={`status-dot ${session.status}`} title={statusLabel(session.status)} />
            </button>
          ))}
        </div>
        <div className="sidebar-footer">
          <div className="connection"><i className={online ? "online" : "offline"} /><span>{online ? "Local API connected" : "API unavailable"}</span></div>
          <button aria-label="Settings"><GearSix /></button>
        </div>
      </aside>
    </>
  );
}

function Header({ session, status, onMenu, onDetails, onClose }) {
  return (
    <header className="topbar">
      <button className="mobile-menu" onClick={onMenu} aria-label="Open sidebar"><SidebarSimple /></button>
      <div className="header-title">
        <FolderSimple />
        <div><strong>{session?.name || "Amadeus"}</strong><span>{session ? session.profile : "agent workspace"}</span></div>
      </div>
      <div className="header-actions">
        {session && <span className={`status-badge ${status}`}>{statusLabel(status)}</span>}
        <button className="toolbar-button" onClick={onDetails}><List /><span>Details</span></button>
        {session && <button className="icon-button danger-hover" onClick={onClose} aria-label="Close session"><Trash /></button>}
      </div>
    </header>
  );
}

function TimelineItem({ item }) {
  if (item.kind === "tool") return <ToolCard tool={item} />;
  if (item.kind === "assistant") return <AssistantMessage text={item.text} />;
  if (item.kind === "user") return <UserMessage text={item.text} />;
  if (item.kind === "error") return <div className="inline-notice error"><WarningCircle />{item.text}</div>;
  return <div className="inline-notice"><ArrowCounterClockwise />{item.text}</div>;
}

function UserMessage({ text }) {
  return <article className="message user-message"><div className="message-label">You</div><p>{text}</p></article>;
}

function AssistantMessage({ text, streaming = false }) {
  return (
    <article className="message assistant-message">
      <div className="assistant-rail"><div className="assistant-mark"><Sparkle weight="fill" /></div></div>
      <div className="message-body"><div className="message-label">Amadeus</div><p>{text}<span className={streaming ? "stream-caret" : ""} /></p></div>
    </article>
  );
}

function ThinkingBlock({ text }) {
  return <details className="thinking-block" open><summary><Sparkle />Reasoning<CaretDown /></summary><p>{text}</p></details>;
}

function ToolCard({ tool, live = false }) {
  const [expanded, setExpanded] = useState(live);
  const detail = tool.command || tool.inputText || (tool.input ? JSON.stringify(tool.input, null, 2) : "");
  return (
    <article className={`tool-card ${tool.status || "complete"}`}>
      <button className="tool-summary" onClick={() => setExpanded((value) => !value)}>
        <span className="tool-icon"><Code /></span>
        <span className="tool-heading"><strong>{tool.name || "Tool"}</strong><small>{tool.progress || (live ? "Running" : tool.is_error ? "Failed" : "Completed")}</small></span>
        {typeof tool.percent === "number" && <span className="tool-percent">{tool.percent}%</span>}
        <CaretDown className={expanded ? "rotated" : ""} />
      </button>
      {expanded && (
        <div className="tool-content">
          {detail && <CodeBlock label="input" text={detail} />}
          {tool.output && <CodeBlock label="output" text={tool.output} />}
        </div>
      )}
    </article>
  );
}

function CodeBlock({ label, text }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1200);
  };
  return (
    <div className="code-block"><div className="code-header"><span>{label}</span><button onClick={copy}>{copied ? <Check /> : <Copy />}</button></div><pre>{text}</pre></div>
  );
}

function ApprovalCard({ approval, onDecision }) {
  return (
    <article className="approval-card">
      <div className="approval-icon"><WarningCircle weight="fill" /></div>
      <div className="approval-copy"><span>Permission required</span><strong>{approval.tool}</strong><p>{approval.action || approval.reason || "This tool needs your approval before it can continue."}</p><CodeBlock label="input" text={JSON.stringify(approval.input, null, 2)} /></div>
      <div className="approval-actions"><button onClick={() => onDecision(approval.id, "deny")}>Deny</button><button onClick={() => onDecision(approval.id, "always_approve")}>Always allow</button><button className="primary" onClick={() => onDecision(approval.id, "approve")}>Allow once</button></div>
    </article>
  );
}

function Composer({ draft, disabled, busy, tokenUsage, onChange, onSubmit, onCancel, textareaRef }) {
  const onKeyDown = (event) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      onSubmit();
    }
  };
  return (
    <div className="composer-wrap">
      <div className={`composer ${disabled ? "disabled" : ""}`}>
        <label htmlFor="agent-prompt">Message Amadeus</label>
        <textarea
          ref={textareaRef}
          id="agent-prompt"
          rows="2"
          value={draft}
          placeholder={disabled ? "Connect to the local Amadeus API to begin" : "Ask Amadeus to inspect, explain, or build anything"}
          disabled={disabled || busy}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={onKeyDown}
        />
        <div className="composer-footer">
          <div className="composer-meta"><button aria-label="Add context"><Plus /></button><span className="access-label"><WarningCircle />Local full access</span></div>
          <div className="composer-meta right">
            {tokenUsage && <span>{tokenUsage.context_percent}% context</span>}
            <span>Default agent</span>
            {busy ? <button className="send-button stop" onClick={onCancel} aria-label="Stop generation"><Stop weight="fill" /></button> : <button className="send-button" onClick={onSubmit} disabled={!draft.trim() || disabled} aria-label="Send message"><ArrowUp weight="bold" /></button>}
          </div>
        </div>
      </div>
      <div className="composer-hint">Enter to send · Shift + Enter for a new line</div>
    </div>
  );
}

function Welcome({ session }) {
  return (
    <div className="welcome">
      <div className="welcome-mark"><Sparkle weight="fill" /></div>
      <h1>What should we work on?</h1>
      <p>{session.name} can inspect your project, execute tools, and keep the entire conversation in this session.</p>
      <div className="starter-grid">
        <button onClick={() => document.getElementById("agent-prompt")?.focus()}><Code /><span><strong>Explore the codebase</strong><small>Map architecture and important flows</small></span></button>
        <button onClick={() => document.getElementById("agent-prompt")?.focus()}><TerminalWindow /><span><strong>Build a feature</strong><small>Plan, implement, test, and verify</small></span></button>
      </div>
    </div>
  );
}

function DetailsPanel({ session, runtime, onClose }) {
  return (
    <aside className="details-panel">
      <div className="details-header"><strong>Session details</strong><button onClick={onClose}><X /></button></div>
      <dl><div><dt>Status</dt><dd>{statusLabel(runtime.status)}</dd></div><div><dt>Profile</dt><dd>{session.profile}</dd></div><div><dt>Session ID</dt><dd className="mono">{session.id}</dd></div><div><dt>Messages</dt><dd>{runtime.timeline.filter((item) => item.kind === "user" || item.kind === "assistant").length}</dd></div><div><dt>Tool calls</dt><dd>{runtime.timeline.filter((item) => item.kind === "tool").length}</dd></div>{runtime.tokenUsage && <><div><dt>Input tokens</dt><dd>{runtime.tokenUsage.input_tokens.toLocaleString()}</dd></div><div><dt>Output tokens</dt><dd>{runtime.tokenUsage.output_tokens.toLocaleString()}</dd></div></>}</dl>
    </aside>
  );
}

function CreateDialog({ value, onChange, onSubmit, onClose }) {
  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={onClose}>
      <form className="dialog" onSubmit={onSubmit} onMouseDown={(event) => event.stopPropagation()}>
        <div className="dialog-icon"><Sparkle weight="fill" /></div><h2>New session</h2><p>Start with a clean conversation and an independent agent context.</p>
        <label htmlFor="session-name">Session name</label><input id="session-name" autoFocus value={value} onChange={(event) => onChange(event.target.value)} placeholder="Feature implementation" />
        <div className="dialog-actions"><button type="button" onClick={onClose}>Cancel</button><button className="primary" type="submit">Create session</button></div>
      </form>
    </div>
  );
}

function ErrorBanner({ message, onDismiss }) {
  return <div className="error-banner"><WarningCircle /><span>{message}</span><button onClick={onDismiss}><X /></button></div>;
}

function AgentWorking() {
  return <div className="agent-working"><span /><span /><span /><em>Amadeus is working</em></div>;
}

function EmptyState({ onCreate, online }) {
  return <div className="empty-state"><Robot /><h1>{online ? "No open sessions" : "Amadeus API is offline"}</h1><p>{online ? "Create a session to begin working with an agent." : `Start the server at ${api.baseUrl}, then refresh this page.`}</p>{online && <button onClick={onCreate}><Plus />New session</button>}</div>;
}

function LoadingScreen() {
  return <div className="loading-screen"><div className="loading-mark"><Sparkle weight="fill" /></div><div className="loading-line" /><div className="loading-line short" /></div>;
}

export default App;
