// @amadeus-header
// summary: Locale selection and English and Simplified Chinese interface message catalog.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - fn: crate::ui::i18n::command_summary
// - fn: crate::ui::i18n::language
// - fn: crate::ui::i18n::set_language
// - fn: crate::ui::i18n::text
// - fn: crate::ui::i18n::thought_label
// - fn: crate::ui::i18n::thought_summary
// uses:
// - type: crate::Language
// invariants:
// - English is the fallback locale for unsupported or untranslated interface text.
// side_effects:
// - Updates process-wide TUI locale state.
// tests:
// - cmd: cargo test -p tui i18n
// @end-amadeus-header

use std::sync::atomic::{AtomicU8, Ordering};

use crate::Language;

static LANGUAGE: AtomicU8 = AtomicU8::new(0);

pub(crate) fn set_language(language: Language) {
    LANGUAGE.store(
        match language {
            Language::English => 0,
            Language::ChineseSimplified => 1,
        },
        Ordering::Relaxed,
    );
}

pub(crate) fn language() -> Language {
    match LANGUAGE.load(Ordering::Relaxed) {
        1 => Language::ChineseSimplified,
        _ => Language::English,
    }
}

pub(crate) fn text(key: &str) -> &'static str {
    localized_text(language(), key)
}

fn localized_text(language: Language, key: &str) -> &'static str {
    if language == Language::ChineseSimplified {
        match key {
            "approval.title" => " 需要批准 ",
            "approval.tool" => "工具：",
            "approval.reason" => "原因：",
            "approval.input" => "输入：",
            "approval.approve" => "批准",
            "approval.deny" => "拒绝",
            "approval.always" => "始终批准",
            "approval.help" => "↑/↓：选择  y：是  n：否  a：始终  Esc：取消",
            "approval.help_confirm" => "↑/↓：选择  Enter：确认  Esc：取消",
            "status.thinking" => "思考中",
            "status.generating" => "生成中",
            "status.working" => "工作中",
            "status.responding" => "响应中",
            "status.awaiting_approval" => "等待批准",
            "input.placeholder" => "试试 \"src/main.rs 是如何工作的？\"",
            "input.shell_hint" => "  ! 进入终端模式 · 退格键退出",
            "input.shortcuts_hint" => "  ? 查看快捷键",
            "sidebar.explorer" => " 资源管理器 ",
            "sidebar.commands" => " 命令 ",
            "sidebar.skills" => " 技能 ",
            "sidebar.shortcuts" => "快捷键",
            "sidebar.sidebar" => "侧边栏",
            "sidebar.tools" => "工具",
            "sidebar.themes" => "主题",
            "sidebar.context" => "上下文",
            "sidebar.viewport" => "实时视图",
            "sidebar.scrolling" => "滚动",
            "sidebar.system" => "系统",
            "sidebar.send" => " 发送",
            "sidebar.new_line" => " 换行",
            "sidebar.history" => " 历史记录",
            "sidebar.files" => " 文件",
            "sidebar.help" => " 帮助",
            "sidebar.expand_tools" => " 展开工具",
            "sidebar.switch_theme" => " 切换主题",
            "sidebar.compact_history" => " 压缩历史",
            "sidebar.collapse" => " 收起",
            "sidebar.exit" => " 退出",
            "sidebar.no_skills" => "   暂无可用技能",
            "sidebar.add_skills" => "   将 .md 文件添加到",
            "language.current" => "当前界面语言",
            "language.changed" => "界面语言已切换为",
            "language.invalid" => "不支持的语言。可用语言：en、zh-CN",
            "language.english" => "English",
            "language.chinese" => "简体中文",
            "help.title" => "**可用命令**",
            "help.btw" => "显示 `/btw` 用法",
            "help.help" => "显示此帮助信息",
            "help.compact" => "强制压缩上下文",
            "help.context" => "显示当前上下文用量",
            "help.tools" => "查看当前工具目录",
            "help.prompt" => "查看当前提示词配置",
            "help.hooks" => "查看已配置的钩子阶段",
            "help.new_agent" => "创建新的代理会话",
            "help.language" => "显示或切换界面语言",
            "help.rewind" => "恢复到之前的本地检查点",
            "help.export" => "导出对话",
            "help.viewport" => "显示或隐藏实时视图",
            "help.exit" => "退出",
            "help.shell" => "终端模式，直接执行命令",
            _ => english_text(key),
        }
    } else {
        english_text(key)
    }
}

