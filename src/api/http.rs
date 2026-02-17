//! # HTTP Server
//!
//! Axum HTTP server setup and configuration for the REST API.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                        HTTP Server                           │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Router                                                      │
//! │  ├── GET  /health   → health::health                        │
//! │  ├── POST /chat     → chat::chat                            │
//! │  ├── POST /execute  → execute::execute                      │
//! │  └── GET  /stream   → stream::stream                        │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use claude_agent::api::http::run_server;
//!
//! #[tokio::main]
//! async fn main() {
//!     run_server(3000).await?;
//! }
//! ```
//!
//! ## CLI Usage
//!
//! ```bash
//! # Start on default port (3000)
//! cargo run -- --server
//!
//! # Start on custom port
//! cargo run -- --server 8080
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
// Json: JSON extractor/serializer
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
// ToSocketAddrs: Trait for resolving addresses
use tokio::net::TcpListener;

// Standard library types
use std::net::SocketAddr;

// Error type
use crate::error::Result;

// Handlers for each endpoint
use crate::api::handlers::{chat, execute, health, stream};

/*
 * ============================================================================
 * SERVER FUNCTIONS
 * ============================================================================
 */

/// Run the HTTP server.
///
/// Starts an Axum server on the specified port.
///
/// # Arguments
///
/// * `port` - Port number to listen on (e.g., 3000, 8080)
///
/// # Returns
///
/// Returns `Ok(())` on successful shutdown (rare) or an error.
///
/// # Example
///
/// ```rust,ignore
/// run_server(3000).await?;
/// ```
///
/// # Endpoints
///
/// | Path | Method | Handler | Description |
/// |------|--------|---------|-------------|
/// | `/health` | GET | `health` | Health check |
/// | `/chat` | POST | `chat` | Chat with agent |
/// | `/execute` | POST | `execute` | Execute command |
/// | `/stream` | GET | `stream` | SSE streaming |
pub async fn run_server(port: u16) -> Result<()> {
    // -------------------------------------------------------------------------
    // CREATE ROUTER
    // -------------------------------------------------------------------------

    // Build the router with all routes
    //
    // Router::new() creates an empty router
    // .route() adds a path with a handler
    let app = create_router();

    // -------------------------------------------------------------------------
    // BIND TO ADDRESS
    // -------------------------------------------------------------------------

    // Create the socket address
    //
    // 0.0.0.0 means listen on all network interfaces
    // This allows connections from localhost, LAN, and potentially WAN
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    // Bind a TCP listener to the address
    //
    // TcpListener::bind returns a future that resolves to the listener
    // .await waits for the bind to complete
    let listener = TcpListener::bind(addr).await?;

    // -------------------------------------------------------------------------
    // LOG STARTUP INFO
    // -------------------------------------------------------------------------

    // Print startup message to stdout
    //
    // This helps users know the server is running
    println!("🚀 Server running at http://{}", addr);
    println!();
    println!("Endpoints:");
    println!("  GET  /health   - Health check");
    println!("  POST /chat     - Send message to agent");
    println!("  POST /execute  - Execute bash command");
    println!("  GET  /stream   - SSE streaming");
    println!();
    println!("Press Ctrl+C to stop");

    // -------------------------------------------------------------------------
    // START SERVER
    // -------------------------------------------------------------------------

    // Start serving requests
    //
    // axum::serve takes:
    // - listener: The TCP listener
    // - app: The router (must be IntoMakeService for this API)
    //
    // .into_make_service() converts the router into a MakeService
    // which is required by serve()
    //
    // This runs forever until the process is killed (Ctrl+C)
    axum::serve(listener, app.into_make_service())
        .await
        .map_err(|e| crate::error::AgentError::Io(e))?;

    Ok(())
}

/// Create the router with all routes and middleware.
///
/// This function builds the complete router configuration including:
/// - Route definitions
/// - CORS middleware
/// - Request tracing (future)
///
/// # Returns
///
/// A configured `Router` ready to serve requests.
///
/// # Routes
///
/// ```text
/// GET  /health  → health::health
/// POST /chat    → chat::chat
/// POST /execute → execute::execute
/// GET  /stream  → stream::stream
/// ```
pub fn create_router() -> Router {
    // -------------------------------------------------------------------------
    // DEFINE ROUTES
    // -------------------------------------------------------------------------

    // Create the router with routes
    //
    // Each .route() call maps a path to a handler function:
    // - First arg: The URL path (e.g., "/health")
    // - Second arg: The route builder (get, post, etc.) with handler
    Router::new()
        // Health check endpoint
        //
        // GET /health
        // Returns: {"status": "ok", "version": "0.1.0"}
        .route("/health", get(health))
        // Chat endpoint
        //
        // POST /chat
        // Body: {"message": "...", "timeout_secs": 60}
        // Returns: {"content": "...", "tool_calls": [], "stop_reason": "..."}
        .route("/chat", post(chat))
        // Execute endpoint
        //
        // POST /execute
        // Body: {"command": "ls -la", "timeout_secs": 30}
        // Returns: {"output": "...", "exit_code": 0, "timed_out": false}
        .route("/execute", post(execute))
        // Stream endpoint (SSE)
        //
        // GET /stream?message=...
        // Returns: SSE stream of events
        .route("/stream", get(stream))
        // -------------------------------------------------------------------------
        // ADD MIDDLEWARE
        // -------------------------------------------------------------------------
        // Add CORS (Cross-Origin Resource Sharing) layer
        //
        // This allows web pages from different origins to call the API
        // Important for browser-based clients
        //
        // .allow_origin(Any): Allow requests from any origin
        // .allow_methods(Any): Allow any HTTP method
        // .allow_headers(Any): Allow any headers
        .layer(
            ServiceBuilder::new().layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            ),
        )
}

/*
 * ============================================================================
 * HELPER FUNCTIONS
 * ============================================================================
 */

/// Get the default port number.
///
/// Returns 3000, which is a common default for development servers.
#[allow(dead_code)]
pub fn default_port() -> u16 {
    3000
}

/*
 * ============================================================================
 * TESTS
 * ============================================================================
 */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_port() {
        assert_eq!(default_port(), 3000);
    }

    #[test]
    fn test_create_router() {
        // Router creation should succeed
        let _router = create_router();
    }
}
