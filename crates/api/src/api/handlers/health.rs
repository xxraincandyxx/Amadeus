// @amadeus-header
// summary: HTTP handler implementation for health routes.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::handlers::health
// - fn: crate::api::handlers::health::health
// uses:
// - module: crate::api::types::HealthResponse
// - protocol: axum HTTP handlers
// invariants:
// - Handler request and response handling stays aligned with route contracts.
// side_effects:
// - Performs network or HTTP operations.
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

//! # Health Check Handler
//!
//! Simple health check endpoint to verify the server is running.
//!
//! ## Endpoint
//!
//! `GET /health`
//!
//! ## Response
//!
//! ```json
//! {
//!   "status": "ok",
//!   "version": "0.1.0"
//! }
//! ```
//!
//! ## Usage
//!
//! ```bash
//! curl http://localhost:3000/health
//! ```
//!
//! ## Purpose
//!
//! Used by:
//! - Load balancers to verify server health
//! - Container orchestrators (Kubernetes, Docker)
//! - Monitoring systems
//! - CI/CD pipelines

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

// Json extractor/serializer from axum
//
// Json<T>:
// - Extracts JSON from request body (for POST)
// - Serializes response to JSON (for all responses)
use axum::Json;

// Health response type
use crate::api::types::HealthResponse;

/*
 * ============================================================================
 * HANDLER FUNCTION
 * ============================================================================
 */

/// Handle GET /health requests.
///
/// Returns a simple health status indicating the server is running.
///
/// # Returns
///
/// JSON response with:
/// - `status`: Always "ok" when healthy
/// - `version`: Crate version from Cargo.toml
///
/// # Example
///
/// ```rust,ignore
/// // In router setup:
/// router.route("/health", get(health));
/// ```
///
/// # HTTP Example
///
/// ```bash
/// $ curl http://localhost:3000/health
/// {"status":"ok","version":"0.1.0"}
/// ```
pub async fn health() -> Json<HealthResponse> {
    // -------------------------------------------------------------------------
    // BUILD RESPONSE
    // -------------------------------------------------------------------------

    // Create the health response
    //
    // env!("CARGO_PKG_VERSION") is a built-in macro that gets the version
    // from Cargo.toml at compile time. This is more reliable than hardcoding.
    Json(HealthResponse {
        // "ok" indicates the server is healthy
        status: "ok".to_string(),
        // Version from Cargo.toml [package].version
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/*
 * ============================================================================
 * TESTS
 * ============================================================================
 */

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_handler() {
        // Call the health handler
        let response = health().await;

        // Verify the response
        assert_eq!(response.status, "ok");
        assert!(!response.version.is_empty());
    }
}
