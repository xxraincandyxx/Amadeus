// @amadeus-header
// summary: End-to-end acceptance tests for KylinMem functional indicators three and five.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides: none
// uses:
// - tool: kylin_memory
// - type: amadeus::memory_service::MemoryService
// invariants:
// - Indicator three tests cover default registration, versioned conflicts, explained retrieval, and reviewed templates.
// - Indicator five tests cover redaction, preview-confirm deletion, exclusions, and physical negative verification.
// side_effects:
// - Writes only to temporary test directories.
// tests:
// - cmd: cargo test --test kylin_memory_indicators_test --features full
// @end-amadeus-header

use std::sync::Arc;

use amadeus::agent::loop_agent::Agent;
use amadeus::agent::Config;
use amadeus::client::anthropic::AnthropicClient;
use amadeus::memory_service::MemoryService;
use amadeus::tools::{KylinMemoryTool, Tool};
use serde_json::{json, Value};
use tempfile::TempDir;

fn setup() -> (TempDir, KylinMemoryTool) {
    let temp = TempDir::new().unwrap();
    let service =
        Arc::new(MemoryService::open(temp.path().join("structured_memory.json")).unwrap());
    (temp, KylinMemoryTool::from_service(service))
}

async fn execute(tool: &KylinMemoryTool, input: Value) -> Value {
    let output = tool.execute(input).await.unwrap();
    serde_json::from_str(&output).unwrap()
}

#[tokio::test]
async fn indicator_3_conflict_search_and_history_form_a_closed_loop() {
    let (_temp, tool) = setup();
    let first = execute(&tool, json!({ "operation": "store_fact", "subject": "用户", "predicate": "住址", "object": "南京", "source": "session-a", "confidence": 0.9 })).await;
    assert_eq!(first["action"], "add");
    let second = execute(&tool, json!({ "operation": "store_fact", "subject": "用户", "predicate": "住址", "object": "北京", "source": "session-b", "confidence": 0.95 })).await;
    assert_eq!(second["action"], "supersede");
    assert!(second["superseded_fact_id"].as_str().is_some());

    let current = execute(&tool, json!({ "operation": "search", "query": "用户地址", "include_history": false, "top_k": 10 })).await;
    assert_eq!(current.as_array().unwrap().len(), 1);
    assert_eq!(current[0]["fact"]["object"], "北京");
    assert!(!current[0]["reasons"].as_array().unwrap().is_empty());

    let history = execute(
        &tool,
        json!({ "operation": "search", "query": "用户地址", "include_history": true, "top_k": 10 }),
    )
    .await;
    assert_eq!(history.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn indicator_3_template_activation_requires_five_examples_review_and_eighty_percent_success()
{
    let (_temp, tool) = setup();
    let registered = execute(&tool, json!({ "operation": "register_template", "template_name": "差旅规划", "intent": "规划差旅", "preconditions": ["目的地"], "steps": ["检索偏好", "生成行程"] })).await;
    let template_id = registered["template_id"].as_str().unwrap();
    for index in 0..5 {
        execute(&tool, json!({ "operation": "store_fact", "subject": "模板案例", "predicate": "执行记录", "object": format!("案例-{index}"), "source": format!("session-{index}") })).await;
    }
    let evidence = execute(
        &tool,
        json!({ "operation": "search", "query": "执行记录", "top_k": 10 }),
    )
    .await;
    assert!(tool
        .execute(json!({ "operation": "record_template_result", "template_id": template_id, "episode_id": "invented", "success": true }))
        .await
        .is_err());
    for (index, hit) in evidence.as_array().unwrap().iter().enumerate() {
        let episode_id = hit["fact"]["source_episode_ids"][0].as_str().unwrap();
        execute(&tool, json!({ "operation": "record_template_result", "template_id": template_id, "episode_id": episode_id, "success": index < 4 })).await;
    }
    execute(
        &tool,
        json!({ "operation": "activate_template", "template_id": template_id }),
    )
    .await;
    let templates = execute(
        &tool,
        json!({ "operation": "find_templates", "intent": "规划" }),
    )
    .await;
    assert_eq!(templates.as_array().unwrap().len(), 1);
    assert_eq!(templates[0]["status"], "active");
}

#[tokio::test]
async fn indicator_5_sensitive_information_is_redacted_at_scan_and_storage_boundaries() {
    let (temp, tool) = setup();
    let scan = execute(&tool, json!({ "operation": "privacy_scan", "content": "联系电话13800138000，邮箱test@example.com，密码：S3cret-value" })).await;
    let redacted = scan["redacted"].as_str().unwrap();
    assert!(!redacted.contains("13800138000"));
    assert!(!redacted.contains("test@example.com"));
    assert!(!redacted.contains("S3cret-value"));
    assert_eq!(scan["detections"].as_array().unwrap().len(), 3);

    execute(&tool, json!({ "operation": "store_fact", "subject": "用户13800138000", "predicate": "邮箱", "object": "test@example.com", "source": "session-private" })).await;
    let persisted = std::fs::read_to_string(temp.path().join("structured_memory.json")).unwrap();
    assert!(!persisted.contains("13800138000"));
    assert!(!persisted.contains("test@example.com"));
}

#[tokio::test]
async fn indicator_5_precise_forgetting_preserves_exclusions_and_requires_confirmation() {
    let (_temp, tool) = setup();
    execute(&tool, json!({ "operation": "store_fact", "subject": "用户", "predicate": "住址", "object": "南京鼓楼区", "source": "session-a" })).await;
    execute(&tool, json!({ "operation": "store_fact", "subject": "用户", "predicate": "城市偏好", "object": "南京", "source": "session-b" })).await;

    let preview = execute(
        &tool,
        json!({ "operation": "forget_preview", "instruction": "忘掉我的所有住址，但保留城市偏好" }),
    )
    .await;
    assert_eq!(preview["fact_ids"].as_array().unwrap().len(), 1);
    assert_eq!(preview["confirmation_required"], true);
    assert!(tool
        .execute(
            json!({ "operation": "forget_confirm", "plan_id": preview["id"], "plan_hash": "wrong" })
        )
        .await
        .is_err());

    let result = execute(&tool, json!({ "operation": "forget_confirm", "plan_id": preview["id"], "plan_hash": preview["plan_hash"] })).await;
    assert_eq!(result["verified"], true);
    assert_eq!(result["residual_matches"], 0);

    let address = execute(&tool, json!({ "operation": "search", "query": "用户地址" })).await;
    assert!(address.as_array().unwrap().is_empty());
    let preference = execute(&tool, json!({ "operation": "search", "query": "城市偏好" })).await;
    assert_eq!(preference.as_array().unwrap().len(), 1);
}

#[test]
fn indicator_3_default_agent_registers_kylin_memory_without_legacy_registry() {
    let temp = TempDir::new().unwrap();
    let config = Config {
        api_key: "test-key".to_string(),
        model: "test-model".to_string(),
        workdir: temp.path().to_path_buf(),
        ..Config::default()
    };
    let client = AnthropicClient::new("test-key".to_string(), None, "test-model".to_string());
    let agent = Agent::new(client, Arc::new(config));
    assert!(agent
        .registry()
        .names()
        .contains(&"kylin_memory".to_string()));
}

#[tokio::test]
async fn indicator_5_rejects_ambiguous_and_non_deletion_requests() {
    let (_temp, tool) = setup();
    assert!(tool
        .execute(json!({ "operation": "forget_preview", "instruction": "请记住我的地址" }))
        .await
        .is_err());
    assert!(tool
        .execute(json!({ "operation": "forget_preview", "instruction": "全部忘掉" }))
        .await
        .is_err());
}
