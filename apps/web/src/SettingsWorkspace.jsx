// @amadeus-header
// summary: Renders persistent application preferences and API connection settings.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: SettingsWorkspace
// uses:
// - module: apps/web/src/api.js
// - module: apps/web/src/i18n.js
// - module: apps/web/src/theme.js
// invariants:
// - Accent, surface-opacity, and language changes apply immediately and persist locally.
// - API endpoint changes reconnect only after explicit save.
// - Workspace identity reflects the connected runtime and remains read-only.
// side_effects:
// - Tests HTTP API connectivity.
// - Persists client preferences through parent callbacks.
// tests:
// - cmd: npm test
// - cmd: npm run build
// @end-amadeus-header

import { Check, FolderSimple, PaintBrush, PlugsConnected, Translate } from "@phosphor-icons/react";
import { useState } from "react";

import { api, getApiBaseUrl, resetApiBaseUrl, setApiBaseUrl } from "./api";
import { normalizeLanguage, SUPPORTED_LANGUAGES } from "./i18n";
import { DEFAULT_SURFACE_APPEARANCE, DEFAULT_THEME_COLOR, THEME_COLOR_PRESETS, normalizeSurfaceAppearance, normalizeThemeColor } from "./theme";

export function SettingsWorkspace({ online, language, themeColor, surfaceAppearance, workspace, onLanguage, onThemeColor, onSurfaceAppearance, onReconnect, t }) {
  const [apiUrl, setApiUrl] = useState(getApiBaseUrl());
  const [customColor, setCustomColor] = useState(themeColor);
  const [status, setStatus] = useState("");
  const [testing, setTesting] = useState(false);

  const selectColor = (color) => {
    const normalized = normalizeThemeColor(color);
    setCustomColor(normalized);
    onThemeColor(normalized);
  };

  const selectSurfaceOpacity = (property, value) => {
    onSurfaceAppearance(normalizeSurfaceAppearance({ ...surfaceAppearance, [property]: value }));
  };

  const testConnection = async () => {
    setTesting(true);
    setStatus("");
    try {
      const normalized = apiUrl.trim().replace(/\/$/, "");
      const parsed = new URL(normalized);
      if (!["http:", "https:"].includes(parsed.protocol)) throw new Error(t("Use an HTTP or HTTPS URL."));
      await api.health(normalized);
      setStatus(t("Connection successful."));
    } catch (caught) {
      if (caught instanceof TypeError) {
        setStatus(t("Could not reach the Amadeus API. Check that the server is running and the address is correct."));
        return;
      }
      setStatus(caught.message);
    } finally {
      setTesting(false);
    }
  };

  const saveConnection = (event) => {
    event.preventDefault();
    try {
      setApiBaseUrl(apiUrl);
      setStatus(t("Connection saved. Reconnecting…"));
      onReconnect();
    } catch (caught) {
      setStatus(caught.message);
    }
  };

  const resetConnection = () => {
    const defaultUrl = resetApiBaseUrl();
    setApiUrl(defaultUrl);
    setStatus(t("Restored the default local address. Save to reconnect."));
  };

  return (
    <section className="settings-workspace" aria-labelledby="settings-title">
      <div className="settings-page">
        <header className="settings-heading">
          <span className="settings-kicker"><PaintBrush />{t("Preferences")}</span>
          <h1 id="settings-title">{t("Settings")}</h1>
          <p>{t("Configure the interface and the local runtime connection.")}</p>
        </header>

        <section className="settings-section" aria-labelledby="workspace-settings-title">
          <div className="settings-section-heading"><FolderSimple /><div><h2 id="workspace-settings-title">{t("Current workspace")}</h2><p>{t("Workspace identity and the last selected session are remembered for this user.")}</p></div></div>
          <div className="settings-section-controls workspace-summary">
            <div><span>{t("Workspace name")}</span><strong>{workspace?.name || t("Unavailable")}</strong></div>
            <div><span>{t("Project folder")}</span><code>{workspace?.path || t("Unavailable")}</code></div>
            <small>{t("Saved for this user on this device.")}</small>
          </div>
        </section>

        <section className="settings-section" aria-labelledby="appearance-settings-title">
          <div className="settings-section-heading"><PaintBrush /><div><h2 id="appearance-settings-title">{t("Appearance")}</h2><p>{t("Choose the accent used for active controls, focus, and workflow state.")}</p></div></div>
          <div className="settings-section-controls">
            <fieldset className="theme-fieldset">
              <legend>{t("Theme color")}</legend>
              <div className="theme-swatches">
                {THEME_COLOR_PRESETS.map((preset) => (
                  <button key={preset.id} type="button" className={themeColor === preset.color ? "selected" : ""} onClick={() => selectColor(preset.color)} aria-label={t(preset.label)} aria-pressed={themeColor === preset.color}>
                    <span className="theme-swatch" style={{ backgroundColor: preset.color }} />
                    <span>{t(preset.label)}</span>
                    {themeColor === preset.color && <Check weight="bold" />}
                  </button>
                ))}
              </div>
            </fieldset>
            <label className="custom-color-control" htmlFor="custom-theme-color">
              <span><strong>{t("Custom color")}</strong><small>{t("Use any six-digit hexadecimal color.")}</small></span>
              <span className="custom-color-inputs">
                <input type="color" aria-label={t("Choose custom theme color")} value={customColor} onChange={(event) => selectColor(event.target.value)} />
                <input id="custom-theme-color" value={customColor} onChange={(event) => setCustomColor(event.target.value)} onBlur={() => selectColor(customColor)} onKeyDown={(event) => { if (event.key === "Enter") selectColor(customColor); }} />
              </span>
            </label>
            <button className="settings-text-button" type="button" onClick={() => selectColor(DEFAULT_THEME_COLOR)}>{t("Restore dark red")}</button>
            <fieldset className="surface-opacity-fieldset">
              <legend>{t("Glass surfaces")}</legend>
              <p>{t("Lower opacity reveals the ambient layer. Set a surface to 100% to disable its glass effect.")}</p>
              <label className="surface-opacity-control" htmlFor="sidebar-opacity">
                <span><strong>{t("Sidebar opacity")}</strong><small>{t("Controls the navigation rail and mobile drawer.")}</small></span>
                <output htmlFor="sidebar-opacity">{surfaceAppearance.sidebarOpacity}%</output>
                <input id="sidebar-opacity" type="range" min="0" max="100" step="1" value={surfaceAppearance.sidebarOpacity} aria-valuetext={`${surfaceAppearance.sidebarOpacity}%`} onChange={(event) => selectSurfaceOpacity("sidebarOpacity", event.target.value)} />
              </label>
              <label className="surface-opacity-control" htmlFor="main-page-opacity">
                <span><strong>{t("Main page opacity")}</strong><small>{t("Defaults to 100% so the workspace remains opaque.")}</small></span>
                <output htmlFor="main-page-opacity">{surfaceAppearance.mainOpacity}%</output>
                <input id="main-page-opacity" type="range" min="0" max="100" step="1" value={surfaceAppearance.mainOpacity} aria-valuetext={`${surfaceAppearance.mainOpacity}%`} onChange={(event) => selectSurfaceOpacity("mainOpacity", event.target.value)} />
              </label>
              <button className="settings-text-button" type="button" onClick={() => onSurfaceAppearance({ ...DEFAULT_SURFACE_APPEARANCE })}>{t("Restore material defaults")}</button>
            </fieldset>
          </div>
        </section>

        <section className="settings-section" aria-labelledby="language-settings-title">
          <div className="settings-section-heading"><Translate /><div><h2 id="language-settings-title">{t("Language")}</h2><p>{t("Select the language used throughout the application.")}</p></div></div>
          <div className="settings-section-controls">
            <label className="settings-field" htmlFor="interface-language"><span>{t("Interface language")}</span><select id="interface-language" value={language} onChange={(event) => onLanguage(normalizeLanguage(event.target.value))}>{SUPPORTED_LANGUAGES.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label>
          </div>
        </section>

        <section className="settings-section" aria-labelledby="connection-settings-title">
          <div className="settings-section-heading"><PlugsConnected /><div><h2 id="connection-settings-title">{t("Connection")}</h2><p>{t("Choose the Amadeus HTTP server used by this client.")}</p></div></div>
          <form className="settings-section-controls" onSubmit={saveConnection}>
            <div className="connection-summary"><i className={online ? "online" : "offline"} /><span>{online ? t("Connected") : t("Not connected")}</span><code>{getApiBaseUrl()}</code></div>
            <label className="settings-field" htmlFor="api-url"><span>{t("HTTP API URL")}</span><input id="api-url" value={apiUrl} onChange={(event) => setApiUrl(event.target.value)} placeholder="http://127.0.0.1:3000" /><small>{t("Remote servers should use HTTPS and authentication at the network boundary.")}</small></label>
            {status && <div className="connection-test-result" role="status">{status}</div>}
            <div className="settings-actions"><button type="button" onClick={resetConnection}>{t("Reset default")}</button><span /><button type="button" onClick={testConnection} disabled={testing}>{testing ? t("Testing…") : t("Test")}</button><button className="primary" type="submit">{t("Save and reconnect")}</button></div>
          </form>
        </section>
      </div>
    </section>
  );
}
