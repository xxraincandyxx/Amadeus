// @amadeus-header
// summary: RagTool — LLM-callable tool exposing RAG ingest, query, list, and delete operations.
// layer: tools
// status: active
// feature_flags: none
// provides:
// - type: crate::tool::RagTool
// uses:
// - crate: core::tools::Tool (tool trait)
// - crate: core::error (AgentError, Result)
// - crate::embedding::EmbeddingClient
// - crate::vector_store::VectorMemoryProvider
// - crate::chunker::chunk_text
// - crate: amadeus_config::Config
// invariants:
// - Schema is constructed once via OnceLock.
// - Ingest reads source, chunks text, embeds, and stores in a single operation.
// - Query embeds query text and returns top-k chunk results with scores.
// side_effects:
// - Writes document chunks to the vector store (persisted to disk).
// - Makes HTTP calls via EmbeddingClient for embedding generation.
// tests:
// - cmd: cargo test -p rag
// @end-amadeus-header

//! LLM-callable RAG tool.
//!
//! Exposes RAG operations (ingest, query, list_documents, delete_document)
//! to the LLM via the [`Tool`] trait from `core`.

use std::sync::Arc;

use amadeus_core::error::{AgentError, Result};
use amadeus_core::tools::tool_trait::Tool;
use serde::Deserialize;
use serde_json::Value;

use crate::chunker::chunk_text;
use crate::embedding::EmbeddingClient;
use crate::vector_store::VectorMemoryProvider;

#[derive(Debug, Deserialize)]
struct RagInput {
    operation: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    document_id: Option<String>,
    #[serde(default)]
    query_text: Option<String>,
    #[serde(default)]
    chunk_size: Option<usize>,
    #[serde(default)]
    chunk_overlap: Option<usize>,
    #[serde(default)]
    top_k: Option<usize>,
}

fn rag_schema() -> &'static Value {
    static SCHEMA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    SCHEMA.get_or_init(|| {
        serde_json::json!({
            "name": "rag",
            "description": "Semantic search over documents using embeddings. Ingest files, URLs, or text into a vector store, then query them with natural language to find relevant chunks. Supports listing and deleting ingested documents.",
            "parameters": {
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["ingest", "query", "list_documents", "delete_document"],
                        "description": "The RAG operation to perform."
                    },
                    "path": {
                        "type": "string",
                        "description": "Path to a local file to ingest (for 'ingest' operation)."
                    },
                    "url": {
                        "type": "string",
                        "description": "URL to fetch and ingest content from (for 'ingest' operation)."
                    },
                    "text": {
                        "type": "string",
                        "description": "Raw text to ingest directly (for 'ingest' operation)."
                    },
                    "document_id": {
                        "type": "string",
                        "description": "A human-readable document identifier (for 'ingest' and 'delete_document'). Auto-generated if not provided."
                    },
                    "query_text": {
                        "type": "string",
                        "description": "Natural language query for semantic search (for 'query' operation)."
                    },
                    "chunk_size": {
                        "type": "integer",
                        "description": "Target characters per chunk (default: from config, usually 1200)."
                    },
                    "chunk_overlap": {
                        "type": "integer",
                        "description": "Overlap characters between adjacent chunks (default: from config, usually 200)."
                    },
                    "top_k": {
                        "type": "integer",
                        "description": "Number of top results to return (default: from config, usually 5)."
                    }
                },
                "required": ["operation"]
            }
        })
    })
}

pub struct RagTool {
    store: Arc<VectorMemoryProvider>,
    embedder: Arc<EmbeddingClient>,
    default_chunk_size: usize,
    default_chunk_overlap: usize,
    default_top_k: usize,
}

impl RagTool {
    pub fn new(
        store: Arc<VectorMemoryProvider>,
        embedder: Arc<EmbeddingClient>,
        default_chunk_size: usize,
        default_chunk_overlap: usize,
        default_top_k: usize,
    ) -> Self {
        Self {
            store,
            embedder,
            default_chunk_size,
            default_chunk_overlap,
            default_top_k,
        }
    }

