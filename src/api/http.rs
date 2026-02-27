//! # HTTP Server
//!
//! Axum HTTP server setup and configuration for the REST API.

use axum::{
    routing::{get, post},
    Router,
};
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tokio::net::TcpListener;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::error::Result;
use crate::api::handlers::{chat, execute, health, stream, handle_task};
use crate::agent::supervisor::Supervisor;
use crate::client::LLMClient;

/// Shared application state.
pub struct AppState<C: LLMClient> {
    pub supervisor: Arc<Supervisor<C>>,
}

/// Run the HTTP server.
pub async fn run_server<C: LLMClient + Clone + 'static>(
    port: u16,
    supervisor: Arc<Supervisor<C>>,
) -> Result<()> {
    let state = Arc::new(AppState { supervisor });
    let app = create_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;

    println!("🚀 Server running at http://{}", addr);
    println!();
    println!("Endpoints:");
    println!("  GET  /health   - Health check");
    println!("  POST /chat     - Send message to agent");
    println!("  POST /execute  - Execute bash command");
    println!("  GET  /stream   - SSE streaming");
    println!("  POST /tasks    - Multi-agent task execution");
    println!();
    println!("Press Ctrl+C to stop");

    axum::serve(listener, app.into_make_service())
        .await
        .map_err(crate::error::AgentError::Io)?;

    Ok(())
}

/// Create the router with all routes and middleware.
pub fn create_router<C: LLMClient + Clone + 'static>(
    state: Arc<AppState<C>>,
) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/chat", post(chat))
        .route("/execute", post(execute))
        .route("/stream", get(stream))
        .route("/tasks", post(handle_task))
        .with_state(state)
        .layer(
            ServiceBuilder::new().layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            ),
        )
}
