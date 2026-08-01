// @amadeus-header
// summary: Provides the browser and native HTTP client for Amadeus APIs.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - const: api
// - fn: getApiBaseUrl
// - fn: resetApiBaseUrl
// - fn: setApiBaseUrl
// uses:
// - protocol: Amadeus HTTP and SSE APIs
// invariants:
// - Runtime endpoint overrides are normalized before use.
// - API failures surface server-provided messages when available.
// side_effects:
// - Reads and writes browser local storage.
// - Performs HTTP requests.
// tests:
// - cmd: npm run build
// @end-amadeus-header

const DEFAULT_API_BASE = (import.meta.env.VITE_AMADEUS_API_URL || "http://127.0.0.1:3000").replace(/\/$/, "");
const API_STORAGE_KEY = "amadeus.apiUrl";
const LEGACY_DESKTOP_API_BASES = new Set(["http://127.0.0.1:3100", "http://localhost:3100"]);

function normalizeBaseUrl(value) {
  return value.trim().replace(/\/$/, "");
}

export function getApiBaseUrl() {
  const stored = localStorage.getItem(API_STORAGE_KEY);
  if (window.__TAURI_INTERNALS__ && LEGACY_DESKTOP_API_BASES.has(normalizeBaseUrl(stored || ""))) {
    localStorage.removeItem(API_STORAGE_KEY);
    return DEFAULT_API_BASE;
  }
  return normalizeBaseUrl(stored || DEFAULT_API_BASE);
}

export function setApiBaseUrl(value) {
  const normalized = normalizeBaseUrl(value);
  if (!normalized) throw new Error("Enter an HTTP API URL.");
  const parsed = new URL(normalized);
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    throw new Error("The API URL must use HTTP or HTTPS.");
  }
  localStorage.setItem(API_STORAGE_KEY, normalized);
  return normalized;
}

export function resetApiBaseUrl() {
  localStorage.removeItem(API_STORAGE_KEY);
  return DEFAULT_API_BASE;
}

async function request(path, options = {}, baseUrl = getApiBaseUrl()) {
  const response = await fetch(`${baseUrl}${path}`, {
    headers: { "Content-Type": "application/json", ...options.headers },
    ...options,
  });

  const text = await response.text();
  let payload = null;
  try {
    payload = text ? JSON.parse(text) : null;
  } catch {
    throw new Error(`The API returned an invalid response (${response.status}).`);
  }
  if (!response.ok) {
    throw new Error(payload?.message || payload?.error || `Request failed (${response.status})`);
  }
  return payload;
}

export const api = {
  get baseUrl() {
    return getApiBaseUrl();
  },
  health: (baseUrl) => request("/health", {}, baseUrl),
  listSessions: () => request("/v1/sessions"),
  createSession: (name, profile = "default") =>
    request("/v1/sessions", { method: "POST", body: JSON.stringify({ name, profile }) }),
  getSession: (id) => request(`/v1/sessions/${id}`),
  getHistory: (id) => request(`/v1/sessions/${id}/history`),
  getConfig: () => request("/config"),
  getToolCatalog: () => request("/tools/catalog"),
  submitMessage: (id, content) =>
    request(`/v1/sessions/${id}/messages`, { method: "POST", body: JSON.stringify({ content }) }),
  cancel: (id) => request(`/v1/sessions/${id}/cancel`, { method: "POST" }),
  close: (id) => request(`/v1/sessions/${id}`, { method: "DELETE" }),
  approvals: (id) => request(`/v1/sessions/${id}/approvals`),
  approve: (sessionId, approvalId, decision) =>
    request(`/v1/sessions/${sessionId}/approvals/${approvalId}`, {
      method: "POST",
      body: JSON.stringify({ decision }),
    }),
  eventUrl: (id) => `${getApiBaseUrl()}/v1/sessions/${id}/events`,
};
