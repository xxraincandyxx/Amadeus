// @amadeus-header
// summary: Persists and applies configurable interface accent and glass-surface appearance.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - const: DEFAULT_THEME_COLOR
// - const: DEFAULT_SURFACE_APPEARANCE
// - const: DEFAULT_SHEET_COLOR
// - const: THEME_COLOR_PRESETS
// - fn: loadThemeColor
// - fn: applyThemeColor
// - fn: loadSurfaceAppearance
// - fn: applySurfaceAppearance
// uses:
// - API: browser local storage
// - API: CSS custom properties
// invariants:
// - Stored theme colors use six-digit hexadecimal notation.
// - Invalid or missing values resolve to the dark-red default.
// - Sidebar opacity defaults to 68 percent, the embedded sheet to 55 percent at
//   #26262b, the background scrim to 78 percent, and the main page to fully opaque.
// side_effects:
// - Reads and writes browser local storage.
// - Updates document-root CSS variables.
// tests:
// - cmd: npm test
// @end-amadeus-header

export const THEME_COLOR_STORAGE_KEY = "amadeus.themeColor";
export const SURFACE_APPEARANCE_STORAGE_KEY = "amadeus.surfaceAppearance.v1";
export const DEFAULT_THEME_COLOR = "#a72f42";
export const DEFAULT_SHEET_COLOR = "#26262b";
export const DEFAULT_SURFACE_APPEARANCE = Object.freeze({
  sidebarOpacity: 68,
  mainOpacity: 100,
  sheetColor: DEFAULT_SHEET_COLOR,
  sheetOpacity: 55,
  scrimOpacity: 78,
});

export const THEME_COLOR_PRESETS = [
  { id: "dark-red", color: DEFAULT_THEME_COLOR, label: "Dark red" },
  { id: "burnt-orange", color: "#c8662e", label: "Burnt orange" },
  { id: "cobalt", color: "#3977c5", label: "Cobalt" },
  { id: "forest", color: "#31815b", label: "Forest" },
  { id: "graphite", color: "#767676", label: "Graphite" },
];

export function normalizeThemeColor(value, fallback = DEFAULT_THEME_COLOR) {
  const normalized = String(value || "").trim().toLowerCase();
  return /^#[0-9a-f]{6}$/.test(normalized) ? normalized : fallback;
}

export function loadThemeColor(storage) {
  return normalizeThemeColor(storage?.getItem(THEME_COLOR_STORAGE_KEY));
}

export function themeColorVariables(value) {
  const color = normalizeThemeColor(value);
  const red = Number.parseInt(color.slice(1, 3), 16);
  const green = Number.parseInt(color.slice(3, 5), 16);
  const blue = Number.parseInt(color.slice(5, 7), 16);
  return {
    "--accent": color,
    "--accent-rgb": `${red}, ${green}, ${blue}`,
    "--accent-soft": `rgba(${red}, ${green}, ${blue}, 0.14)`,
  };
}

export function applyThemeColor(value, storage, root) {
  const color = normalizeThemeColor(value);
  storage?.setItem(THEME_COLOR_STORAGE_KEY, color);
  const target = root || (typeof document === "undefined" ? null : document.documentElement);
  if (target) {
    Object.entries(themeColorVariables(color)).forEach(([property, propertyValue]) => {
      target.style.setProperty(property, propertyValue);
    });
  }
  return color;
}

function normalizeOpacity(value, fallback) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) return fallback;
  return Math.min(100, Math.max(0, Math.round(numeric)));
}

export function normalizeSurfaceAppearance(value) {
  const source = value && typeof value === "object" ? value : {};
  return {
    sidebarOpacity: normalizeOpacity(source.sidebarOpacity, DEFAULT_SURFACE_APPEARANCE.sidebarOpacity),
    mainOpacity: normalizeOpacity(source.mainOpacity, DEFAULT_SURFACE_APPEARANCE.mainOpacity),
    sheetColor: normalizeThemeColor(source.sheetColor, DEFAULT_SHEET_COLOR),
    sheetOpacity: normalizeOpacity(source.sheetOpacity, DEFAULT_SURFACE_APPEARANCE.sheetOpacity),
    scrimOpacity: normalizeOpacity(source.scrimOpacity, DEFAULT_SURFACE_APPEARANCE.scrimOpacity),
  };
}

export function loadSurfaceAppearance(storage) {
  try {
    return normalizeSurfaceAppearance(JSON.parse(storage?.getItem(SURFACE_APPEARANCE_STORAGE_KEY) || "null"));
  } catch {
    return { ...DEFAULT_SURFACE_APPEARANCE };
  }
}

export function surfaceAppearanceVariables(value) {
  const appearance = normalizeSurfaceAppearance(value);
  return {
    "--sidebar-opacity": `${appearance.sidebarOpacity}%`,
    "--main-page-opacity": `${appearance.mainOpacity}%`,
    "--sheet-fill": `color-mix(in srgb, ${appearance.sheetColor} ${appearance.sheetOpacity}%, transparent)`,
    "--glass-scrim": `rgba(4, 4, 6, ${(appearance.scrimOpacity / 100).toFixed(2)})`,
  };
}

export function applySurfaceAppearance(value, storage, root) {
  const appearance = normalizeSurfaceAppearance(value);
  storage?.setItem(SURFACE_APPEARANCE_STORAGE_KEY, JSON.stringify(appearance));
  const target = root || (typeof document === "undefined" ? null : document.documentElement);
  if (target) {
    Object.entries(surfaceAppearanceVariables(appearance)).forEach(([property, propertyValue]) => {
      target.style.setProperty(property, propertyValue);
    });
  }
  return appearance;
}
