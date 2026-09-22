// @amadeus-header
// summary: Persists user-customized agent character profiles per workspace and session.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - const: AGENT_PROFILE_STORAGE_KEY
// - const: MAX_AGENT_AVATAR_BYTES
// - fn: defaultAgentProfile
// - fn: loadAgentProfile
// - fn: saveAgentProfile
// uses:
// - API: browser local storage
// invariants:
// - Profiles are isolated by workspace and session identifiers.
// - Stored profile fields are normalized before use.
// - Avatar data is limited to supported local image data URLs.
// side_effects:
// - Reads and writes browser local storage.
// tests:
// - apps/client/src/agentProfileState.test.js
// @end-amadeus-header

export const AGENT_PROFILE_STORAGE_KEY = "amadeus.agentProfiles.v1";
export const MAX_AGENT_AVATAR_BYTES = 1_000_000;

const MAX_AVATAR_DATA_URL_LENGTH = 1_400_000;
const MAX_TAGS = 12;

function emptyRegistry() {
  return { version: 1, workspaces: {} };
}

function normalizeText(value, maximumLength) {
  return String(value || "").trim().slice(0, maximumLength);
}

function normalizeTags(values) {
  const seen = new Set();
  return (Array.isArray(values) ? values : []).reduce((tags, value) => {
    const tag = normalizeText(value, 40);
    const key = tag.toLocaleLowerCase();
    if (!tag || seen.has(key) || tags.length >= MAX_TAGS) return tags;
    seen.add(key);
    tags.push(tag);
    return tags;
  }, []);
}

function normalizeAvatar(value) {
  if (typeof value !== "string" || value.length > MAX_AVATAR_DATA_URL_LENGTH) return null;
  return /^data:image\/(?:png|jpeg|webp);base64,/i.test(value) ? value : null;
}

function normalizeProfile(profile, session) {
  return {
    displayName: normalizeText(profile?.displayName, 80) || normalizeText(session?.name, 80) || "Agent",
    role: normalizeText(profile?.role, 80),
    bio: normalizeText(profile?.bio, 600),
    likes: normalizeTags(profile?.likes),
    dislikes: normalizeTags(profile?.dislikes),
    avatarDataUrl: normalizeAvatar(profile?.avatarDataUrl),
  };
}

function loadRegistry(storage) {
  try {
    const value = JSON.parse(storage?.getItem(AGENT_PROFILE_STORAGE_KEY));
    if (value?.version !== 1 || !value.workspaces || Array.isArray(value.workspaces)) return emptyRegistry();
    return value;
  } catch {
    return emptyRegistry();
  }
}

export function defaultAgentProfile(session) {
  return normalizeProfile({}, session);
}

export function loadAgentProfile(storage, workspaceId, session) {
  if (!workspaceId || !session?.id) return defaultAgentProfile(session);
  const stored = loadRegistry(storage).workspaces[workspaceId]?.agents?.[session.id];
  return normalizeProfile(stored, session);
}

export function saveAgentProfile(storage, workspaceId, session, profile) {
  const normalized = normalizeProfile(profile, session);
  if (!workspaceId || !session?.id) return normalized;
  const registry = loadRegistry(storage);
  const workspace = registry.workspaces[workspaceId] || { agents: {} };
  storage?.setItem(AGENT_PROFILE_STORAGE_KEY, JSON.stringify({
    version: 1,
    workspaces: {
      ...registry.workspaces,
      [workspaceId]: {
        ...workspace,
        agents: { ...workspace.agents, [session.id]: normalized },
      },
    },
  }));
  return normalized;
}
