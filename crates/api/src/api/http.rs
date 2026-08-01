// @amadeus-header
// summary: HTTP server bootstrap and router assembly for the public API.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::http
// - type: crate::api::http::AppState
// - fn: crate::api::http::run_server
// - fn: crate::api::http::create_router
// - route: /health
// - route: /chat
// - route: /execute
// - route: /v1/sessions
// - route: /v1/sessions/:id/messages
// - route: /v1/sessions/:id/events
// - route: /v1/sessions/:id/approvals/:approval_id
// - route: /v1/sessions/:id/checkpoint
// uses:
// - module: crate::agent::config::Config
// - module: crate::agent::orchestra
// - module: crate::client::LLMClient
// - module: crate::error::Result
// - runtime: tokio async runtime
// - protocol: axum HTTP handlers
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Performs network or HTTP operations.
// - Writes output to stdout or stderr.
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

//! # HTTP Server
//!
//! Axum HTTP server setup and configuration for the REST API.
//!
//! ## Architecture
//!
//! The server uses a shared `AppState` to provide handlers access to the
//! core client, configuration, and orchestra-aware agent orchestrator.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                        HTTP Server                          │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Router (Shared AppState)                                   │
//! │  ├── GET  /health         → health::health                  │
//! │  ├── POST /chat           → chat::chat                      │
//! │  ├── POST /execute        → execute::execute                │
//! │  ├── /v1/sessions/*       → stateful external protocol      │
//! │  ├── POST /tasks          → tasks::handle_task              │
//! │  ├── GET  /sessions       → sessions::list_sessions         │
//! │  ├── GET  /sessions/:id   → sessions::get_session           │
//! │  ├── POST /sessions/:id/restore → sessions::restore_session │
//! │  ├── GET  /config         → config::get_config              │
//! │  ├── PATCH /config        → config::update_config           │
//! │  ├── GET  /skills         → skills::list_skills             │
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
//!     run_server(3000, client, config).await?;
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
    routing::{delete, get, patch, post},
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
use crate::agent::config::Config;
use crate::agent::orchestra::{AgentOrchestrator, OrchestraLeader};
use crate::agent::profile::AgentProfile;
use crate::api::handlers::{
    build_prompt, cancel_external_session, chat, close_external_session, compact_external_session,
    create_external_session, delete_entry, execute, external_session_checkpoint,
    external_session_events, external_session_history, get_compaction_config,
    get_compaction_triggers, get_config, get_external_session, get_session, get_tool_catalog,
    handle_task, health, list_external_session_approvals, list_external_sessions,
    list_memory_providers, list_prompt_sections, list_sessions, list_skills, load_memory_entries,
    rag_delete_document, rag_ingest, rag_list_documents, rag_query,
    restore_external_session_checkpoint, restore_session, store_entry, submit_external_approval,
    submit_external_message, summarize, update_compaction_config, update_config,
};
use crate::bridge::LocalSessionBridge;
use crate::client::LLMClient;
use crate::context::memory_json::JsonFileMemoryProvider;
use crate::error::Result;
use amadeus_rag::embedding::EmbeddingClient;
use amadeus_rag::vector_store::VectorMemoryProvider;
use tokio::sync::RwLock;

/*
 * ============================================================================
 * STATE MANAGEMENT
 * ============================================================================
 */

/// Shared application state.
///
/// This struct is passed to every request handler via Axum's `State` extractor.
/// It provides access to the shared client, config, and orchestra-aware agent orchestrator.
pub struct AppState<C: LLMClient + Clone + 'static> {
    /// The shared base LLM client.
    pub client: C,
    /// The shared runtime configuration.
    pub config: Arc<Config>,
    /// The multi-agent orchestrator for standalone agent management.
    pub orchestrator: Arc<RwLock<AgentOrchestrator<C>>>,
    /// Interactive session bridge shared by richer agent routes.
    pub session_bridge: Arc<LocalSessionBridge<C>>,
    /// The default user-led orchestra used by stateless task endpoints.
    pub default_orchestra_id: crate::core::id::TeamId,
    /// Persistent memory provider backed by .amadeus/memory.json.
    pub memory_provider: Arc<JsonFileMemoryProvider>,
    /// Vector store for RAG document ingestion and semantic search.
    pub rag_provider: Arc<VectorMemoryProvider>,
    /// Embedding client for vectorizing text.
    pub embedding_client: Arc<EmbeddingClient>,
}

