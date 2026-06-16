// @amadeus-header
// summary: Module root for the ui subsystem and its exports.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - tests/mod.rs
// @end-amadeus-header

//! # UI Module
//!
//! Terminal user interface components with gemini-cli-inspired theming.
//!
//! ## Theming System
//!
//! - **`semantic_colors`**: Semantic color definitions (text, background, border, ui, status)
//! - **`themes`**: Built-in theme collection (Dracula, GitHub, Solarized, etc.)
//! - **`theme_manager`**: Dynamic theme switching and management
//!
//! ## Scrolling System
//!
//! - **`scroll`**: Scroll state management and animated scrollbars
//!
//! ## Components
//!
//! - **`colors`**: Legacy color utilities (backward compatible)
//! - **`app`**: Main application state and event loop
//! - **`event`**: Keyboard and mouse event handling
//! - **`components`**: UI components (input, messages, sidebar, status)
//! - **`repl`**: Legacy REPL (kept for backward compatibility)

pub mod api_client;
pub mod app;
pub mod colors;
pub mod components;
pub mod constants;
pub mod event;
pub mod export;
pub mod repl;
pub mod scroll;
pub mod semantic_colors;
pub mod theme_manager;
pub mod themes;

pub use api_client::ApiClient;
pub use app::App;
pub use event::{AppEvent, EventHandler};
pub use repl::Repl;

pub use components::{
    ContextInfo, FileSidebar, Footer, FooterInfo, GeminiSpinner, HelpSidebar, InputComponent,
    LoadingIndicator, MessagesComponent, PhraseCycler, PhraseMode, SandboxStatus, Sidebar,
    SidebarKind, StreamingState,
};

pub use semantic_colors::SemanticColors;
pub use theme_manager::{
    get_available_themes, get_colors, get_theme, next_theme, previous_theme, set_theme,
    THEME_MANAGER,
};

#[deprecated(note = "Use get_colors() from theme_manager for semantic colors")]
pub use colors::THEME;
