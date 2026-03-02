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

pub mod app;
pub mod colors;
pub mod components;
pub mod constants;
pub mod event;
pub mod repl;
pub mod scroll;
pub mod semantic_colors;
pub mod themes;
pub mod theme_manager;

pub use app::App;
pub use event::{AppEvent, EventHandler};
pub use repl::Repl;

pub use components::{
    AppState, FileSidebar, Footer, FooterInfo, GeminiSpinner, HelpSidebar, InputComponent,
    LoadingIndicator, MessagesComponent, PhraseCycler, PhraseMode, SandboxStatus, Sidebar,
    SidebarKind, StatusBar, StreamingState,
};

pub use semantic_colors::SemanticColors;
pub use theme_manager::{get_theme, get_colors, set_theme, next_theme, previous_theme, get_available_themes, THEME_MANAGER};

#[deprecated(note = "Use get_colors() from theme_manager for semantic colors")]
pub use colors::THEME;
