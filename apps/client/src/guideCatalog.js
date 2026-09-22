// @amadeus-header
// summary: Defines the localized chapter catalog for the bundled Amadeus user guide.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - const: GUIDE_CHAPTERS
// - const: DEVELOPER_REFERENCES
// - fn: guideChapterLabel
// uses:
// - format: bilingual Markdown manuscripts
// invariants:
// - Chapter identifiers and source paths are stable across locales.
// - Every chapter has English and Simplified Chinese metadata.
// side_effects: none
// tests:
// - apps/client/src/guideCatalog.test.js
// @end-amadeus-header

export const GUIDE_CHAPTERS = [
  {
    id: "getting-started",
    icon: "start",
    title: { en: "Getting started", "zh-CN": "快速开始" },
    summary: { en: "Sessions, workspace layout, and a productive first request", "zh-CN": "会话、工作区布局和清晰的第一个请求" },
  },
  {
    id: "conversations",
    icon: "conversation",
    title: { en: "Conversations and context", "zh-CN": "对话与上下文" },
    summary: { en: "Thinking, Markdown, context usage, compaction, and export", "zh-CN": "思考、Markdown、上下文用量、压缩和导出" },
  },
  {
    id: "multi-agent",
    icon: "agents",
    title: { en: "Multi-agent work", "zh-CN": "多智能体协作" },
    summary: { en: "Coordinators, delegated sessions, ownership, and status", "zh-CN": "协调智能体、委派会话、职责和状态" },
  },
  {
    id: "tools-and-approvals",
    icon: "tools",
    title: { en: "Tools and approvals", "zh-CN": "工具与审批" },
    summary: { en: "Tool activity, file diffs, permissions, and cancellation", "zh-CN": "工具活动、文件差异、权限和取消操作" },
  },
  {
    id: "commands",
    icon: "commands",
    title: { en: "Commands and shortcuts", "zh-CN": "命令与快捷操作" },
    summary: { en: "Composer commands and keyboard navigation", "zh-CN": "输入框命令和键盘导航" },
  },
  {
    id: "settings-and-troubleshooting",
    icon: "settings",
    title: { en: "Settings and troubleshooting", "zh-CN": "设置与故障排查" },
    summary: { en: "Connection, language, and common recovery steps", "zh-CN": "连接、语言和常见恢复步骤" },
  },
];

export const DEVELOPER_REFERENCES = [
  { id: "architecture", label: "Architecture", path: "docs/ARCHITECTURE.md" },
  { id: "agent-architectures", label: "Agent architectures", path: "docs/AGENT_ARCHITECTURES.md" },
  { id: "http-api", label: "HTTP API", path: "docs/HTTP_API.md" },
  { id: "contributing", label: "Contributing", path: "CONTRIBUTING.md" },
];

export function guideChapterLabel(chapter, language) {
  const locale = language === "zh-CN" ? "zh-CN" : "en";
  return {
    title: chapter.title[locale],
    summary: chapter.summary[locale],
  };
}