fn english_text(key: &str) -> &'static str {
    match key {
        "approval.title" => " Approval Required ",
        "approval.tool" => "Tool: ",
        "approval.reason" => "Reason: ",
        "approval.input" => "Input:",
        "approval.approve" => "Approve",
        "approval.deny" => "Deny",
        "approval.always" => "Always Approve",
        "approval.help" => "↑/↓: Select  y: Yes  n: No  a: Always  Esc: Cancel",
        "approval.help_confirm" => "↑/↓: Select  Enter: Confirm  Esc: Cancel",
        "status.thinking" => "thinking",
        "status.generating" => "generating",
        "status.working" => "working",
        "status.responding" => "responding",
        "status.awaiting_approval" => "awaiting approval",
        "input.placeholder" => "Try \"how does src/main.rs work?\"",
        "input.shell_hint" => "  ! for shell mode · backspace to exit",
        "input.shortcuts_hint" => "  ? for shortcuts",
        "sidebar.explorer" => " EXPLORER ",
        "sidebar.commands" => " COMMANDS ",
        "sidebar.skills" => " SKILLS ",
        "sidebar.shortcuts" => "SHORTCUTS",
        "sidebar.sidebar" => "SIDEBAR",
        "sidebar.tools" => "TOOLS",
        "sidebar.themes" => "THEMES",
        "sidebar.context" => "CONTEXT",
        "sidebar.viewport" => "LIVE VIEWPORT",
        "sidebar.scrolling" => "SCROLLING",
        "sidebar.system" => "SYSTEM",
        "sidebar.send" => " Send",
        "sidebar.new_line" => " New Line",
        "sidebar.history" => " History",
        "sidebar.files" => " Files",
        "sidebar.help" => " Help",
        "sidebar.expand_tools" => " Expand Tools",
        "sidebar.switch_theme" => " Switch Theme",
        "sidebar.compact_history" => " Compact History",
        "sidebar.collapse" => " Collapse",
        "sidebar.exit" => " Exit",
        "sidebar.no_skills" => "   No skills available",
        "sidebar.add_skills" => "   Add .md files to",
        "language.current" => "Current interface language",
        "language.changed" => "Interface language changed to",
        "language.invalid" => "Unsupported language. Available languages: en, zh-CN",
        "language.english" => "English",
        "language.chinese" => "Simplified Chinese",
        "help.title" => "**Available Commands**",
        "help.btw" => "Show `/btw` usage",
        "help.help" => "Show this help message",
        "help.compact" => "Force context compaction",
        "help.context" => "Show current context usage",
        "help.tools" => "Inspect active tool catalog",
        "help.prompt" => "Inspect active prompt profile",
        "help.hooks" => "Inspect configured hook phases",
        "help.new_agent" => "Spawn new agent session",
        "help.language" => "Show or change the interface language",
        "help.rewind" => "Restore an earlier local checkpoint",
        "help.export" => "Export the conversation",
        "help.viewport" => "Show or hide the live viewport",
        "help.exit" => "Quit",
        "help.shell" => "Shell mode, execute shell commands directly",
        _ => "",
    }
}

pub(crate) fn command_summary(name: &str, fallback: &'static str) -> &'static str {
    if language() != Language::ChineseSimplified {
        return fallback;
    }
    match name {
        "btw" => "提出不写入对话历史的旁支问题",
        "help" => "显示可用命令",
        "compact" => "触发上下文压缩",
        "context" => "显示当前上下文用量",
        "tools" => "查看当前工具目录",
        "prompt" => "查看当前提示词配置",
        "hooks" => "查看已配置的钩子阶段",
        "new-agent" => "创建新的代理会话",
        "language" => "显示或切换界面语言（en | zh-CN）",
        "rewind" => "恢复到之前的检查点",
        "export" => "导出当前对话",
        "viewport" => "显示或隐藏实时视图",
        "exit" => "退出当前 TUI 会话",
        _ => fallback,
    }
}

pub(crate) fn language_name(value: Language) -> &'static str {
    match value {
        Language::English => text("language.english"),
        Language::ChineseSimplified => text("language.chinese"),
    }
}

pub(crate) fn help_text() -> String {
    [
        text("help.title").to_string(),
        format!("- `/btw`: {}", text("help.btw")),
        format!("- `/help`: {}", text("help.help")),
        format!("- `/compact` or `/compress`: {}", text("help.compact")),
        format!("- `/context`: {}", text("help.context")),
        format!("- `/tools`: {}", text("help.tools")),
        format!("- `/prompt`: {}", text("help.prompt")),
        format!("- `/hooks`: {}", text("help.hooks")),
        format!("- `/new-agent`: {}", text("help.new_agent")),
        format!("- `/language [en|zh-CN]`: {}", text("help.language")),
        format!("- `/rewind`: {}", text("help.rewind")),
        format!("- `/export [path|json]`: {}", text("help.export")),
        format!(
            "- `/viewport [hidden|auto|always]`: {}",
            text("help.viewport")
        ),
        format!("- `/exit`: {}", text("help.exit")),
        format!("- `!`: {}", text("help.shell")),
        "- `Ctrl+C` / `Esc`: Cancel active stream".to_string(),
        "- `Tab` / `Shift+Tab`: Switch sessions".to_string(),
        "- `Ctrl+]` / `Ctrl[`: Switch to child / parent session".to_string(),
        "- `Ctrl+Backspace`: Close active session".to_string(),
    ]
    .join("\n")
}

pub(crate) fn thought_summary(seconds: u64) -> String {
    if language() == Language::ChineseSimplified {
        format!("思考了 {seconds} 秒")
    } else if seconds == 1 {
        "Thought for 1 second".to_string()
    } else {
        format!("Thought for {seconds} seconds")
    }
}

pub(crate) fn thought_label() -> &'static str {
    match language() {
        Language::English => "Thought",
        Language::ChineseSimplified => "思考",
    }
}

#[cfg(test)]
mod tests {
    use super::localized_text;
    use crate::Language;

    #[test]
    fn i18n_switches_between_supported_languages() {
        assert_eq!(
            localized_text(Language::ChineseSimplified, "status.thinking"),
            "思考中"
        );
        assert_eq!(
            localized_text(Language::English, "status.thinking"),
            "thinking"
        );
    }
}
