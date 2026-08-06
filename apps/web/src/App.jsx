// @amadeus-header
// summary: Renders and coordinates the Amadeus web and native agent workspace.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: App
// - runtime: React agent workspace
// uses:
// - module: apps/web/src/api.js
// - module: apps/web/src/i18n.js
// - module: apps/web/src/sessionState.js
// - protocol: Amadeus REST and SSE APIs
// invariants:
// - Live reasoning is visually distinct from final assistant output.
// - Reasoning disclosures remain keyboard accessible, user-controlled, and collapsed by default.
// - Slash commands advertised by the composer execute without model involvement.
// - Interface language selection persists across web and native client launches.
// side_effects:
// - Reads and writes browser local storage.
// - Opens REST, SSE, and external-link connections.
// tests:
// - cmd: npm test
// - cmd: npm run build
// @end-amadeus-header

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowCounterClockwise,
  ArrowsInLineVertical,
  ArrowSquareOut,
  ArrowUp,
  BookOpenText,
  Brain,
  BracketsCurly,
  CaretDown,
  Check,
  Code,
  Copy,
  DownloadSimple,
  FileText,
  FolderSimple,
  GearSix,
  GithubLogo,
  Gauge,
  List,
  Plus,
  PlugsConnected,
  Robot,
  SidebarSimple,
  Sparkle,
  Stop,
  StopCircle,
  TerminalWindow,
  Trash,
  UserPlus,
  WarningCircle,
  Wrench,
  X,
  XCircle,
} from "@phosphor-icons/react";

import { api, getApiBaseUrl, resetApiBaseUrl, setApiBaseUrl } from "./api";
import { normalizeLanguage, SUPPORTED_LANGUAGES, translate } from "./i18n";
import { MarkdownContent } from "./MarkdownContent";
import { historyToTimeline, preserveThinkingTimeline, reduceEvent } from "./sessionState";
import { commandDraft, filterSlashCommands, parseSlashInput, SLASH_COMMANDS } from "./slashCommands";

