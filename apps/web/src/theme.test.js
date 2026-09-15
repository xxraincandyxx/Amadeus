// @amadeus-header
// summary: Verifies web theme color and glass-surface persistence and CSS variable generation.
// layer: test
// status: active
// feature_flags: none
// provides:
// - tests: theme color behavior
// uses:
// - module: apps/web/src/theme.js
// invariants:
// - Invalid stored colors always fall back to dark red.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

import assert from "node:assert/strict";
import test from "node:test";

import {
  applySurfaceAppearance,
  applyThemeColor,
  DEFAULT_SURFACE_APPEARANCE,
  DEFAULT_THEME_COLOR,
  loadSurfaceAppearance,
  loadThemeColor,
  normalizeThemeColor,
  normalizeSurfaceAppearance,
  surfaceAppearanceVariables,
  themeColorVariables,
} from "./theme.js";

test("theme colors default to dark red and normalize valid hex", () => {
  assert.equal(normalizeThemeColor("invalid"), DEFAULT_THEME_COLOR);
  assert.equal(normalizeThemeColor(" #3977C5 "), "#3977c5");
  assert.equal(loadThemeColor({ getItem: () => null }), DEFAULT_THEME_COLOR);
});

test("theme variables expose matching solid and translucent colors", () => {
  assert.deepEqual(themeColorVariables("#3977c5"), {
    "--accent": "#3977c5",
    "--accent-rgb": "57, 119, 197",
    "--accent-soft": "rgba(57, 119, 197, 0.14)",
  });
});

test("applying a theme persists it and updates the root properties", () => {
  const stored = new Map();
  const properties = new Map();
  const color = applyThemeColor(
    "#31815b",
    { setItem: (key, value) => stored.set(key, value) },
    { style: { setProperty: (key, value) => properties.set(key, value) } },
  );

  assert.equal(color, "#31815b");
  assert.equal(stored.get("amadeus.themeColor"), "#31815b");
  assert.equal(properties.get("--accent-rgb"), "49, 129, 91");
});

test("surface appearance defaults keep the sidebar glassy and the main page opaque", () => {
  assert.deepEqual(loadSurfaceAppearance({ getItem: () => null }), DEFAULT_SURFACE_APPEARANCE);
  assert.deepEqual(loadSurfaceAppearance({ getItem: () => "not json" }), DEFAULT_SURFACE_APPEARANCE);
});

test("surface opacity normalization rounds and clamps stored values", () => {
  assert.deepEqual(normalizeSurfaceAppearance({ sidebarOpacity: 42.6, mainOpacity: 120 }), {
    sidebarOpacity: 43,
    mainOpacity: 100,
  });
  assert.deepEqual(normalizeSurfaceAppearance({ sidebarOpacity: -10 }), {
    sidebarOpacity: 0,
    mainOpacity: 100,
  });
});

test("applying surface appearance persists values and updates opacity variables", () => {
  const stored = new Map();
  const properties = new Map();
  const appearance = applySurfaceAppearance(
    { sidebarOpacity: 74, mainOpacity: 86 },
    { setItem: (key, value) => stored.set(key, value) },
    { style: { setProperty: (key, value) => properties.set(key, value) } },
  );

  assert.deepEqual(appearance, { sidebarOpacity: 74, mainOpacity: 86 });
  assert.equal(stored.get("amadeus.surfaceAppearance.v1"), '{"sidebarOpacity":74,"mainOpacity":86}');
  assert.deepEqual(surfaceAppearanceVariables(appearance), {
    "--sidebar-opacity": "74%",
    "--main-page-opacity": "86%",
  });
  assert.equal(properties.get("--main-page-opacity"), "86%");
});
