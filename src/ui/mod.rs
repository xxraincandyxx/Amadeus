//! # UI Module
//!
//! Terminal user interface components.
//!
//! ## Components
//!
//! - **`colors`**: Color palette and theme (Dracula-inspired)
//! - **`app`**: Main application state and event loop
//! - **`event`**: Keyboard and mouse event handling
//! - **`components`**: UI components (input, messages, sidebar, status)
//! - **`repl`**: Legacy REPL (kept for backward compatibility)

pub mod app;
pub mod colors;
pub mod components;
pub mod event;
pub mod repl;

pub use app::App;
pub use colors::{Theme, THEME};
pub use event::{AppEvent, EventHandler};
pub use repl::Repl;

pub use components::{
    AppState, FileSidebar, HelpSidebar, InputComponent, MessagesComponent, Sidebar, SidebarKind,
    StatusBar, ToolPanel,
};
