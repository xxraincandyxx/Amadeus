// @amadeus-header
// summary: Derives stable multi-agent hierarchy and workload summaries from bridge sessions.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: agentSessionRows
// - fn: agentSessionSummary
// - fn: sessionRelations
// - fn: upsertSession
// uses:
// - protocol: Amadeus session metadata
// invariants:
// - Orphaned and cyclic parent references remain visible as root rows.
// - Every session appears exactly once in the derived hierarchy.
// side_effects: none
// tests:
// - apps/client/src/agentSessions.test.js
// @end-amadeus-header

export function upsertSession(sessions, nextSession) {
  const index = sessions.findIndex((session) => session.id === nextSession.id);
  if (index < 0) return [...sessions, nextSession];
  const updated = [...sessions];
  updated[index] = { ...updated[index], ...nextSession };
  return updated;
}

export function agentSessionRows(sessions = []) {
  const visible = sessions.filter((session) => session.status !== "closed");
  const byId = new Map(visible.map((session) => [session.id, session]));
  const children = new Map();

  for (const session of visible) {
    const parentId = session.parent_session_id;
    if (!parentId || parentId === session.id || !byId.has(parentId)) continue;
    children.set(parentId, [...(children.get(parentId) || []), session]);
  }

  const visited = new Set();
  const rows = [];
  const append = (session, depth, ancestry) => {
    if (visited.has(session.id)) return;
    visited.add(session.id);
    const childSessions = children.get(session.id) || [];
    rows.push({
      session,
      depth,
      parent: byId.get(session.parent_session_id) || null,
      childCount: childSessions.length,
    });
    const nextAncestry = new Set(ancestry).add(session.id);
    for (const child of childSessions) {
      if (!nextAncestry.has(child.id)) append(child, depth + 1, nextAncestry);
    }
  };

  const roots = visible.filter((session) => {
    const parentId = session.parent_session_id;
    return !parentId || parentId === session.id || !byId.has(parentId);
  });
  for (const root of roots) append(root, 0, new Set());
  for (const session of visible) append(session, 0, new Set());
  return rows;
}

export function agentSessionSummary(sessions = []) {
  const visible = sessions.filter((session) => session.status !== "closed");
  return {
    total: visible.length,
    delegated: visible.filter((session) => Boolean(session.parent_session_id)).length,
    running: visible.filter((session) => session.status === "running").length,
    awaitingApproval: visible.filter((session) => session.status === "awaiting_approval").length,
    failed: visible.filter((session) => session.status === "failed").length,
    completed: visible.filter((session) => session.status === "completed").length,
  };
}

export function sessionRelations(sessions = [], sessionId) {
  const rows = agentSessionRows(sessions);
  const row = rows.find((candidate) => candidate.session.id === sessionId);
  if (!row) return { parent: null, children: [], depth: 0 };
  return {
    parent: row.parent,
    children: rows
      .filter((candidate) => candidate.session.parent_session_id === sessionId)
      .map((candidate) => candidate.session),
    depth: row.depth,
  };
}
