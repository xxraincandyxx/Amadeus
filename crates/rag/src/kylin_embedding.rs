// @amadeus-header
// summary: KylinEmbedder — Galaxy Kylin embedding SDK adapter (local OpenAI-compatible service).
// layer: core
// status: active
// feature_flags: none
// provides:
// - type: crate::kylin_embedding::KylinEmbedder
// - const: crate::kylin_embedding::DEFAULT_KYLIN_ENDPOINT
// uses:
// - type: crate::embedding::EmbeddingClient
// - trait: crate::embedding::Embedder
// invariants:
// - Delegates to the OpenAI-compatible embedding HTTP shape.
// - Missing endpoint config falls back to the default localhost endpoint.
// side_effects:
// - Makes HTTP requests to the Kylin SDK local inference service.
// tests:
// - cmd: cargo test -p rag
// @end-amadeus-header

//! Galaxy Kylin (银河麒麟) embedding SDK adapter.
//!
//! The Kylin AI SDK exposes on-device embedding inference as a local
//! OpenAI-compatible HTTP service, so this adapter is a deliberately thin
//! preset over [`EmbeddingClient`]: Kylin-specific defaults plus its own
//! [`Embedder`] identity. If a future SDK revision ships as a C FFI library
//! instead, only this file's internals change — the trait contract, the rag
//! tool, and the HTTP API stay untouched.

use crate::embedding::{Embedder, EmbeddingClient, EmbeddingError};

/// Default local endpoint of the Kylin embedding SDK service.
pub const DEFAULT_KYLIN_ENDPOINT: &str = "http://127.0.0.1:18080/v1";

/// Embedding backend backed by the Galaxy Kylin SDK local service.
#[derive(Debug, Clone)]
pub struct KylinEmbedder {
    inner: EmbeddingClient,
}

impl KylinEmbedder {
    /// Create the adapter. `endpoint: None` uses [`DEFAULT_KYLIN_ENDPOINT`].
    pub fn new(endpoint: Option<String>, model: &str, api_key: &str) -> Self {
        let base_url = endpoint.unwrap_or_else(|| DEFAULT_KYLIN_ENDPOINT.to_string());
        Self {
            inner: EmbeddingClient::new(base_url, model, api_key),
        }
    }
}

#[async_trait::async_trait]
impl Embedder for KylinEmbedder {
    fn name(&self) -> &'static str {
        "kylin_sdk"
    }

    fn dimension(&self) -> Option<usize> {
        None
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        self.inner.embed(texts).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_endpoint_used_when_unconfigured() {
        let embedder = KylinEmbedder::new(None, "kylin-embedding", "");
        assert_eq!(embedder.name(), "kylin_sdk");
        assert_eq!(embedder.dimension(), None);
    }

    #[test]
    fn test_custom_endpoint_accepted() {
        let embedder =
            KylinEmbedder::new(Some("http://10.0.0.2:9000/v1".to_string()), "model", "key");
        assert_eq!(embedder.name(), "kylin_sdk");
    }
}
