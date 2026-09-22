// @amadeus-header
// summary: Renders the searchable runtime tool catalog and active tool profile.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: ToolsWorkspace
// uses:
// - module: apps/client/src/api.js
// - library: Phosphor Icons
// invariants:
// - Tool catalog data is loaded from the active Amadeus API endpoint.
// - Loading, error, empty, and populated states remain keyboard accessible.
// side_effects:
// - Fetches tool catalog and configuration data from the Amadeus API.
// tests:
// - cmd: npm test
// - cmd: npm run build
// @end-amadeus-header

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ArrowClockwise,
  CheckCircle,
  MagnifyingGlass,
  ShieldCheck,
  TerminalWindow,
  WarningCircle,
  Wrench,
} from "@phosphor-icons/react";

import { api } from "./api";

function readableValue(value) {
  return String(value || "unknown").replaceAll("_", " ");
}

export function ToolsWorkspace({ online, t }) {
  const [tools, setTools] = useState([]);
  const [profile, setProfile] = useState("default");
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  const loadTools = useCallback(async () => {
    if (!online) {
      setLoading(false);
      setError(t("Connect to the Amadeus API to inspect tools."));
      return;
    }
    setLoading(true);
    setError("");
    try {
      const [catalog, config] = await Promise.all([api.getToolCatalog(), api.getConfig()]);
      setTools(catalog.tools || []);
      setProfile(config.tools?.active_profile || "default");
    } catch (caught) {
      setError(caught.message);
    } finally {
      setLoading(false);
    }
  }, [online, t]);

  useEffect(() => {
    loadTools();
  }, [loadTools]);

  const filteredTools = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    if (!normalized) return tools;
    return tools.filter((tool) => (
      `${tool.name} ${tool.description} ${tool.level} ${tool.permission_mode}`
        .toLocaleLowerCase()
        .includes(normalized)
    ));
  }, [query, tools]);

  const permissionCount = new Set(tools.map((tool) => tool.permission_mode)).size;

  return (
    <section className="tools-workspace" aria-labelledby="tools-workspace-title">
      <div className="tools-workspace-inner">
        <header className="tools-workspace-heading">
          <div>
            <span className="tools-workspace-kicker"><TerminalWindow />{t("Runtime capabilities")}</span>
            <h1 id="tools-workspace-title">{t("Tools")}</h1>
            <p>{t("Inspect the tools available to agents and the permissions each tool requires.")}</p>
          </div>
          <button className="tools-refresh-button" type="button" onClick={loadTools} disabled={loading}>
            <ArrowClockwise />{loading ? t("Refreshing…") : t("Refresh")}
          </button>
        </header>

        <div className="tools-summary" aria-label={t("Tool catalog summary")}>
          <div><Wrench /><span>{t("Available tools")}</span><strong>{tools.length}</strong></div>
          <div><CheckCircle /><span>{t("Active profile")}</span><strong>{profile}</strong></div>
          <div><ShieldCheck /><span>{t("Permission modes")}</span><strong>{permissionCount}</strong></div>
        </div>

        <div className="tools-catalog">
          <div className="tools-catalog-toolbar">
            <div><strong>{t("Tool catalog")}</strong><span>{t("{count} tools", { count: filteredTools.length })}</span></div>
            <label className="tools-search" htmlFor="tools-search-input">
              <MagnifyingGlass aria-hidden="true" />
              <input
                id="tools-search-input"
                type="search"
                aria-label={t("Search tools")}
                value={query}
                placeholder={t("Search tools")}
                onChange={(event) => setQuery(event.target.value)}
              />
            </label>
          </div>

          {loading ? (
            <div className="tools-loading" aria-label={t("Loading tool catalog")}>
              {[0, 1, 2, 3].map((item) => <div key={item}><i /><span /><small /></div>)}
            </div>
          ) : error ? (
            <div className="tools-state error" role="alert">
              <WarningCircle /><strong>{t("Tool catalog unavailable")}</strong><span>{error}</span>
              <button type="button" onClick={loadTools}>{t("Retry")}</button>
            </div>
          ) : filteredTools.length ? (
            <div className="tool-table">
              <div className="tool-table-header" aria-hidden="true">
                <span>{t("Tool")}</span><span>{t("Description")}</span><span>{t("Level")}</span><span>{t("Permission")}</span>
              </div>
              {filteredTools.map((tool) => (
                <article className="tool-row" key={tool.name}>
                  <div className="tool-name"><TerminalWindow /><code>{tool.name}</code></div>
                  <p>{tool.description || t("No description provided.")}</p>
                  <span className="tool-level">{readableValue(tool.level)}</span>
                  <span className="tool-permission"><ShieldCheck />{readableValue(tool.permission_mode)}</span>
                </article>
              ))}
            </div>
          ) : (
            <div className="tools-state"><MagnifyingGlass /><strong>{t("No tools found")}</strong><span>{t("Try a different search term.")}</span></div>
          )}
        </div>
      </div>
    </section>
  );
}
