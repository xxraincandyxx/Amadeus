// @amadeus-header
// summary: Persists and applies the configurable web and native interface accent color.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - const: DEFAULT_THEME_COLOR
// - const: THEME_COLOR_PRESETS
// - fn: loadThemeColor
// - fn: applyThemeColor
// uses:
// - API: browser local storage
// - API: CSS custom properties
// invariants:
// - Stored theme colors use six-digit hexadecimal notation.
// - Invalid or missing values resolve to the dark-red default.
// side_effects:
// - Reads and writes browser local storage.
// - Updates document-root CSS variables.
// tests:
// - cmd: npm test
// @end-amadeus-header

export const THEME_COLOR_STORAGE_KEY = "amadeus.themeColor";
export const DEFAULT_THEME_COLOR = "#a72f42";

export const THEME_COLOR_PRESETS = [
  { id: "dark-red", color: DEFAULT_THEME_COLOR, label: "Dark red" },
  { id: "burnt-orange", color: "#c8662e", label: "Burnt orange" },
  { id: "cobalt", color: "#3977c5", label: "Cobalt" },
  { id: "forest", color: "#31815b", label: "Forest" },
  { id: "graphite", color: "#767676", label: "Graphite" },
];

export function normalizeThemeColor(value) {
  const normalized = String(value || "").trim().toLowerCase();
  return /^#[0-9a-f]{6}$/.test(normalized) ? normalized : DEFAULT_THEME_COLOR;
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
