// @amadeus-header
// summary: Verifies slash command parsing, filtering, and completion behavior.
// layer: test
// status: test-only
// feature_flags: none
// provides:
// - cmd: npm test
// uses:
// - module: apps/web/src/slashCommands.js
// invariants:
// - Slash completion never activates away from the first input character.
// - Argument commands retain a trailing space after completion.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

import assert from "node:assert/strict";
import test from "node:test";

import { commandDraft, filterSlashCommands, parseSlashInput, SLASH_COMMANDS } from "./slashCommands.js";

test("slash completion activates only at the first character", () => {
  assert.equal(filterSlashCommands("explain /help").length, 0);
  assert.equal(filterSlashCommands(" /help").length, 0);
  assert.equal(filterSlashCommands("/").length, SLASH_COMMANDS.length);
});

test("slash completion filters command names without description noise", () => {
  assert.deepEqual(filterSlashCommands("/new").map((command) => command.name), ["new-agent"]);
  assert.deepEqual(filterSlashCommands("/to").map((command) => command.name), ["tools"]);
  assert.deepEqual(filterSlashCommands("/token"), []);
});

test("slash completion closes after an argument starts", () => {
  assert.deepEqual(filterSlashCommands("/export json"), []);
  assert.deepEqual(parseSlashInput("/export json"), { name: "export", argument: "json" });
});

test("argument commands complete with a trailing space", () => {
  const command = SLASH_COMMANDS.find((candidate) => candidate.name === "new-agent");
  assert.equal(commandDraft(command), "/new-agent ");
  assert.equal(commandDraft(SLASH_COMMANDS[0]), "/help");
});
