// @amadeus-header
// summary: Provides English and Simplified Chinese translations for the web and native client.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - const: SUPPORTED_LANGUAGES
// - fn: normalizeLanguage
// - fn: translate
// uses:
// - format: ICU-like named placeholders
// invariants:
// - English source keys remain the fallback for missing translations.
// - Supported language identifiers match BCP 47 values stored by the client.
// side_effects: none
// tests:
// - apps/web/src/i18n.test.js
// @end-amadeus-header

export const SUPPORTED_LANGUAGES = [
  { value: "en", label: "English" },
  { value: "zh-CN", label: "简体中文" },
];

const ZH_CN = {
  "New session": "新建会话",
  "Main Agent": "主智能体",
  "Session {number}": "会话 {number}",
  Agents: "智能体",
  Tools: "工具",
  Tool: "工具",
  Contribute: "参与贡献",
  Workspace: "工作区",
  "Local API connected": "本地 API 已连接",
  "API unavailable": "API 不可用",
  "Connection settings": "连接设置",
  Primary: "主导航",
  "Close sidebar": "关闭侧边栏",
  "Open sidebar": "打开侧边栏",
  "agent workspace": "智能体工作区",
  Details: "详情",
  "Close session": "关闭会话",
  idle: "空闲",
  running: "运行中",
  completed: "已完成",
  failed: "失败",
  "Needs approval": "需要批准",
  You: "你",
  Thought: "思考",
  "Thought for {seconds} second": "思考了 {seconds} 秒",
  "Thought for {seconds} seconds": "思考了 {seconds} 秒",
  "Reasoning unavailable": "思考过程不可用",
  Running: "运行中",
  Failed: "失败",
  Completed: "已完成",
  input: "输入",
  output: "输出",
  "Permission required": "需要权限",
  "This tool needs your approval before it can continue.": "此工具需要你的批准才能继续。",
  Deny: "拒绝",
  "Always allow": "始终允许",
  "Allow once": "允许一次",
  Commands: "命令",
  "Navigate, select, or close": "方向键导航，回车选择，Esc 关闭",
  "Slash commands": "斜杠命令",
  "Show commands available in this app": "显示此应用中的可用命令",
  "Create and switch to a new agent session": "创建并切换到新的智能体会话",
  "Show current token and session usage": "显示当前令牌和会话用量",
  "Summarize older context and recover space": "总结较早的上下文并释放空间",
  "Inspect the active tool catalog": "查看当前工具目录",
  "Inspect the active model and prompt profile": "查看当前模型和提示词配置档",
  "Download this conversation": "下载此对话",
  "Open API connection settings": "打开 API 连接设置",
  "Open contribution resources": "打开贡献资源",
  "Stop the active agent turn": "停止当前智能体任务",
  "Close the current session": "关闭当前会话",
  "Message Amadeus": "给 Amadeus 发消息",
  "Connect to the local Amadeus API to begin": "连接本地 Amadeus API 后开始",
  "Ask Amadeus to inspect, explain, or build anything": "让 Amadeus 检查、解释或构建任何内容",
  "Add context": "添加上下文",
  "Local full access": "本地完全访问",
  "{percent}% context": "上下文 {percent}%",
  "Default agent": "默认智能体",
  "Stop generation": "停止生成",
  "Send message": "发送消息",
  "Enter to send · Shift + Enter for a new line": "回车发送 · Shift + 回车换行",
  "What should we work on?": "我们要做什么？",
  "{name} can inspect your project, execute tools, and keep the entire conversation in this session.": "{name} 可以检查项目、执行工具，并在此会话中保留完整对话。",
  "Explore the codebase": "探索代码库",
  "Map architecture and important flows": "梳理架构和重要流程",
  "Build a feature": "构建功能",
  "Plan, implement, test, and verify": "规划、实现、测试并验证",
  "Session details": "会话详情",
  Status: "状态",
  Profile: "配置档",
  "Session ID": "会话 ID",
  Messages: "消息",
  "Tool calls": "工具调用",
  "Input tokens": "输入令牌",
  "Output tokens": "输出令牌",
  "Start with a clean conversation and an independent agent context.": "以全新对话和独立智能体上下文开始。",
  "Session name": "会话名称",
  "Feature implementation": "功能实现",
  "The Amadeus API is unavailable.": "Amadeus API 不可用。",
  Cancel: "取消",
  "Creating…": "正在创建…",
  "Create session": "创建会话",
  Connection: "连接",
  "Choose the Amadeus HTTP server used by this client.": "选择此客户端使用的 Amadeus HTTP 服务器。",
  Connected: "已连接",
  "Not connected": "未连接",
  "HTTP API URL": "HTTP API 地址",
  "Remote servers should use HTTPS and authentication at the network boundary.": "远程服务器应使用 HTTPS，并在网络边界进行身份验证。",
  Language: "语言",
  "Interface language": "界面语言",
  "Reset default": "恢复默认",
  "Testing…": "正在测试…",
  Test: "测试",
  "Save and reconnect": "保存并重新连接",
  "Build Amadeus with us": "与我们一起构建 Amadeus",
  "Every contribution should leave the product clearer, more useful, and more coherent.": "每一次贡献都应让产品更清晰、更实用、更一致。",
  "Preserve one visual language": "保持统一的视觉语言",
  "Ship every interaction state": "完整交付每种交互状态",
  "Verify desktop and mobile": "验证桌面端和移动端",
  "Contribution guide": "贡献指南",
  "Setup, change scope, checks, and pull requests": "环境设置、变更范围、检查和拉取请求",
  "Interface design system": "界面设计系统",
  "Tokens, component rules, states, and visual QA": "设计令牌、组件规则、状态和视觉质检",
  "HTTP API contract": "HTTP API 契约",
  "External endpoints, stability, and availability": "外部端点、稳定性和可用性",
  Close: "关闭",
  "Open repository": "打开代码仓库",
  Retry: "重试",
  Settings: "设置",
  Dismiss: "关闭",
  "Amadeus is working": "Amadeus 正在工作",
  "No open sessions": "没有打开的会话",
  "Amadeus API is offline": "Amadeus API 已离线",
  "Create a session to begin working with an agent.": "创建会话以开始与智能体协作。",
  "Start the server at {url}, then refresh this page.": "在 {url} 启动服务器，然后刷新此页面。",
  "Connection successful.": "连接成功。",
  "Use an HTTP or HTTPS URL.": "请使用 HTTP 或 HTTPS 地址。",
  "Restored the default local address. Save to reconnect.": "已恢复默认本地地址。保存后将重新连接。",
  "Connect to the Amadeus API before creating a session.": "创建会话前请先连接 Amadeus API。",
  "Live connection interrupted. Amadeus will retry automatically.": "实时连接已中断。Amadeus 将自动重试。",
  "Amadeus API is unavailable. Start the server, then retry.": "Amadeus API 不可用。请启动服务器后重试。",
};

export function normalizeLanguage(value = "") {
  return String(value).toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}

export function translate(language, key, variables = {}) {
  const template = normalizeLanguage(language) === "zh-CN" ? ZH_CN[key] || key : key;
  return template.replace(/\{(\w+)\}/g, (match, name) => String(variables[name] ?? match));
}