    async fn do_ingest(&self, input: RagInput) -> Result<String> {
        // Resolve source text
        let text = if let Some(path) = input.path {
            std::fs::read_to_string(&path).map_err(|e| AgentError::ToolInput {
                tool: "rag".into(),
                reason: format!("Failed to read file '{}': {}", path, e),
            })?
        } else if let Some(url) = input.url {
            let resp = reqwest::get(&url)
                .await
                .map_err(|e| AgentError::ToolInput {
                    tool: "rag".into(),
                    reason: format!("Failed to fetch URL '{}': {}", url, e),
                })?;
            if !resp.status().is_success() {
                return Err(AgentError::ToolInput {
                    tool: "rag".into(),
                    reason: format!("HTTP {} fetching '{}'", resp.status().as_u16(), url),
                });
            }
            resp.text().await.map_err(|e| AgentError::ToolInput {
                tool: "rag".into(),
                reason: format!("Failed to read response body from '{}': {}", url, e),
            })?
        } else if let Some(text) = input.text {
            text
        } else {
            return Err(AgentError::ToolInput {
                tool: "rag".into(),
                reason: "One of 'path', 'url', or 'text' is required for the 'ingest' operation."
                    .into(),
            });
        };

        let document_id = input
            .document_id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let chunk_size = input.chunk_size.unwrap_or(self.default_chunk_size);
        let chunk_overlap = input.chunk_overlap.unwrap_or(self.default_chunk_overlap);

        let chunks = chunk_text(&text, chunk_size, chunk_overlap);
        if chunks.is_empty() {
            return Ok(format!(
                "No content to ingest for document '{}'.",
                document_id
            ));
        }

        let embeddings = self
            .embedder
            .embed(&chunks)
            .await
            .map_err(|e| AgentError::ToolInput {
                tool: "rag".into(),
                reason: format!("Embedding failed: {}", e),
            })?;

        let chunk_count = self
            .store
            .ingest_chunks(&document_id, "", chunks, embeddings)
            .map_err(|e| AgentError::ToolInput {
                tool: "rag".into(),
                reason: format!("Store failed: {}", e),
            })?;

        Ok(format!(
            "Ingested {} chunks for document '{}'.",
            chunk_count, document_id
        ))
    }

    async fn do_query(&self, input: RagInput) -> Result<String> {
        let query_text = input.query_text.ok_or_else(|| AgentError::ToolInput {
            tool: "rag".into(),
            reason: "query_text is required for the 'query' operation.".into(),
        })?;
        let top_k = input.top_k.unwrap_or(self.default_top_k);

        let query_embedding =
            self.embedder
                .embed_single(&query_text)
                .await
                .map_err(|e| AgentError::ToolInput {
                    tool: "rag".into(),
                    reason: format!("Embedding failed: {}", e),
                })?;

        let results = self.store.search(&query_embedding, top_k);

        if results.is_empty() {
            return Ok("No relevant chunks found.".to_string());
        }

        let lines: Vec<String> = results
            .into_iter()
            .enumerate()
            .map(|(i, (entry, score))| {
                format!(
                    "{}. [score: {:.3}] ({}) {}\n   {}",
                    i + 1,
                    score,
                    entry.source,
                    entry.key,
                    entry.content
                )
            })
            .collect();

        Ok(format!(
            "Found {} result(s):\n\n{}",
            lines.len(),
            lines.join("\n\n")
        ))
    }

    fn do_list_documents(&self) -> Result<String> {
        let docs = self.store.list_documents();
        if docs.is_empty() {
            return Ok("No documents ingested.".to_string());
        }

        let lines: Vec<String> = docs
            .iter()
            .map(|d| {
                format!(
                    "- {} ({} chunks, ingested at {})",
                    d.id, d.chunk_count, d.ingested_at
                )
            })
            .collect();

        Ok(format!(
            "{} document(s):\n{}",
            lines.len(),
            lines.join("\n")
        ))
    }

