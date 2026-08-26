// @amadeus-header
// summary: Verifies web theme color normalization, persistence, and CSS variable generation.
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
  applyThemeColor,
  DEFAULT_THEME_COLOR,
  loadThemeColor,
  normalizeThemeColor,
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
