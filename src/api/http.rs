//! # HTTP Server
//!
//! Axum HTTP server setup and configuration for the REST API.
//!
//! ## Architecture
//!
//! The server uses a shared `AppState` to provide handlers access to the
//! multi-agent `Supervisor`. This ensures that all API requests (stateless
//! chat or stateful tasks) are orchestrated by the same engine.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                        HTTP Server                          │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Router (Shared AppState)                                   │
//! │  ├── GET  /health         → health::health                  │
//! │  ├── POST /chat           → chat::chat                      │
//! │  ├── POST /execute        → execute::execute                │
//! │  ├── GET  /stream         → stream::stream (SSE)            │
//! │  ├── POST /tasks          → tasks::handle_task              │
//! │  ├── GET  /sessions       → sessions::list_sessions         │
//! │  ├── GET  /sessions/:id   → sessions::get_session           │
//! │  ├── POST /sessions/:id/restore → sessions::restore_session │
//! │  ├── GET  /config         → config::get_config              │
//! │  ├── PATCH /config        → config::update_config           │
//! │  ├── GET  /history        → history::get_history            │
//! │  ├── GET  /skills         → skills::list_skills             │
//! │  ├── GET  /approvals      → approvals::list_pending         │
//! │  └── POST /approvals/:id  → approvals::submit_approval      │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use amadeus::api::http::run_server;
//!
//! #[tokio::main]
//! async fn main() {
//!     let supervisor = Arc::new(Supervisor::new(...));
//!     run_server(3000, supervisor).await?;
//! }
//! ```

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

// Axum web framework types
//
// Router: The main router that maps paths to handlers
// routing: Module with route builders (get, post, etc.)
use axum::{
    routing::{get, patch, post},
    Router,
};

// Tower middleware
//
// ServiceBuilder: Builder for layering middleware
// CorsLayer: CORS (Cross-Origin Resource Sharing) support
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

// Tokio net types
//
// TcpListener: Listens for TCP connections
use tokio::net::TcpListener;

// Standard library types
use std::net::SocketAddr;
use std::sync::Arc;

// Internal dependencies
use crate::agent::supervisor::Supervisor;
use crate::api::handlers::{
    chat, execute, get_config, get_history, get_session, handle_task, health, list_pending_approvals,
    list_sessions, list_skills, restore_session, stream, submit_approval, update_config,
};
use crate::client::LLMClient;
use crate::error::Result;

/*
 * ============================================================================
 * STATE MANAGEMENT
 * ============================================================================
 */

/// Shared application state.
///
/// This struct is passed to every request handler via Axum's `State` extractor.
/// It provides access to the global `Supervisor`, which manages the worker pool.
pub struct AppState<C: LLMClient> {
    /// The multi-agent supervisor instance.
    pub supervisor: Arc<Supervisor<C>>,
}

/*
 * ============================================================================
 * SERVER FUNCTIONS
 * ============================================================================
 */