    fn do_delete_document(&self, document_id: &str) -> Result<String> {
        let removed =
            self.store
                .delete_document(document_id)
                .map_err(|e| AgentError::ToolInput {
                    tool: "rag".into(),
                    reason: format!("Delete failed: {}", e),
                })?;

        Ok(format!(
            "Deleted {} chunk(s) for document '{}'.",
            removed, document_id
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_tool(temp: &TempDir) -> RagTool {
        let path = temp.path().join("rag_index.json");
        let store = Arc::new(VectorMemoryProvider::new(path));
        // Use a dummy embedder that will fail if actually called
        let embedder = Arc::new(EmbeddingClient::new(
            "http://localhost:0/v1",
            "test-model",
            "test-key",
        ));
        RagTool::new(store, embedder, 500, 100, 3)
    }

    #[test]
    fn test_schema_is_valid() {
        let tool = make_tool(&TempDir::new().unwrap());
        let schema = tool.schema();
        assert_eq!(schema["name"], "rag");
        assert!(schema["parameters"]["properties"]["operation"].is_object());
    }

    #[test]
    fn test_tool_name() {
        let tool = make_tool(&TempDir::new().unwrap());
        assert_eq!(tool.name(), "rag");
    }

    #[test]
    fn test_list_documents_empty() {
        let tool = make_tool(&TempDir::new().unwrap());
        let result = tool.do_list_documents().unwrap();
        assert!(result.contains("No documents"));
    }

    #[test]
    fn test_list_and_delete_documents() {
        let temp = TempDir::new().unwrap();
        let tool = make_tool(&temp);

        // Manually insert chunks without embedding
        tool.store
            .ingest_chunks(
                "doc1",
                "/tmp/doc1.md",
                vec!["chunk a".to_string(), "chunk b".to_string()],
                vec![vec![1.0, 0.0], vec![0.0, 1.0]],
            )
            .unwrap();

        let list = tool.do_list_documents().unwrap();
        assert!(list.contains("doc1"));
        assert!(list.contains("2 chunks"));

        let del = tool.do_delete_document("doc1").unwrap();
        assert!(del.contains("2 chunk"));
        assert!(del.contains("doc1"));

        let list_after = tool.do_list_documents().unwrap();
        assert!(list_after.contains("No documents"));
    }

    #[test]
    fn test_delete_nonexistent_document() {
        let tool = make_tool(&TempDir::new().unwrap());
        // Should succeed with 0 removed
        let result = tool.do_delete_document("nonexistent").unwrap();
        assert!(result.contains("Deleted 0 chunk"));
    }

    #[tokio::test]
    async fn test_ingest_missing_source() {
        let tool = make_tool(&TempDir::new().unwrap());
        let input = serde_json::json!({"operation": "ingest"});
        let result = tool.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("path"));
    }

    #[tokio::test]
    async fn test_query_missing_query_text() {
        let tool = make_tool(&TempDir::new().unwrap());
        let input = serde_json::json!({"operation": "query"});
        let result = tool.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("query_text"));
    }

    #[tokio::test]
    async fn test_unknown_operation() {
        let tool = make_tool(&TempDir::new().unwrap());
        let input = serde_json::json!({"operation": "invalid"});
        let result = tool.execute(input).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown operation"));
    }

    #[tokio::test]
    async fn test_delete_document_missing_id() {
        let tool = make_tool(&TempDir::new().unwrap());
        let input = serde_json::json!({"operation": "delete_document"});
        let result = tool.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("document_id"));
    }
}

#[async_trait::async_trait]
impl Tool for RagTool {
    fn name(&self) -> &'static str {
        "rag"
    }

    fn schema(&self) -> &'static Value {
        rag_schema()
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let parsed: RagInput =
            serde_json::from_value(input).map_err(|e| AgentError::ToolInput {
                tool: "rag".to_string(),
                reason: e.to_string(),
            })?;

        match parsed.operation.as_str() {
            "ingest" => self.do_ingest(parsed).await,
            "query" => self.do_query(parsed).await,
            "list_documents" => self.do_list_documents(),
            "delete_document" => {
                let doc_id = parsed.document_id.ok_or_else(|| AgentError::ToolInput {
                    tool: "rag".into(),
                    reason: "document_id is required for 'delete_document' operation.".into(),
                })?;
                self.do_delete_document(&doc_id)
            }
            other => Err(AgentError::ToolInput {
                tool: "rag".into(),
                reason: format!(
                    "Unknown operation '{}'. Use ingest, query, list_documents, or delete_document.",
                    other
                ),
            }),
        }
    }
}
