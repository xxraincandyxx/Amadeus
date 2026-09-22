// @amadeus-header
// summary: Renders the multi-agent hierarchy, delegated tasks, and workload status.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: AgentWorkspace
// uses:
// - module: apps/client/src/agentArchitecture.js
// - module: apps/client/src/agentSessions.js
// - library: Phosphor Icons
// invariants:
// - Agent hierarchy remains readable without relying on color alone.
// - Opening an agent returns the user to that session conversation.
// side_effects: none
// tests:
// - cmd: npm test
// - cmd: npm run build
// @end-amadeus-header

import {
  ArrowRight,
  Brain,
  CheckCircle,
  ClockCountdown,
  Plus,
  Robot,
  SpinnerGap,
  TreeStructure,
  WarningCircle,
} from "@phosphor-icons/react";

import { agentSessionRows, agentSessionSummary } from "./agentSessions";
import { architectureRuntimeStatus } from "./agentArchitecture";

function WorkloadMetric({ icon: Icon, label, value, tone = "default" }) {
  return (
    <div className={`agent-metric ${tone}`}>
      <Icon aria-hidden="true" />
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

export function AgentWorkspace({ sessions, activeId, metadata, architectures, sessionArchitectures, onSelect, onCreate, onEditArchitecture, statusLabel, t }) {
  const rows = agentSessionRows(sessions);
  const summary = agentSessionSummary(sessions);

  return (
    <section className="agent-workspace" aria-labelledby="agent-workspace-title">
      <div className="agent-workspace-heading">
        <div>
          <span className="agent-workspace-kicker"><TreeStructure />{t("Multi-agent runtime")}</span>
          <h1 id="agent-workspace-title">{t("Agent workspace")}</h1>
          <p>{t("Track coordinators, delegated work, and sessions that need attention.")}</p>
        </div>
        <button className="agent-new-button" type="button" onClick={() => onCreate()}><Plus />{t("New coordinator")}</button>
      </div>

      <div className="agent-metrics" aria-label={t("Agent workload summary")}>
        <WorkloadMetric icon={SpinnerGap} label={t("Running")} value={summary.running} tone="running" />
        <WorkloadMetric icon={ClockCountdown} label={t("Needs approval")} value={summary.awaitingApproval} tone="waiting" />
        <WorkloadMetric icon={CheckCircle} label={t("Completed")} value={summary.completed} tone="completed" />
        <WorkloadMetric icon={WarningCircle} label={t("Failed")} value={summary.failed} tone="failed" />
      </div>

      <section className="agent-architecture-library" aria-labelledby="agent-architecture-library-title">
        <div className="agent-architecture-heading"><div><strong id="agent-architecture-library-title">{t("Agent architectures")}</strong><span>{t("Choose the control algorithm used to construct a new agent.")}</span></div><span>{t("Runtime")}</span></div>
        <div className="agent-architecture-rows">
          {architectures.map((architecture) => {
            const production = architectureRuntimeStatus(architecture) === "production";
            return <div className="agent-architecture-row" key={architecture.id}>
              <span className="agent-architecture-icon"><Brain /></span>
              <span className="agent-architecture-copy"><strong>{architecture.name}</strong><small>{t(architecture.description)}</small></span>
              <span className={`agent-architecture-status ${production ? "production" : "planned"}`}><i />{production ? t("Runnable now") : t("Runtime planned")}</span>
              <span className="agent-architecture-actions"><button type="button" onClick={() => onEditArchitecture(architecture.id)}>{t("Edit design")}</button><button type="button" className="primary" disabled={!production} title={!production ? t("This architecture preset is not executable yet") : t("Create agent")} onClick={() => onCreate(architecture.id)}><Plus />{t("Create agent")}</button></span>
            </div>;
          })}
        </div>
      </section>

      <div className="agent-roster">
        <div className="agent-roster-heading">
          <div><strong>{t("Agent hierarchy")}</strong><span>{t("{count} sessions, {delegated} delegated", { count: summary.total, delegated: summary.delegated })}</span></div>
          <span>{t("Status")}</span>
        </div>
        {rows.length ? (
          <div className="agent-rows">
            {rows.map(({ session, parent, depth, childCount }) => {
              const task = metadata[session.id]?.prompt;
              const isRoot = !session.parent_session_id;
              return (
                <button
                  key={session.id}
                  className={`agent-row ${isRoot ? "root" : "child"} ${session.id === activeId ? "selected" : ""}`}
                  style={{ "--agent-depth": depth }}
                  type="button"
                  onClick={() => onSelect(session.id)}
                >
                  <span className="agent-tree-cell">
                    <span className={`agent-avatar ${isRoot ? "root" : "child"}`}>{isRoot ? <Robot /> : <TreeStructure />}</span>
                    <span className="agent-identity">
                      <strong>{session.name}</strong>
                      <small>{isRoot ? t("Coordinator") : t("Sub-agent of {name}", { name: parent?.name || t("Unknown agent") })}</small>
                    </span>
                  </span>
                  <span className="agent-task-cell">
                    <strong>{task || (isRoot ? t("Coordinates this session and delegates focused work.") : t("Delegated task details are available in the conversation."))}</strong>
                    <small>{childCount ? t(childCount === 1 ? "1 child agent" : "{count} child agents", { count: childCount }) : architectures.find(({ id }) => id === sessionArchitectures[session.id])?.name || t("Production ReAct")}</small>
                  </span>
                  <span className={`agent-row-status ${session.status}`}><i />{statusLabel(session.status)}</span>
                  <span className="agent-open-icon" aria-hidden="true"><ArrowRight /></span>
                </button>
              );
            })}
          </div>
        ) : (
          <div className="agent-roster-empty"><Robot /><strong>{t("No agents yet")}</strong><span>{t("Create a coordinator to start a multi-agent task.")}</span></div>
        )}
      </div>
    </section>
  );
}
