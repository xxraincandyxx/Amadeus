// @amadeus-header
// summary: Persists workspace identity and last-session selection for the local application user.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - const: WORKSPACE_REGISTRY_STORAGE_KEY
// - fn: loadWorkspaceRegistry
// - fn: workspaceFromConfig
// - fn: rememberWorkspace
// - fn: loadWorkspaceActiveSession
// - fn: saveWorkspaceActiveSession
// uses:
// - API: browser local storage
// invariants:
// - Workspace identities are stable for a normalized API endpoint and working directory.
// - Invalid stored workspace data falls back to an empty versioned registry.
// side_effects:
// - Reads and writes browser local storage.
// tests:
// - apps/client/src/workspaceState.test.js
// @end-amadeus-header

export const WORKSPACE_REGISTRY_STORAGE_KEY = "amadeus.workspaces.v1";

const EMPTY_REGISTRY = Object.freeze({ version: 1, activeWorkspaceId: null, workspaces: {} });

function normalizedPath(value) {
  const path = String(value || "").trim().replace(/[\\/]+$/, "");
  return path || "unknown";
}

function normalizedApiUrl(value) {
  return String(value || "").trim().replace(/\/$/, "");
}

function workspaceName(path) {
  if (path === "unknown") return "Workspace";
  return path.split(/[\\/]/).filter(Boolean).at(-1) || "Workspace";
}

function stableHash(value) {
  let hash = 2166136261;
  for (const character of value) {
    hash ^= character.codePointAt(0);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(36);
}

export function loadWorkspaceRegistry(storage) {
  try {
    const value = JSON.parse(storage?.getItem(WORKSPACE_REGISTRY_STORAGE_KEY));
    if (value?.version !== 1 || !value.workspaces || Array.isArray(value.workspaces)) {
      return { ...EMPTY_REGISTRY, workspaces: {} };
    }
    return {
      version: 1,
      activeWorkspaceId: typeof value.activeWorkspaceId === "string" ? value.activeWorkspaceId : null,
      workspaces: value.workspaces,
    };
  } catch {
    return { ...EMPTY_REGISTRY, workspaces: {} };
  }
}

export function workspaceFromConfig(config, apiUrl) {
  const path = normalizedPath(config?.working_dir);
  const endpoint = normalizedApiUrl(apiUrl);
  return {
    id: `workspace-${stableHash(`${endpoint}\n${path}`)}`,
    name: workspaceName(path),
    path,
    apiUrl: endpoint,
  };
}

export function rememberWorkspace(storage, config, apiUrl, openedAt = Date.now()) {
  const identity = workspaceFromConfig(config, apiUrl);
  const registry = loadWorkspaceRegistry(storage);
  const previous = registry.workspaces[identity.id] || {};
  const workspace = { ...previous, ...identity, lastOpenedAt: openedAt };
  storage?.setItem(WORKSPACE_REGISTRY_STORAGE_KEY, JSON.stringify({
    version: 1,
    activeWorkspaceId: identity.id,
    workspaces: { ...registry.workspaces, [identity.id]: workspace },
  }));
  return workspace;
}

export function loadWorkspaceActiveSession(storage, workspaceId) {
  if (!workspaceId) return null;
  const value = loadWorkspaceRegistry(storage).workspaces[workspaceId]?.activeSessionId;
  return typeof value === "string" && value ? value : null;
}

export function saveWorkspaceActiveSession(storage, workspaceId, sessionId) {
  if (!workspaceId) return;
  const registry = loadWorkspaceRegistry(storage);
  const workspace = registry.workspaces[workspaceId];
  if (!workspace) return;
  storage?.setItem(WORKSPACE_REGISTRY_STORAGE_KEY, JSON.stringify({
    ...registry,
    activeWorkspaceId: workspaceId,
    workspaces: {
      ...registry.workspaces,
      [workspaceId]: { ...workspace, activeSessionId: sessionId || null },
    },
  }));
}
