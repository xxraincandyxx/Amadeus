// @amadeus-header
// summary: Module root for the components subsystem and its exports.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::components
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - tests/mod.rs
// @end-amadeus-header

mod approval;
mod compaction_animation;
mod completion;
mod diff;
mod footer;
mod input;
mod loading_indicator;
mod markdown;
mod messages;
mod phrase_cycler;
pub mod sessions;
mod sidebar;
mod spinner;
mod status;
mod status_bar;
mod tool_group;

pub use approval::{ApprovalDialog, ApprovalResponse};
pub use compaction_animation::CompactionAnimator;
pub use diff::{DiffLine, DiffView, DiffView as DiffRenderer};
pub use footer::{Footer, FooterInfo, SandboxStatus};
pub use input::InputComponent;
pub use loading_indicator::{LoadingIndicator, StreamingState};
pub use markdown::render_markdown;
pub use messages::{CompressionItem, CompressionStatus, HistoryItem, MessagesComponent};
pub use phrase_cycler::{PhraseCycler, PhraseMode};
pub use sessions::{SessionBrowser, SessionMetadata};
pub use sidebar::{ContextInfo, FileSidebar, HelpSidebar, Sidebar, SidebarKind, SkillSidebar};
pub use spinner::GeminiSpinner;
pub use status_bar::StatusBar;
pub use tool_group::{ToolCall, ToolGroup, ToolStatus};
