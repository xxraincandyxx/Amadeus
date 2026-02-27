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
//! │  ├── GET  /health   → health::health                        │
//! │  ├── POST /chat     → chat::chat (Stateless via Supervisor) │
//! │  ├── POST /execute  → execute::execute (Direct Tool)        │
//! │  ├── GET  /stream   → stream::stream (SSE Updates)          │
//! │  └── POST /tasks    → tasks::handle_task (Multi-Agent)      │
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
    routing::{get, post},
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
use crate::api::handlers::{chat, execute, handle_task, health, stream};
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
    println!("Endpoints:");
    println!("  GET  /health   - Health check");
    println!("  POST /chat     - Send stateless message");
    println!("  POST /execute  - Execute bash command");
    println!("  GET  /stream   - SSE streaming");
    println!("  POST /tasks    - Multi-agent task dispatch");
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
        // Health check endpoint (Stateless)
        .route("/health", get(health))
        // Chat endpoint (Stateless wrapper around Supervisor)
        // POST /chat
        .route("/chat", post(chat))
        // Execute endpoint (Direct tool access)
        // POST /execute
        .route("/execute", post(execute))
        // Stream endpoint (SSE event stream)
        // GET /stream?message=...
        .route("/stream", get(stream))
        // Tasks endpoint (Multi-agent orchestration)
        // POST /tasks
        .route("/tasks", post(handle_task))
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
