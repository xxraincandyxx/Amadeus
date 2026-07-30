const API_BASE = (import.meta.env.VITE_AMADEUS_API_URL || "http://127.0.0.1:3000").replace(/\/$/, "");

async function request(path, options = {}) {
  const response = await fetch(`${API_BASE}${path}`, {
    headers: { "Content-Type": "application/json", ...options.headers },
    ...options,
  });

  const text = await response.text();
  const payload = text ? JSON.parse(text) : null;
  if (!response.ok) {
    throw new Error(payload?.message || payload?.error || `Request failed (${response.status})`);
  }
  return payload;
}

export const api = {
  baseUrl: API_BASE,
  health: () => request("/health"),
  listSessions: () => request("/v1/sessions"),
  createSession: (name, profile = "default") =>
    request("/v1/sessions", { method: "POST", body: JSON.stringify({ name, profile }) }),
  getSession: (id) => request(`/v1/sessions/${id}`),
  getHistory: (id) => request(`/v1/sessions/${id}/history`),
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
  eventUrl: (id) => `${API_BASE}/v1/sessions/${id}/events`,
};