const emptyRuntime = {
  timeline: [],
  tools: {},
  streamingText: "",
  thinking: "",
  rawStreamingText: "",
  providerThinking: "",
  thinkingStartedAt: null,
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

const TranslationContext = createContext((key, variables) => translate("en", key, variables));

function useTranslation() {
  return useContext(TranslationContext);
}

function parseData(event) {
  try {
    return JSON.parse(event.data);
  } catch {
    return { message: event.data };
  }
}

function statusLabel(status, t = translate.bind(null, "en")) {
  if (status === "awaiting_approval") return t("Needs approval");
  return t(status?.replaceAll("_", " ") || "idle");
}

function exportConversation(session, timeline, format) {
  const normalizedFormat = format === "json" ? "json" : "markdown";
  const safeName = (session?.name || "amadeus-session").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
  let content;
  let type;
  let extension;

  if (normalizedFormat === "json") {
    content = JSON.stringify({ session, timeline }, null, 2);
    type = "application/json";
    extension = "json";
  } else {
    content = timeline.map((item) => {
      if (item.kind === "user") return `## You\n\n${item.text}`;
      if (item.kind === "assistant") return `## Amadeus\n\n${item.text}`;
      if (item.kind === "thinking") return `> Reasoning: ${item.text}`;
      if (item.kind === "tool") return `### Tool: ${item.name || "Tool"}\n\n\`\`\`text\n${item.output || item.inputText || ""}\n\`\`\``;
      if (item.kind === "command") return `### ${item.title}\n\n${item.text}`;
      return item.text || "";
    }).filter(Boolean).join("\n\n");
    type = "text/markdown";
    extension = "md";
  }

  const url = URL.createObjectURL(new Blob([content], { type }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `${safeName || "amadeus-session"}.${extension}`;
  anchor.click();
  URL.revokeObjectURL(url);
  return extension;
}

function App() {
  const [language, setLanguage] = useState(() => normalizeLanguage(localStorage.getItem("amadeus.language") || navigator.language));
  const [sessions, setSessions] = useState([]);
  const [activeId, setActiveId] = useState(localStorage.getItem("amadeus.activeSession"));
  const [runtimeBySession, setRuntimeBySession] = useState({});
  const [draft, setDraft] = useState("");
  const [loading, setLoading] = useState(true);
  const [serverOnline, setServerOnline] = useState(null);
  const [error, setError] = useState("");
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [creating, setCreating] = useState(false);
  const [creatingSession, setCreatingSession] = useState(false);
  const [createError, setCreateError] = useState("");
  const [newSessionName, setNewSessionName] = useState("");
  const [showDetails, setShowDetails] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showContribute, setShowContribute] = useState(false);
  const [apiEpoch, setApiEpoch] = useState(0);
  const streamRef = useRef(null);
  const endRef = useRef(null);
  const textareaRef = useRef(null);
  const t = useCallback((key, variables) => translate(language, key, variables), [language]);

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
      timeline: preserveThinkingTimeline(historyToTimeline(data.messages), previous.timeline),
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
          const created = await api.createSession(t("Main Agent"), "default");
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
  }, [refreshSessions, apiEpoch, t]);

  useEffect(() => {
    if (!activeId) return;
    localStorage.setItem("amadeus.activeSession", activeId);
    loadHistory(activeId).catch((caught) => setError(caught.message));
    setSidebarOpen(false);
  }, [activeId, apiEpoch, loadHistory]);

  useEffect(() => {
    streamRef.current?.close();
    if (!activeId || !serverOnline) return undefined;

    const source = new EventSource(api.eventUrl(activeId));
    streamRef.current = source;
    eventNames.forEach((eventName) => {
      source.addEventListener(eventName, (event) => {
        if (eventName === "error" && typeof event.data !== "string") return;
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
    source.onopen = () => {
      setServerOnline(true);
      setError("");
    };
    source.onerror = (event) => {
      if (typeof event.data === "string") return;
      setError(t("Live connection interrupted. Amadeus will retry automatically."));
      api.health()
        .then(() => setServerOnline(true))
        .catch(() => {
          setServerOnline(false);
          setError(t("Amadeus API is unavailable. Start the server, then retry."));
        });
      loadHistory(activeId).catch(() => undefined);
    };

    return () => source.close();
  }, [activeId, apiEpoch, loadHistory, serverOnline, setRuntime, t]);

  useEffect(() => {
    if (window.__TAURI_INTERNALS__) document.documentElement.classList.add("is-tauri");
  }, []);

  useEffect(() => {
    localStorage.setItem("amadeus.language", language);
    document.documentElement.lang = language;
  }, [language]);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [runtime.timeline, runtime.streamingText, runtime.thinking, runtime.approvals]);

  const openCreateDialog = useCallback(() => {
    setNewSessionName("");
    setCreateError("");
    setCreating(true);
  }, []);

  const createSession = async (event) => {
    event?.preventDefault();
    if (!serverOnline || creatingSession) {
      setCreateError(t("Connect to the Amadeus API before creating a session."));
      return;
    }
    setCreatingSession(true);
    setCreateError("");
    try {
      const name = newSessionName.trim() || t("Session {number}", { number: sessions.length + 1 });
      const session = await api.createSession(name, "default");
      setSessions((current) => [...current, session]);
      setActiveId(session.id);
      setNewSessionName("");
      setCreating(false);
      requestAnimationFrame(() => textareaRef.current?.focus());
    } catch (caught) {
      setCreateError(caught.message);
    } finally {
      setCreatingSession(false);
    }
  };

  const cancel = async () => {
    try {
      await api.cancel(activeId);
      setRuntime(activeId, (previous) => ({
        ...previous,
        status: "idle",
        streamingText: "",
        thinking: "",
        rawStreamingText: "",
        providerThinking: "",
        thinkingStartedAt: null,
        approvals: [],
      }));
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
      if (!remaining.length) openCreateDialog();
    } catch (caught) {
      setError(caught.message);
    }
  };

  const addCommandResult = (title, text, tone = "default") => {
    if (!activeId) return;
    setRuntime(activeId, (previous) => ({
      ...previous,
      timeline: [...previous.timeline, { id: `command-${Date.now()}`, kind: "command", title, text, tone }],
    }));
  };

  const executeSlashCommand = async (input) => {
    const parsed = parseSlashInput(input);
    if (!parsed) return false;
    const command = SLASH_COMMANDS.find((candidate) => candidate.name === parsed.name);
    if (!command) {
      addCommandResult("Unknown command", `No command named \`/${parsed.name || ""}\` is available. Type \`/\` to browse commands.`, "error");
      return true;
    }

    try {
      if (command.name === "help") {
        const lines = SLASH_COMMANDS.map((item) => `- \`/${item.name}${item.argumentHint ? ` ${item.argumentHint}` : ""}\`: ${item.summary}`);
        addCommandResult("Slash commands", lines.join("\n"));
      }
      if (command.name === "new-agent") {
        const name = parsed.argument || t("Session {number}", { number: sessions.length + 1 });
        const session = await api.createSession(name, "default");
        setSessions((current) => [...current, session]);
        setActiveId(session.id);
      }
      if (command.name === "context") {
        const usage = runtime.tokenUsage;
        const lines = [
          `- Status: **${statusLabel(runtime.status, t)}**`,
          `- Timeline items: **${runtime.timeline.length}**`,
          `- Pending approvals: **${runtime.approvals.length}**`,
          usage ? `- Input tokens: **${usage.input_tokens.toLocaleString()}**` : "- Input tokens: not reported yet",
          usage ? `- Output tokens: **${usage.output_tokens.toLocaleString()}**` : "- Output tokens: not reported yet",
          usage ? `- Context used: **${usage.context_percent}%**` : "- Context used: not reported yet",
        ];
        addCommandResult("Session context", lines.join("\n"));
      }
      if (command.name === "compact") {
        const result = await api.compact(activeId);
        await loadHistory(activeId);
        const title = result.status === "compressed" ? "Context compacted" : "Compaction complete";
        const lines = [
          `- Status: **${result.status.replaceAll("_", " ")}**`,
          `- Messages: **${result.original_count} → ${result.compacted_count}**`,
          `- Messages summarized: **${result.messages_summarized}**`,
          `- Estimated tokens: **${result.original_tokens.toLocaleString()} → ${result.new_tokens.toLocaleString()}**`,
          `- Estimated tokens recovered: **${result.tokens_saved.toLocaleString()}**`,
        ];
        addCommandResult(title, lines.join("\n"));
      }
      if (command.name === "tools") {
        const data = await api.getToolCatalog();
        const rows = (data.tools || []).map((tool) => `| \`${tool.name}\` | ${tool.level} | ${tool.permission_mode} |`);
        addCommandResult("Active tool catalog", ["| Tool | Level | Permission |", "| --- | --- | --- |", ...rows].join("\n"));
      }
      if (command.name === "prompt") {
        const config = await api.getConfig();
        addCommandResult("Active prompt", [
          `- Model: \`${config.model}\``,
          `- Prompt profile: \`${config.prompt?.active_profile || "default"}\``,
          `- Prompt sections: **${config.prompt?.section_count ?? 0}**`,
          `- Tool profile: \`${config.tools?.active_profile || "default"}\``,
          `- Context window: **${config.context_window_size.toLocaleString()} tokens**`,
        ].join("\n"));
      }
      if (command.name === "export") {
        const requested = parsed.argument.toLowerCase();
        if (requested && !["markdown", "md", "json"].includes(requested)) {
          addCommandResult("Export failed", "Use `/export markdown` or `/export json`.", "error");
        } else {
          const extension = exportConversation(activeSession, runtime.timeline, requested === "json" ? "json" : "markdown");
          addCommandResult("Conversation exported", `Downloaded \`${activeSession?.name || "Amadeus session"}.${extension}\`.`);
        }
      }
      if (command.name === "settings") setShowSettings(true);
      if (command.name === "contribute") setShowContribute(true);
      if (command.name === "cancel") {
        if (busy) await cancel();
        else addCommandResult("Nothing to cancel", "The current session has no active agent turn.");
      }
      if (command.name === "close") await closeSession();
    } catch (caught) {
      addCommandResult("Command failed", caught.message, "error");
    }
    return true;
  };

  const submit = async (contentOverride) => {
    const content = (typeof contentOverride === "string" ? contentOverride : draft).trim();
    if (!content || !activeId || busy) return;
    setDraft("");
    if (await executeSlashCommand(content)) return;
    setRuntime(activeId, (previous) => ({
      ...previous,
      status: "running",
      timeline: [...previous.timeline, { id: `user-${Date.now()}`, kind: "user", text: content }],
      streamingText: "",
      thinking: "",
      rawStreamingText: "",
      providerThinking: "",
      thinkingStartedAt: null,
    }));
    try {
      await api.submitMessage(activeId, content);
    } catch (caught) {
      setError(caught.message);
      setRuntime(activeId, (previous) => ({ ...previous, status: "failed" }));
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

  const reconnect = useCallback(() => {
    streamRef.current?.close();
    setLoading(true);
    setServerOnline(null);
    setError("");
    setRuntimeBySession({});
    setApiEpoch((value) => value + 1);
  }, []);

  if (loading) return <LoadingScreen />;

  return (
    <TranslationContext.Provider value={t}>
    <div className="app-shell">
      <Sidebar
        sessions={sessions}
        activeId={activeId}
        open={sidebarOpen}
        online={serverOnline}
        onSelect={setActiveId}
        onCreate={openCreateDialog}
        onSettings={() => setShowSettings(true)}
        onContribute={() => setShowContribute(true)}
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

        {error && (
          <ErrorBanner
            message={error}
            onRetry={reconnect}
            onSettings={() => setShowSettings(true)}
            onDismiss={() => setError("")}
          />
        )}

        <section className="conversation" aria-live="polite">
          {!activeSession ? (
            <EmptyState onCreate={openCreateDialog} online={serverOnline} />
          ) : (
            <div className="conversation-column">
              {!runtime.timeline.length && !runtime.streamingText && (
                <Welcome session={activeSession} />
              )}
              {runtime.timeline.map((item) => <TimelineItem key={item.id} item={item} />)}
              {runtime.thinking && <ThinkingBlock text={runtime.thinking} live startedAt={runtime.thinkingStartedAt} />}
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
          onCommand={submit}
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
          online={serverOnline}
          submitting={creatingSession}
          error={createError}
          onChange={setNewSessionName}
          onSubmit={createSession}
          onClose={() => setCreating(false)}
          onSettings={() => {
            setCreating(false);
            setShowSettings(true);
          }}
        />
      )}

      {showSettings && (
        <SettingsDialog
          online={serverOnline}
          language={language}
          onLanguage={setLanguage}
          onReconnect={reconnect}
          onClose={() => setShowSettings(false)}
        />
      )}

      {showContribute && <ContributionDialog onClose={() => setShowContribute(false)} />}
    </div>
    </TranslationContext.Provider>
  );
}

function Sidebar({ sessions, activeId, open, online, onSelect, onCreate, onSettings, onContribute, onClose }) {
  const t = useTranslation();
  return (
    <>
      <button className={`sidebar-scrim ${open ? "visible" : ""}`} aria-label={t("Close sidebar")} onClick={onClose} />
      <aside className={`sidebar ${open ? "open" : ""}`}>
        <div className="traffic-lights" aria-hidden="true"><i /><i /><i /></div>
        <div className="brand-row"><div className="brand-mark"><Sparkle weight="fill" /></div><strong>Amadeus</strong></div>
        <nav className="primary-nav" aria-label={t("Primary")}>
          <button onClick={onCreate}><Plus /><span>{t("New session")}</span></button>
          <button><Robot /><span>{t("Agents")}</span><span className="nav-count">{sessions.length}</span></button>
          <button><TerminalWindow /><span>{t("Tools")}</span></button>
          <button onClick={onContribute}><GithubLogo /><span>{t("Contribute")}</span></button>
        </nav>
        <div className="section-label">{t("Workspace")}</div>
        <div className="project-label"><FolderSimple /><span>amadeus</span></div>
        <div className="session-list">
          {sessions.map((session) => (
            <button
              key={session.id}
              className={`session-button ${session.id === activeId ? "active" : ""}`}
              onClick={() => onSelect(session.id)}
            >
              <span>{session.name}</span>
              <i className={`status-dot ${session.status}`} title={statusLabel(session.status, t)} />
            </button>
          ))}
        </div>
        <div className="sidebar-footer">
          <div className="connection"><i className={online ? "online" : "offline"} /><span>{online ? t("Local API connected") : t("API unavailable")}</span></div>
          <button onClick={onSettings} aria-label={t("Connection settings")}><GearSix /></button>
        </div>
      </aside>
    </>
  );
}

function Header({ session, status, onMenu, onDetails, onClose }) {
  const t = useTranslation();
  return (
    <header className="topbar">
      <button className="mobile-menu" onClick={onMenu} aria-label={t("Open sidebar")}><SidebarSimple /></button>
      <div className="header-title">
        <FolderSimple />
        <div><strong>{session?.name || "Amadeus"}</strong><span>{session ? session.profile : t("agent workspace")}</span></div>
      </div>
      <div className="header-actions">
        {session && <span className={`status-badge ${status}`}>{statusLabel(status, t)}</span>}
        <button className="toolbar-button" onClick={onDetails}><List /><span>{t("Details")}</span></button>
        {session && <button className="icon-button danger-hover" onClick={onClose} aria-label={t("Close session")}><Trash /></button>}
      </div>
    </header>
  );
}

function TimelineItem({ item }) {
  if (item.kind === "tool") return <ToolCard tool={item} />;
  if (item.kind === "thinking") return <ThinkingBlock text={item.text} available={item.available !== false} durationSeconds={item.durationSeconds} />;
  if (item.kind === "assistant") return <AssistantMessage text={item.text} />;
  if (item.kind === "user") return <UserMessage text={item.text} />;
  if (item.kind === "command") return <CommandResult item={item} />;
  if (item.kind === "error") return <div className="inline-notice error"><WarningCircle />{item.text}</div>;
  return <div className="inline-notice"><ArrowCounterClockwise />{item.text}</div>;
}

function UserMessage({ text }) {
  const t = useTranslation();
  return <article className="message user-message"><div className="message-label">{t("You")}</div><p>{text}</p></article>;
}

function AssistantMessage({ text, streaming = false }) {
  return (
    <article className="message assistant-message">
      <div className="assistant-rail"><div className="assistant-mark"><Sparkle weight="fill" /></div></div>
      <div className="message-body">
        <div className="message-label">Amadeus</div>
        <MarkdownContent text={text} />
        {streaming && <span className="stream-caret" aria-hidden="true" />}
      </div>
    </article>
  );
}

function CommandResult({ item }) {
  return (
    <article className={`command-result ${item.tone || "default"}`}>
      <div className="command-result-icon"><TerminalWindow /></div>
      <div className="message-body">
        <div className="message-label">{item.title}</div>
        <MarkdownContent text={item.text} />
      </div>
    </article>
  );
}

function ThinkingBlock({ text, live = false, available = true, durationSeconds = null, startedAt = null }) {
  const t = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const [elapsedSeconds, setElapsedSeconds] = useState(durationSeconds);

  useEffect(() => {
    if (!live || !startedAt) {
      setElapsedSeconds(durationSeconds);
      return undefined;
    }
    const updateElapsed = () => setElapsedSeconds(Math.max(1, Math.ceil((Date.now() - startedAt) / 1000)));
    updateElapsed();
    const timer = window.setInterval(updateElapsed, 1000);
    return () => window.clearInterval(timer);
  }, [durationSeconds, live, startedAt]);

  if (!available) {
    return (
      <section className="thinking-block unavailable" role="status">
        <div className="thinking-summary static">
          <span className="thinking-title"><WarningCircle /><strong>{t("Reasoning unavailable")}</strong></span>
        </div>
        <p>{text}</p>
      </section>
    );
  }
  const thoughtLabel = elapsedSeconds
    ? t(elapsedSeconds === 1 ? "Thought for {seconds} second" : "Thought for {seconds} seconds", { seconds: elapsedSeconds })
    : t("Thought");
  return (
    <section className={`thinking-block ${live ? "live" : "complete"}`}>
      <button className="thinking-summary" type="button" aria-expanded={expanded} onClick={() => setExpanded((value) => !value)}>
        <span className="thinking-title"><Brain /><strong>{thoughtLabel}</strong></span>
        <CaretDown className={expanded ? "expanded" : "collapsed"} aria-hidden="true" />
      </button>
      {expanded && <p>{text}</p>}
    </section>
  );
}

function ToolCard({ tool, live = false }) {
  const t = useTranslation();
  const [expanded, setExpanded] = useState(live);
  const detail = tool.command || tool.inputText || (tool.input ? JSON.stringify(tool.input, null, 2) : "");
  return (
    <article className={`tool-card ${tool.status || "complete"}`}>
      <button className="tool-summary" onClick={() => setExpanded((value) => !value)}>
        <span className="tool-icon"><Code /></span>
        <span className="tool-heading"><strong>{tool.name || t("Tool")}</strong><small>{tool.progress || (live ? t("Running") : tool.is_error ? t("Failed") : t("Completed"))}</small></span>
        {typeof tool.percent === "number" && <span className="tool-percent">{tool.percent}%</span>}
        <CaretDown className={expanded ? "rotated" : ""} />
      </button>
      {expanded && (
        <div className="tool-content">
          {detail && <CodeBlock label={t("input")} text={detail} />}
          {tool.output && <CodeBlock label={t("output")} text={tool.output} />}
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
  const t = useTranslation();
  return (
    <article className="approval-card">
      <div className="approval-icon"><WarningCircle weight="fill" /></div>
      <div className="approval-copy"><span>{t("Permission required")}</span><strong>{approval.tool}</strong><p>{approval.action || approval.reason || t("This tool needs your approval before it can continue.")}</p><CodeBlock label={t("input")} text={JSON.stringify(approval.input, null, 2)} /></div>
      <div className="approval-actions"><button onClick={() => onDecision(approval.id, "deny")}>{t("Deny")}</button><button onClick={() => onDecision(approval.id, "always_approve")}>{t("Always allow")}</button><button className="primary" onClick={() => onDecision(approval.id, "approve")}>{t("Allow once")}</button></div>
    </article>
  );
}

function SlashCommandIcon({ name }) {
  const icons = {
    help: BookOpenText,
    agent: UserPlus,
    context: Gauge,
    compact: ArrowsInLineVertical,
    tools: Wrench,
    prompt: BracketsCurly,
    export: DownloadSimple,
    settings: GearSix,
    contribute: GithubLogo,
    cancel: StopCircle,
    close: XCircle,
  };
  const Icon = icons[name] || FileText;
  return <Icon />;
}

function Composer({ draft, disabled, busy, tokenUsage, onChange, onSubmit, onCommand, onCancel, textareaRef }) {
  const t = useTranslation();
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [dismissedDraft, setDismissedDraft] = useState("");
  const matches = useMemo(() => filterSlashCommands(draft), [draft]);
  const paletteVisible = matches.length > 0 && dismissedDraft !== draft && !disabled && !busy;

  useEffect(() => {
    setSelectedIndex(0);
    if (dismissedDraft && dismissedDraft !== draft) setDismissedDraft("");
  }, [draft, dismissedDraft]);

  const selectCommand = (command) => {
    if (command.argumentHint) {
      onChange(commandDraft(command));
      requestAnimationFrame(() => textareaRef.current?.focus());
      return;
    }
    onChange("");
    onCommand(commandDraft(command));
  };

  const onKeyDown = (event) => {
    if (paletteVisible && event.key === "ArrowDown") {
      event.preventDefault();
      setSelectedIndex((index) => (index + 1) % matches.length);
      return;
    }
    if (paletteVisible && event.key === "ArrowUp") {
      event.preventDefault();
      setSelectedIndex((index) => (index - 1 + matches.length) % matches.length);
      return;
    }
    if (paletteVisible && event.key === "Escape") {
      event.preventDefault();
      setDismissedDraft(draft);
      return;
    }
    if (paletteVisible && (event.key === "Tab" || (event.key === "Enter" && !event.shiftKey))) {
      event.preventDefault();
      selectCommand(matches[selectedIndex]);
      return;
    }
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      onSubmit();
    }
  };
  return (
    <div className="composer-wrap">
      <div className="composer-stack">
        {paletteVisible && (
          <div className="slash-palette" id="slash-command-palette" role="listbox" aria-label={t("Slash commands")}>
            <div className="slash-palette-heading"><span>{t("Commands")}</span><small>{t("Navigate, select, or close")}</small></div>
            <div className="slash-command-list">
              {matches.map((command, index) => (
                <button
                  key={command.name}
                  id={`slash-command-${command.name}`}
                  className={index === selectedIndex ? "selected" : ""}
                  type="button"
                  role="option"
                  aria-selected={index === selectedIndex}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => selectCommand(command)}
                  onMouseEnter={() => setSelectedIndex(index)}
                >
                  <span className="slash-command-icon"><SlashCommandIcon name={command.icon} /></span>
                  <span className="slash-command-copy"><strong>{command.name}</strong><small>{t(command.summary)}</small></span>
                  {command.argumentHint && <code>{command.argumentHint}</code>}
                </button>
              ))}
            </div>
          </div>
        )}
        <div className={`composer ${disabled ? "disabled" : ""}`}>
          <label htmlFor="agent-prompt">{t("Message Amadeus")}</label>
          <textarea
            ref={textareaRef}
            id="agent-prompt"
            rows="2"
            value={draft}
            placeholder={disabled ? t("Connect to the local Amadeus API to begin") : t("Ask Amadeus to inspect, explain, or build anything")}
            disabled={disabled || busy}
            aria-autocomplete="list"
            aria-controls={paletteVisible ? "slash-command-palette" : undefined}
            aria-expanded={paletteVisible}
            aria-activedescendant={paletteVisible ? `slash-command-${matches[selectedIndex]?.name}` : undefined}
            onChange={(event) => onChange(event.target.value)}
            onKeyDown={onKeyDown}
          />
          <div className="composer-footer">
            <div className="composer-meta"><button aria-label={t("Add context")}><Plus /></button><span className="access-label"><WarningCircle />{t("Local full access")}</span></div>
            <div className="composer-meta right">
              {tokenUsage && <span>{t("{percent}% context", { percent: tokenUsage.context_percent })}</span>}
              <span>{t("Default agent")}</span>
              {busy ? <button className="send-button stop" onClick={onCancel} aria-label={t("Stop generation")}><Stop weight="fill" /></button> : <button className="send-button" onClick={onSubmit} disabled={!draft.trim() || disabled} aria-label={t("Send message")}><ArrowUp weight="bold" /></button>}
            </div>
          </div>
        </div>
      </div>
      <div className="composer-hint">{t("Enter to send · Shift + Enter for a new line")}</div>
    </div>
  );
}

function Welcome({ session }) {
  const t = useTranslation();
  return (
    <div className="welcome">
      <div className="welcome-mark"><Sparkle weight="fill" /></div>
      <h1>{t("What should we work on?")}</h1>
      <p>{t("{name} can inspect your project, execute tools, and keep the entire conversation in this session.", { name: session.name })}</p>
      <div className="starter-grid">
        <button onClick={() => document.getElementById("agent-prompt")?.focus()}><Code /><span><strong>{t("Explore the codebase")}</strong><small>{t("Map architecture and important flows")}</small></span></button>
        <button onClick={() => document.getElementById("agent-prompt")?.focus()}><TerminalWindow /><span><strong>{t("Build a feature")}</strong><small>{t("Plan, implement, test, and verify")}</small></span></button>
      </div>
    </div>
  );
}

function DetailsPanel({ session, runtime, onClose }) {
  const t = useTranslation();
  return (
    <aside className="details-panel">
      <div className="details-header"><strong>{t("Session details")}</strong><button onClick={onClose}><X /></button></div>
      <dl><div><dt>{t("Status")}</dt><dd>{statusLabel(runtime.status, t)}</dd></div><div><dt>{t("Profile")}</dt><dd>{session.profile}</dd></div><div><dt>{t("Session ID")}</dt><dd className="mono">{session.id}</dd></div><div><dt>{t("Messages")}</dt><dd>{runtime.timeline.filter((item) => item.kind === "user" || item.kind === "assistant").length}</dd></div><div><dt>{t("Tool calls")}</dt><dd>{runtime.timeline.filter((item) => item.kind === "tool").length}</dd></div>{runtime.tokenUsage && <><div><dt>{t("Input tokens")}</dt><dd>{runtime.tokenUsage.input_tokens.toLocaleString()}</dd></div><div><dt>{t("Output tokens")}</dt><dd>{runtime.tokenUsage.output_tokens.toLocaleString()}</dd></div></>}</dl>
    </aside>
  );
}

function CreateDialog({ value, online, submitting, error, onChange, onSubmit, onClose, onSettings }) {
  const t = useTranslation();
  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={onClose}>
      <form className="dialog" role="dialog" aria-modal="true" aria-labelledby="create-session-title" onSubmit={onSubmit} onMouseDown={(event) => event.stopPropagation()}>
        <div className="dialog-icon"><Sparkle weight="fill" /></div><h2 id="create-session-title">{t("New session")}</h2><p>{t("Start with a clean conversation and an independent agent context.")}</p>
        <label htmlFor="session-name">{t("Session name")}</label><input id="session-name" autoFocus value={value} onChange={(event) => onChange(event.target.value)} placeholder={t("Feature implementation")} />
        {(!online || error) && (
          <div className="dialog-inline-error" role="alert"><WarningCircle /><span>{error || t("The Amadeus API is unavailable.")}</span>{!online && <button type="button" onClick={onSettings}>{t("Connection settings")}</button>}</div>
        )}
        <div className="dialog-actions"><button type="button" onClick={onClose}>{t("Cancel")}</button><button className="primary" type="submit" disabled={!online || submitting}>{submitting ? t("Creating…") : online ? t("Create session") : t("API unavailable")}</button></div>
      </form>
    </div>
  );
}

function SettingsDialog({ online, language, onLanguage, onReconnect, onClose }) {
  const t = useTranslation();
  const [value, setValue] = useState(getApiBaseUrl());
  const [status, setStatus] = useState("");
  const [testing, setTesting] = useState(false);

  const testConnection = async () => {
    setTesting(true);
    setStatus("");
    try {
      const normalized = value.trim().replace(/\/$/, "");
      const parsed = new URL(normalized);
      if (!["http:", "https:"].includes(parsed.protocol)) throw new Error(t("Use an HTTP or HTTPS URL."));
      await api.health(normalized);
      setStatus(t("Connection successful."));
    } catch (caught) {
      setStatus(caught.message);
    } finally {
      setTesting(false);
    }
  };

  const save = (event) => {
    event.preventDefault();
    try {
      setApiBaseUrl(value);
      onClose();
      onReconnect();
    } catch (caught) {
      setStatus(caught.message);
    }
  };

  const reset = () => {
    const defaultUrl = resetApiBaseUrl();
    setValue(defaultUrl);
    setStatus(t("Restored the default local address. Save to reconnect."));
  };

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={onClose}>
      <form className="dialog settings-dialog" onSubmit={save} onMouseDown={(event) => event.stopPropagation()}>
        <div className="dialog-heading">
          <div className="dialog-icon"><PlugsConnected weight="fill" /></div>
          <div><h2>{t("Connection")}</h2><p>{t("Choose the Amadeus HTTP server used by this client.")}</p></div>
        </div>
        <div className="connection-summary"><i className={online ? "online" : "offline"} /><span>{online ? t("Connected") : t("Not connected")}</span><code>{getApiBaseUrl()}</code></div>
        <label htmlFor="api-url">{t("HTTP API URL")}</label>
        <input id="api-url" autoFocus value={value} onChange={(event) => setValue(event.target.value)} placeholder="http://127.0.0.1:3000" />
        <p className="field-help">{t("Remote servers should use HTTPS and authentication at the network boundary.")}</p>
        <label htmlFor="interface-language">{t("Interface language")}</label>
        <select id="interface-language" value={language} onChange={(event) => onLanguage(normalizeLanguage(event.target.value))}>
          {SUPPORTED_LANGUAGES.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
        </select>
        {status && <div className="connection-test-result" role="status">{status}</div>}
        <div className="dialog-actions split-actions">
          <button type="button" onClick={reset}>{t("Reset default")}</button>
          <span />
          <button type="button" onClick={testConnection} disabled={testing}>{testing ? t("Testing…") : t("Test")}</button>
          <button className="primary" type="submit">{t("Save and reconnect")}</button>
        </div>
      </form>
    </div>
  );
}

function ContributionDialog({ onClose }) {
  const t = useTranslation();
  const repositoryUrl = "https://github.com/xxraincandyxx/Amadeus";
  const resources = [
    { icon: BookOpenText, label: "Contribution guide", detail: "Setup, change scope, checks, and pull requests", href: `${repositoryUrl}/blob/master/CONTRIBUTING.md` },
    { icon: Sparkle, label: "Interface design system", detail: "Tokens, component rules, states, and visual QA", href: `${repositoryUrl}/blob/master/docs/WEB_DESIGN_SYSTEM.md` },
    { icon: PlugsConnected, label: "HTTP API contract", detail: "External endpoints, stability, and availability", href: `${repositoryUrl}/blob/master/docs/HTTP_API.md` },
  ];

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="dialog contribution-dialog" role="dialog" aria-modal="true" aria-labelledby="contribution-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="dialog-heading">
          <div className="dialog-icon"><GithubLogo weight="fill" /></div>
          <div><h2 id="contribution-title">{t("Build Amadeus with us")}</h2><p>{t("Every contribution should leave the product clearer, more useful, and more coherent.")}</p></div>
        </div>
        <div className="contribution-principles"><span>{t("Preserve one visual language")}</span><span>{t("Ship every interaction state")}</span><span>{t("Verify desktop and mobile")}</span></div>
        <div className="resource-list">
          {resources.map(({ icon: Icon, label, detail, href }) => (
            <a key={label} href={href} target="_blank" rel="noreferrer"><Icon /><span><strong>{t(label)}</strong><small>{t(detail)}</small></span><ArrowSquareOut /></a>
          ))}
        </div>
        <div className="dialog-actions"><button type="button" onClick={onClose}>{t("Close")}</button><a className="button-link primary" href={repositoryUrl} target="_blank" rel="noreferrer">{t("Open repository")}<ArrowSquareOut /></a></div>
      </section>
    </div>
  );
}

function ErrorBanner({ message, onRetry, onSettings, onDismiss }) {
  const t = useTranslation();
  return <div className="error-banner"><WarningCircle /><span>{message}</span><button className="text-action" onClick={onRetry}>{t("Retry")}</button><button className="text-action" onClick={onSettings}>{t("Settings")}</button><button onClick={onDismiss} aria-label={t("Dismiss")}><X /></button></div>;
}

function AgentWorking() {
  const t = useTranslation();
  return <div className="agent-working"><span /><span /><span /><em>{t("Amadeus is working")}</em></div>;
}

function EmptyState({ onCreate, online }) {
  const t = useTranslation();
  return <div className="empty-state"><Robot /><h1>{online ? t("No open sessions") : t("Amadeus API is offline")}</h1><p>{online ? t("Create a session to begin working with an agent.") : t("Start the server at {url}, then refresh this page.", { url: api.baseUrl })}</p>{online && <button onClick={onCreate}><Plus />{t("New session")}</button>}</div>;
}

function LoadingScreen() {
  return <div className="loading-screen"><div className="loading-mark"><Sparkle weight="fill" /></div><div className="loading-line" /><div className="loading-line short" /></div>;
}

export default App;
