// @amadeus-header
// summary: End-to-end acceptance tests for functional indicator four (edge deployment, pluggable embedding backends, lightweight quantization).
// layer: test
// status: test-only
// feature_flags:
// - full
// provides: none
// uses:
// - tool: rag
// - type: amadeus::rag::local_embedding::LocalHashEmbedder
// - type: amadeus::rag::vector_store::VectorMemoryProvider
// invariants:
// - Local hashing is deterministic, offline, and lexically grounded (CJK bigrams + Latin words).
// - int8 quantization preserves search rankings while shrinking storage 4x.
// side_effects:
// - Writes only to temporary test directories.
// tests:
// - cmd: cargo test --test embedding_indicator_test --features full
// @end-amadeus-header

use std::sync::Arc;

use amadeus::agent::loop_agent::Agent;
use amadeus::agent::Config;
use amadeus::client::anthropic::AnthropicClient;
use amadeus::rag::embedding::{embedder_from_config, Embedder, EmbedderConfig};
use amadeus::rag::local_embedding::LocalHashEmbedder;
use amadeus::rag::tool::RagTool;
use amadeus::rag::vector_store::{Quantization, VectorMemoryProvider};
use amadeus::tools::Tool;
use serde_json::json;
use tempfile::TempDir;

fn local_tool(temp: &TempDir) -> RagTool {
    let store = Arc::new(VectorMemoryProvider::new(
        temp.path().join("rag_index.json"),
    ));
    let embedder: Arc<dyn Embedder> = Arc::new(LocalHashEmbedder::new(128));
    RagTool::new(store, embedder, 200, 40, 3)
}

#[tokio::test]
async fn indicator_4_local_hash_embedder_is_deterministic_normalized_and_offline() {
    let embedder = LocalHashEmbedder::new(64);
    assert_eq!(embedder.name(), "local_hash");
    assert_eq!(embedder.dimension(), Some(64));

    let first = embedder
        .embed_single("银河麒麟 操作系统 edge")
        .await
        .unwrap();
    let second = embedder
        .embed_single("银河麒麟 操作系统 edge")
        .await
        .unwrap();
    assert_eq!(first, second, "hash embeddings must be deterministic");

    let norm: f32 = first.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-4, "embeddings are L2-normalized");
    assert!(first.iter().any(|x| *x != 0.0));
}

#[tokio::test]
async fn indicator_4_rag_tool_ingest_and_query_end_to_end() {
    let temp = TempDir::new().unwrap();
    let tool = local_tool(&temp);

    let ingested = tool
        .execute(json!({
            "operation": "ingest",
            "text": "银河麒麟操作系统支持端侧大模型推理与轻量化部署",
            "document_id": "kylin-doc"
        }))
        .await
        .unwrap();
    assert!(ingested.contains("kylin-doc"));

    tool.execute(json!({
        "operation": "ingest",
        "text": "apple banana fruit salad recipe for breakfast",
        "document_id": "food-doc"
    }))
    .await
    .unwrap();

    let answer = tool
        .execute(json!({ "operation": "query", "query_text": "麒麟 操作系统 端侧" }))
        .await
        .unwrap();
    assert!(answer.contains("kylin-doc"), "expected kylin hit: {answer}");

    // Lexical overlap must outrank the unrelated document.
    let kylin_pos = answer.find("kylin-doc").unwrap();
    if let Some(food_pos) = answer.find("food-doc") {
        assert!(kylin_pos < food_pos, "kylin doc must rank first");
    }

    let docs = tool
        .execute(json!({ "operation": "list_documents" }))
        .await
        .unwrap();
    assert!(docs.contains("kylin-doc") && docs.contains("food-doc"));

    let deleted = tool
        .execute(json!({ "operation": "delete_document", "document_id": "food-doc" }))
        .await
        .unwrap();
    assert!(deleted.contains("food-doc"));
}

#[tokio::test]
async fn indicator_4_int8_quantized_store_survives_restart() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("rag_index.json");

    {
        let store = VectorMemoryProvider::with_quantization(path.clone(), Quantization::Int8);
        store
            .ingest_chunks(
                "doc",
                "/d",
                vec!["端侧部署".to_string()],
                vec![vec![0.5, -0.25, 1.0]],
            )
            .unwrap();
    }

    // On disk the store is a v2 envelope with quantized vectors (4x smaller).
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.contains("\"version\": 2"));
    assert!(raw.contains("embedding_i8"));

    let reopened = VectorMemoryProvider::with_quantization(path, Quantization::Int8);
    let results = reopened.search(&[0.5, -0.25, 1.0], 1);
    assert_eq!(results.len(), 1);
    assert!(results[0].1 > 0.99, "self-similarity survives quantization");
}

#[test]
fn indicator_4_backend_selection_from_config() {
    let local = embedder_from_config(&EmbedderConfig {
        backend: "local".to_string(),
        local_dimension: 96,
        ..EmbedderConfig::default()
    });
    assert_eq!(local.name(), "local_hash");
    assert_eq!(local.dimension(), Some(96));

    let kylin = embedder_from_config(&EmbedderConfig {
        backend: "kylin".to_string(),
        ..EmbedderConfig::default()
    });
    assert_eq!(kylin.name(), "kylin_sdk");

    let remote = embedder_from_config(&EmbedderConfig::default());
    assert_eq!(remote.name(), "remote_http");

    let unknown = embedder_from_config(&EmbedderConfig {
        backend: "bogus".to_string(),
        ..EmbedderConfig::default()
    });
    assert_eq!(unknown.name(), "remote_http", "unknown backends fall back");
}

#[test]
fn indicator_4_agent_registers_rag_tool_when_enabled() {
    let temp = TempDir::new().unwrap();
    let mut config = Config {
        api_key: "test-key".to_string(),
        model: "test-model".to_string(),
        workdir: temp.path().to_path_buf(),
        embedding_backend: "local".to_string(),
        ..Config::default()
    };
    config.rag_enabled = true;
    let config = Arc::new(config);

    let client = AnthropicClient::new("test-key".to_string(), None, "test-model".to_string());
    // Same call sequence as the production composition root (`build_agent`).
    let mut builder = Agent::builder(client, Arc::clone(&config)).with_default_tools();
    if config.rag_enabled {
        builder = builder.with_rag(Box::new(RagTool::open_in_workspace(
            &config.workdir,
            &config,
        )));
    }
    let agent = builder.build();
    assert!(agent.registry().names().contains(&"rag".to_string()));
}
