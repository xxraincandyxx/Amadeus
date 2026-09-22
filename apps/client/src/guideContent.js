// @amadeus-header
// summary: Bundles localized user-guide Markdown manuscripts into the web and native app.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - const: GUIDE_CONTENT
// uses:
// - module: docs/user-guide
// - bundler: Vite raw imports
// invariants:
// - Guide content is available offline in production bundles.
// side_effects: none
// tests:
// - apps/client/src/guideCatalog.test.js
// @end-amadeus-header

import enCommands from "../../../docs/user-guide/en/commands.md?raw";
import enConversations from "../../../docs/user-guide/en/conversations.md?raw";
import enGettingStarted from "../../../docs/user-guide/en/getting-started.md?raw";
import enMultiAgent from "../../../docs/user-guide/en/multi-agent.md?raw";
import enSettings from "../../../docs/user-guide/en/settings-and-troubleshooting.md?raw";
import enTools from "../../../docs/user-guide/en/tools-and-approvals.md?raw";
import zhCommands from "../../../docs/user-guide/zh-CN/commands.md?raw";
import zhConversations from "../../../docs/user-guide/zh-CN/conversations.md?raw";
import zhGettingStarted from "../../../docs/user-guide/zh-CN/getting-started.md?raw";
import zhMultiAgent from "../../../docs/user-guide/zh-CN/multi-agent.md?raw";
import zhSettings from "../../../docs/user-guide/zh-CN/settings-and-troubleshooting.md?raw";
import zhTools from "../../../docs/user-guide/zh-CN/tools-and-approvals.md?raw";

export const GUIDE_CONTENT = {
  en: {
    "getting-started": enGettingStarted,
    conversations: enConversations,
    "multi-agent": enMultiAgent,
    "tools-and-approvals": enTools,
    commands: enCommands,
    "settings-and-troubleshooting": enSettings,
  },
  "zh-CN": {
    "getting-started": zhGettingStarted,
    conversations: zhConversations,
    "multi-agent": zhMultiAgent,
    "tools-and-approvals": zhTools,
    commands: zhCommands,
    "settings-and-troubleshooting": zhSettings,
  },
};