/// Run the HTTP server.
///
/// Starts an Axum server on the specified port, using the provided supervisor
/// for task orchestration.
///
/// # Arguments
///
/// * `port` - Port number to listen on (e.g., 3000, 8080)
/// * `supervisor` - Thread-safe reference to the Supervisor
///
/// # Returns
///
/// Returns `Ok(())` on successful shutdown or an error if the server fails to start.
///
/// # Example
///
/// ```rust,ignore
/// run_server(3000, supervisor).await?;
/// ```
///
/// # Endpoints
///
/// | Path | Method | Handler | Description |
/// |------|--------|---------|-------------|
/// | `/health` | GET | `health` | Health check |
/// | `/chat` | POST | `chat` | Stateless chat via supervisor |
/// | `/execute` | POST | `execute` | Direct bash command execution |
/// | `/stream` | GET | `stream` | SSE event streaming |
/// | `/tasks` | POST | `tasks` | Multi-agent task execution |
/// | `/sessions` | GET | `list_sessions` | List saved sessions |
/// | `/sessions/{id}` | GET | `get_session` | Get session details |
/// | `/sessions/{id}/restore` | POST | `restore_session` | Restore a session |
/// | `/config` | GET | `get_config` | Get current config |
/// | `/config` | PATCH | `update_config` | Update config settings |
/// | `/history` | GET | `get_history` | Get conversation history |
/// | `/skills` | GET | `list_skills` | List available skills |
/// | `/approvals` | GET | `list_pending_approvals` | List pending approvals |
/// | `/approvals/{id}` | POST | `submit_approval` | Submit approval decision |
pub async fn run_server<C: LLMClient + Clone + 'static>(
    port: u16,
    supervisor: Arc<Supervisor<C>>,
) -> Result<()> {
    // -------------------------------------------------------------------------
    // CREATE SHARED STATE
    // -------------------------------------------------------------------------
    let state = Arc::new(AppState { supervisor });

    // -------------------------------------------------------------------------
    // CREATE ROUTER
    // -------------------------------------------------------------------------
    let app = create_router(state);

    // -------------------------------------------------------------------------
    // BIND TO ADDRESS
    // -------------------------------------------------------------------------
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;

    // -------------------------------------------------------------------------
    // LOG STARTUP INFO
    // -------------------------------------------------------------------------
    println!("🚀 Amadeus Server running at http://{}", addr);
    println!();
    println!("Core Endpoints:");
    println!("  GET  /health   - Health check");
    println!("  POST /chat     - Send stateless message");
    println!("  POST /execute  - Execute bash command");
    println!("  GET  /stream   - SSE streaming");
    println!("  POST /tasks    - Multi-agent task dispatch");
    println!();
    println!("Session Management:");
    println!("  GET  /sessions           - List saved sessions");
    println!("  GET  /sessions/:id       - Get session details");
    println!("  POST /sessions/:id/restore - Restore a session");
    println!();
    println!("Configuration & Info:");
    println!("  GET  /config   - Get current configuration");
    println!("  PATCH /config  - Update configuration");
    println!("  GET  /history  - Get conversation history");
    println!("  GET  /skills   - List available skills");
    println!();
    println!("Approval Flow:");
    println!("  GET  /approvals       - List pending approvals");
    println!("  POST /approvals/:id   - Submit approval decision");
    println!();
    println!("Press Ctrl+C to stop");

    // -------------------------------------------------------------------------
    // START SERVER
    // -------------------------------------------------------------------------
    axum::serve(listener, app.into_make_service())
        .await
        .map_err(crate::error::AgentError::Io)?;

    Ok(())
}

/// Create the router with all routes and middleware.
///
/// This function builds the complete router configuration including:
/// - Route definitions mapped to handlers
/// - Shared state injection
/// - CORS middleware
///
/// # Arguments
///
/// * `state` - The shared application state to be injected into handlers
///
/// # Returns
///
/// A configured `Router` ready to serve requests.
pub fn create_router<C: LLMClient + Clone + 'static>(state: Arc<AppState<C>>) -> Router {
    Router::new()
        // =====================================================================
        // CORE ENDPOINTS
        // =====================================================================
        // Health check endpoint (Stateless)
        .route("/health", get(health))
        // Chat endpoint (Stateless wrapper around Supervisor)
        .route("/chat", post(chat))
        // Execute endpoint (Direct tool access)
        .route("/execute", post(execute))
        // Stream endpoint (SSE event stream)
        .route("/stream", get(stream))
        // Tasks endpoint (Multi-agent orchestration)
        .route("/tasks", post(handle_task))
        // =====================================================================
        // SESSION MANAGEMENT
        // =====================================================================
        // List all saved sessions
        .route("/sessions", get(list_sessions))
        // Get details of a specific session
        .route("/sessions/:id", get(get_session))
        // Restore a session into current history
        .route("/sessions/:id/restore", post(restore_session))
        // =====================================================================
        // CONFIGURATION
        // =====================================================================
        // Get current configuration
        .route("/config", get(get_config))
        // Update configuration settings
        .route("/config", patch(update_config))
        // =====================================================================
        // INFO ENDPOINTS
        // =====================================================================
        // Get conversation history
        .route("/history", get(get_history))
        // List available skills
        .route("/skills", get(list_skills))
        // =====================================================================
        // APPROVAL FLOW
        // =====================================================================
        // List pending approvals
        .route("/approvals", get(list_pending_approvals))
        // Submit approval decision
        .route("/approvals/:id", post(submit_approval))
        // Inject shared state into all handlers
        .with_state(state)
        // Add middleware layer
        .layer(
            ServiceBuilder::new().layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            ),
        )
}