/*
 * ============================================================================
 * SERVER FUNCTIONS
 * ============================================================================
 */

/// Run the HTTP server.
///
/// Starts an Axum server on the specified port, using the provided orchestra
/// for task orchestration.
///
/// # Arguments
///
/// * `port` - Port number to listen on (e.g., 3000, 8080)
/// * `client` - Shared base client for creating agents
/// * `config` - Shared runtime configuration
///
/// # Returns
///
/// Returns `Ok(())` on successful shutdown or an error if the server fails to start.
///
/// # Example
///
/// ```rust,ignore
/// run_server(3000, client, config).await?;
/// ```
///
/// # Endpoints
///
/// | Path | Method | Handler | Description |
/// |------|--------|---------|-------------|
/// | `/health` | GET | `health` | Health check |
/// | `/chat` | POST | `chat` | Stateless chat via agent orchestra |
/// | `/execute` | POST | `execute` | Direct bash command execution |
/// | `/v1/sessions/*` | Multiple | external sessions | Stateful external protocol |
/// | `/tasks` | POST | `tasks` | Multi-agent task execution |
/// | `/sessions` | GET | `list_sessions` | List saved sessions |
/// | `/sessions/{id}` | GET | `get_session` | Get session details |
/// | `/sessions/{id}/restore` | POST | `restore_session` | Restore a session |
/// | `/config` | GET | `get_config` | Get current config |
/// | `/config` | PATCH | `update_config` | Update config settings |
/// | `/history` | GET | `get_history` | Get conversation history |
/// | `/skills` | GET | `list_skills` | List available skills |
pub async fn run_server<C: LLMClient + Clone + 'static>(
    port: u16,
    client: C,
    config: Arc<Config>,
    llm_trace: Option<Arc<crate::agent::llm_trace::LlmTraceSink>>,
) -> Result<()> {
    // Build memory registry for persistent context injection.
    let memory_path = config.workdir.join(".amadeus").join("memory.json");
    let memory_provider = Arc::new(JsonFileMemoryProvider::new(memory_path.clone()));
    let mut memory_registry = crate::context::memory::MemoryRegistry::new();
    memory_registry.register(memory_provider.clone());

    // Build RAG vector store and embedding client.
    let rag_path = config.workdir.join(".amadeus").join("rag_index.json");
    let rag_provider = Arc::new(VectorMemoryProvider::new(rag_path));
    let embedding_base_url = config
        .embedding_base_url
        .clone()
        .unwrap_or_else(|| config.base_url.clone().unwrap_or_default());
    let embedding_model = config
        .embedding_model
        .clone()
        .unwrap_or_else(|| config.model.clone());
    let embedding_client = Arc::new(EmbeddingClient::new(
        embedding_base_url,
        embedding_model,
        config.api_key.clone(),
    ));

    let orchestrator = AgentOrchestrator::new(client.clone(), Arc::clone(&config))
        .with_memory_registry(memory_registry.clone());
    let mut orchestrator = if let Some(ref trace) = llm_trace {
        orchestrator.with_llm_trace(Arc::clone(trace))
    } else {
        orchestrator
    };
    let default_orchestra_id = orchestrator.ensure_default_orchestra(OrchestraLeader::User);
    let main_agent_id = orchestrator
        .create_agent(Some("Main Agent".to_string()), AgentProfile::Default)
        .await?;
    orchestrator.add_agent_to_orchestra(default_orchestra_id, main_agent_id)?;
    let orchestrator = Arc::new(RwLock::new(orchestrator));
    let session_bridge = Arc::new({
        let bridge = LocalSessionBridge::new(client.clone(), Arc::clone(&config))
            .with_memory_registry(memory_registry);
        if let Some(ref trace) = llm_trace {
            bridge.with_llm_trace(Arc::clone(trace))
        } else {
            bridge
        }
    });
    let _ = session_bridge
        .create_session(Some("Main Agent".to_string()), AgentProfile::Default)
        .await?;

    let state = Arc::new(AppState {
        client,
        config,
        orchestrator,
        session_bridge,
        default_orchestra_id,
        memory_provider,
        rag_provider,
        embedding_client,
    });

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
    println!("External Session API:");
    println!("  GET/POST /v1/sessions");
    println!("  GET/DELETE /v1/sessions/:id");
    println!("  POST /v1/sessions/:id/messages");
    println!("  GET  /v1/sessions/:id/events");
    println!("  GET  /v1/sessions/:id/history");
    println!("  GET/POST /v1/sessions/:id/approvals[/approval_id]");
    println!("  GET/PUT /v1/sessions/:id/checkpoint");
    println!("  POST /v1/sessions/:id/cancel");
    println!();
    println!("Unversioned Utilities:");
    println!("  GET  /health   - Health check");
    println!("  POST /chat     - Send stateless message");
    println!("  POST /execute  - Execute bash command");
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
        .route(
            "/v1/sessions",
            get(list_external_sessions).post(create_external_session),
        )
        .route(
            "/v1/sessions/:id",
            get(get_external_session).delete(close_external_session),
        )
        .route("/v1/sessions/:id/messages", post(submit_external_message))
        .route("/v1/sessions/:id/events", get(external_session_events))
        .route("/v1/sessions/:id/history", get(external_session_history))
        .route(
            "/v1/sessions/:id/approvals",
            get(list_external_session_approvals),
        )
        .route(
            "/v1/sessions/:id/approvals/:approval_id",
            post(submit_external_approval),
        )
        .route(
            "/v1/sessions/:id/checkpoint",
            get(external_session_checkpoint).put(restore_external_session_checkpoint),
        )
        .route("/v1/sessions/:id/compact", post(compact_external_session))
        .route("/v1/sessions/:id/cancel", post(cancel_external_session))
        // =====================================================================
        // CORE ENDPOINTS
        // =====================================================================
        // Health check endpoint (Stateless)
        .route("/health", get(health))
        // Chat endpoint (Stateless wrapper around the orchestra orchestrator)
        .route("/chat", post(chat))
        // Execute endpoint (Direct tool access)
        .route("/execute", post(execute))
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
        // List available skills
        .route("/skills", get(list_skills))
        // Summarize arbitrary text for research workflows
        .route("/summarize", post(summarize))
        // =====================================================================
        // MODULAR PROMPT & MEMORY API
        // =====================================================================
        // Compaction config and triggers
        .route("/compaction/config", get(get_compaction_config))
        .route("/compaction/config", patch(update_compaction_config))
        .route("/compaction/triggers", get(get_compaction_triggers))
        // System prompt sections and custom building
        .route("/prompts/sections", get(list_prompt_sections))
        .route("/prompts/build", post(build_prompt))
        // Memory providers and entries
        .route("/memory/providers", get(list_memory_providers))
        .route("/memory/entries", get(load_memory_entries))
        .route("/memory/entries", post(store_entry))
        .route("/memory/entries/:key", delete(delete_entry))
        // Tool catalog
        .route("/tools/catalog", get(get_tool_catalog))
        // RAG semantic search
        .route("/rag/ingest", post(rag_ingest))
        .route("/rag/query", post(rag_query))
        .route("/rag/documents", get(rag_list_documents))
        .route("/rag/documents/:id", delete(rag_delete_document))
        // =====================================================================
        // MULTI-AGENT ENDPOINTS
        // =====================================================================
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
