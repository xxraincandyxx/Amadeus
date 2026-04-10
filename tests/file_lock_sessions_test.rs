// @amadeus-header
// summary: Integration tests covering multi-session file lock behavior.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::file_lock_sessions_test
// uses:
// - module: amadeus::concurrency::FileLockManager
// - module: amadeus::core::id::AgentId
// - module: amadeus::error::AgentError
// - module: amadeus::tools::file::FileTools
// - artifact: filesystem paths and files
// - runtime: tokio async runtime
// invariants:
// - Assertions stay aligned with current multi-session file-lock semantics.
// side_effects:
// - Reads or writes filesystem state.
// tests:
// - cmd: cargo test file_lock_sessions_test --features full
// @end-amadeus-header

use std::sync::Arc;
use std::time::Duration;

use amadeus::concurrency::FileLockManager;
use amadeus::core::id::AgentId;
use amadeus::error::AgentError;
use amadeus::tools::file::FileTools;
use tempfile::TempDir;

fn session_tools(temp_dir: &TempDir, manager: Arc<FileLockManager>) -> FileTools {
    FileTools::new_with_locks(
        temp_dir.path().to_path_buf(),
        16_384,
        manager,
        AgentId::new(),
    )
}

#[tokio::test]
async fn stale_edit_is_rejected_after_another_session_writes() {
    let temp_dir = TempDir::new().expect("temp dir");
    let file_path = temp_dir.path().join("shared.txt");
    tokio::fs::write(&file_path, "original")
        .await
        .expect("seed file");

    let manager = Arc::new(FileLockManager::with_timeout(Duration::from_millis(250)));
    let session_a = session_tools(&temp_dir, manager.clone());
    let session_b = session_tools(&temp_dir, manager.clone());
    let session_c = session_tools(&temp_dir, manager);

    let initial = session_a
        .read("shared.txt", None)
        .await
        .expect("session a read");
    assert_eq!(initial, "original");

    let write_result = session_b
        .write("shared.txt", "updated by session b")
        .await
        .expect("session b write");
    assert!(write_result.contains("Wrote"));

    let err = session_a
        .edit("shared.txt", "original", "session a overwrite", false)
        .await
        .expect_err("stale session a edit should fail");

    match err {
        AgentError::TextNotFound { path, snippet } => {
            assert_eq!(path, "shared.txt");
            assert_eq!(snippet, "original");
        }
        other => panic!("expected TextNotFound error, got {other:?}"),
    }

    let latest = session_c
        .read("shared.txt", None)
        .await
        .expect("session c read");
    assert_eq!(latest, "updated by session b");
}

#[tokio::test]
async fn waiting_reader_proceeds_after_writer_session_releases_lock() {
    let temp_dir = TempDir::new().expect("temp dir");
    let file_path = temp_dir.path().join("shared.txt");
    tokio::fs::write(&file_path, "content")
        .await
        .expect("seed file");

    let manager = Arc::new(FileLockManager::with_timeout(Duration::from_millis(500)));
    let writer_session = AgentId::new();
    let reader_session = AgentId::new();
    let path = file_path.to_string_lossy().to_string();

    let write_guard = manager
        .acquire_write(writer_session, &path)
        .await
        .expect("writer lock");

    let waiting_manager = manager.clone();
    let waiting_path = path.clone();
    let waiting_reader = tokio::spawn(async move {
        waiting_manager
            .acquire_read_with_timeout(reader_session, &waiting_path, Duration::from_millis(400))
            .await
    });

    let early = tokio::time::timeout(Duration::from_millis(75), waiting_reader).await;
    assert!(
        early.is_err(),
        "reader should still be blocked while writer holds lock"
    );

    drop(write_guard);

    let read_guard = manager
        .acquire_read_with_timeout(reader_session, &path, Duration::from_millis(400))
        .await
        .expect("reader lock after writer release");
    drop(read_guard);
}
