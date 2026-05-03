// @amadeus-header
// summary: EmbeddingClient — calls OpenAI-compatible /v1/embeddings endpoint.
// layer: core
// status: active
// feature_flags: none
// provides:
// - type: crate::embedding::EmbeddingClient
// - type: crate::embedding::EmbeddingError
// uses:
// - crate: reqwest (HTTP client)
// - service: /v1/embeddings (OpenAI-compatible)
// invariants:
// - Batching: max 32 texts per request.
// - Embedding vectors are f32 slices.
// side_effects:
// - Makes HTTP POST requests to the embedding API endpoint.
// tests:
// - cmd: cargo test -p rag
// @end-amadeus-header

//! Embedding client for OpenAI-compatible `/v1/embeddings` endpoints.
//!
//! Calls the same vLLM server already used for LLM inference.

use reqwest::Client;
use serde::{Deserialize, Serialize};

const BATCH_SIZE: usize = 32;

#[derive(Debug, Clone, Serialize)]
struct EmbeddingRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Clone, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

/// Calls an OpenAI-compatible `/v1/embeddings` endpoint.
///
/// Usage:
/// ```ignore
/// let client = EmbeddingClient::new("http://host/v1", "model", "key");
/// let vecs = client.embed(&["hello".into(), "world".into()]).await?;
/// ```
#[derive(Debug, Clone)]
pub struct EmbeddingClient {
    client: Client,
    base_url: String,
    model: String,
    api_key: String,
}

impl EmbeddingClient {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>, api_key: impl Into<String>) -> Self {
        let mut builder = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .connect_timeout(std::time::Duration::from_secs(10));

        if std::env::var("AMADEUS_NO_PROXY")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(true)
        {
            builder = builder.no_proxy();
        }

        let client = builder.build().expect("Failed to create HTTP client");

        Self {
            client,
            base_url: base_url.into(),
            model: model.into(),
            api_key: api_key.into(),
        }
    }

    /// Embed multiple texts. Batches large inputs into groups of [`BATCH_SIZE`].
    pub async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let url = format!("{}/embeddings", self.base_url.trim_end_matches('/'));

        let mut all_embeddings: Vec<Vec<f32>> = Vec::with_capacity(texts.len());

        for batch in texts.chunks(BATCH_SIZE) {
            let batch_texts: Vec<String> = batch.to_vec();
            let req = EmbeddingRequest {
                model: self.model.clone(),
                input: batch_texts.clone(),
            };

            let resp = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&req)
                .send()
                .await
                .map_err(|e| EmbeddingError::Network(e.to_string()))?;

            if !resp.status().is_success() {
                let status = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();
                return Err(EmbeddingError::Api { status, body });
            }

            let parsed: EmbeddingResponse = resp
                .json()
                .await
                .map_err(|e| EmbeddingError::Parse(e.to_string()))?;

            for item in parsed.data {
                all_embeddings.push(item.embedding);
            }
        }

        Ok(all_embeddings)
    }

    /// Convenience: embed a single text.
    pub async fn embed_single(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let results = self.embed(&[text.to_string()]).await?;
        Ok(results.into_iter().next().unwrap_or_default())
    }
}

#[derive(Debug, Clone)]
pub enum EmbeddingError {
    Network(String),
    Api { status: u16, body: String },
    Parse(String),
}

impl std::fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(msg) => write!(f, "embedding network error: {}", msg),
            Self::Api { status, body } => write!(f, "embedding API error {}: {}", status, body),
            Self::Parse(msg) => write!(f, "embedding parse error: {}", msg),
        }
    }
}
