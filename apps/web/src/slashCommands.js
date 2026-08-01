// @amadeus-header
// summary: Defines and filters slash commands available in the React and macOS composer.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - const: SLASH_COMMANDS
// - fn: commandDraft
// - fn: filterSlashCommands
// - fn: parseSlashInput
// uses: none
// invariants:
// - Only commands executable by the external client are advertised.
// - Completion activates only when slash is the first input character.
// side_effects: none
// tests:
// - apps/web/src/slashCommands.test.js
// @end-amadeus-header

export const SLASH_COMMANDS = [
  { name: "help", summary: "Show commands available in this app", icon: "help" },
  { name: "new-agent", summary: "Create and switch to a new agent session", argumentHint: "[name]", icon: "agent" },
  { name: "context", summary: "Show current token and session usage", icon: "context" },
  { name: "compact", summary: "Summarize older context and recover space", icon: "compact" },
  { name: "tools", summary: "Inspect the active tool catalog", icon: "tools" },
  { name: "prompt", summary: "Inspect the active model and prompt profile", icon: "prompt" },
  { name: "export", summary: "Download this conversation", argumentHint: "[markdown|json]", icon: "export" },
  { name: "settings", summary: "Open API connection settings", icon: "settings" },
  { name: "contribute", summary: "Open contribution resources", icon: "contribute" },
  { name: "cancel", summary: "Stop the active agent turn", icon: "cancel" },
  { name: "close", summary: "Close the current session", icon: "close" },
];

export function parseSlashInput(input = "") {
  if (!input.startsWith("/")) return null;
  const trimmed = input.slice(1).trimStart();
  const separator = trimmed.search(/\s/);
  const name = (separator < 0 ? trimmed : trimmed.slice(0, separator)).toLowerCase();
  const argument = separator < 0 ? "" : trimmed.slice(separator).trim();
  return { name, argument };
}

export function filterSlashCommands(input = "", commands = SLASH_COMMANDS) {
  const parsed = parseSlashInput(input);
  if (!parsed || parsed.argument || /\s/.test(input.slice(1))) return [];
  const query = parsed.name;
  if (!query) return commands;
  return commands.filter((command) => command.name.includes(query));
}

export function commandDraft(command) {
  return `/${command.name}${command.argumentHint ? " " : ""}`;
}
