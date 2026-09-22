// @amadeus-header
// summary: Verifies side-panel width clamping and pointer-delta calculations.
// layer: test
// status: active
// feature_flags: none
// provides:
// - test: panel resize calculations
// uses:
// - module: apps/client/src/panelResize.js
// invariants:
// - Invalid and out-of-range panel widths resolve to safe bounds.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

import assert from "node:assert/strict";
import test from "node:test";

import { clampPanelWidth, resizedPanelWidth } from "./panelResize.js";

test("panel widths are clamped to configured bounds", () => {
  assert.equal(clampPanelWidth(180, 220, 440), 220);
  assert.equal(clampPanelWidth(310, 220, 440), 310);
  assert.equal(clampPanelWidth(520, 220, 440), 440);
  assert.equal(clampPanelWidth("invalid", 220, 440), 220);
});

test("pointer deltas resize from the drag starting width", () => {
  assert.equal(resizedPanelWidth(292, 48, 220, 440), 340);
  assert.equal(resizedPanelWidth(292, -200, 220, 440), 220);
  assert.equal(resizedPanelWidth(420, 100, 220, 440), 440);
});
