//! # File Lock Manager
//!
//! Provides read-write locking for file operations and tracks per-agent read caches
//! to detect concurrent modifications.
//!
//! ## Features
//!
//! - **RW Locking**: Multiple readers allowed, exclusive writer
//! - **Read Cache**: Tracks last read time + file modification time per agent
//! - **Modification Detection**: Validates file wasn't modified since last read
//! - **Timeout Support**: Configurable lock acquisition timeout

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use tokio::sync::RwLock;
use tracing::debug;

use crate::core::id::AgentId;
use crate::error::{AgentError, Result};

/// Format SystemTime to ISO 8601 string for error messages.
fn format_system_time(time: SystemTime) -> String {
    let duration_since_epoch = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration_since_epoch.as_secs();
    let nanos = duration_since_epoch.subsec_nanos();

    // Convert to chrono DateTime for formatting
    use chrono::{DateTime, Utc};
    let datetime = DateTime::<Utc>::from_timestamp(secs as i64, nanos);
    datetime
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
        .unwrap_or_else(|| format!("{secs}s"))
}

/// Information about a cached file read.
#[derive(Debug, Clone)]
pub struct FileReadInfo {
    /// When the file was read.
    pub read_at: Instant,
    /// File modification time at the time of read (from metadata).
    pub modified_at: SystemTime,
    /// The content hash (optional, for additional validation).
    pub content_hash: Option<u64>,
}

/// Per-file lock state.
#[derive(Debug)]
struct FileLock {
    /// State encoding writer bit and reader count.
    /// bit 0 = writer active (1 = active, 0 = inactive)
    /// bits 1+ = reader count
    state: AtomicUsize,
}

impl FileLock {
    fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
        }
    }
}

/// Manages file locks and read caches for concurrent file access.
#[derive(Debug)]
pub struct FileLockManager {
    /// Per-file RW locks.
    locks: RwLock<HashMap<String, Arc<FileLock>>>,
    /// Per-agent read cache: AgentId -> (file_path -> read info).
    read_cache: RwLock<HashMap<AgentId, HashMap<String, FileReadInfo>>>,
    /// Default lock timeout.
    default_timeout: Duration,
}

impl Default for FileLockManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FileLockManager {
    /// Create a new FileLockManager.
    pub fn new() -> Self {
        Self {
            locks: RwLock::new(HashMap::new()),
            read_cache: RwLock::new(HashMap::new()),
            default_timeout: Duration::from_secs(30),
        }
    }

    /// Create with custom default timeout.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            locks: RwLock::new(HashMap::new()),
            read_cache: RwLock::new(HashMap::new()),
            default_timeout: timeout,
        }
    }

    /// Get or create a file lock for the given path.
    async fn get_lock(&self, path: &str) -> Arc<FileLock> {
        let mut locks: tokio::sync::RwLockWriteGuard<'_, HashMap<String, Arc<FileLock>>> =
            self.locks.write().await;
        locks
            .entry(path.to_string())
            .or_insert_with(|| Arc::new(FileLock::new()))
            .clone()
    }

    /// Acquire a shared (read) lock on a file.
    ///
    /// Multiple readers can hold the lock simultaneously.
    /// Returns a guard that must be dropped to release the lock.
    pub async fn acquire_read(&self, agent_id: AgentId, path: &str) -> Result<FileReadGuard> {
        self.acquire_read_with_timeout(agent_id, path, self.default_timeout).await
    }

    /// Acquire a shared (read) lock with custom timeout.
    pub async fn acquire_read_with_timeout(
        &self,
        agent_id: AgentId,
        path: &str,
        timeout: Duration,
    ) -> Result<FileReadGuard> {
        let lock = self.get_lock(path).await;

        // State encoding: bit 0 = writer active, bits 1+ = reader count
        const WRITER_BIT: usize = 1;

        // Wait for any writer to finish and increment reader count
        let start = Instant::now();
        loop {
            let current = lock.state.load(Ordering::SeqCst);
            let writer_active = (current & WRITER_BIT) != 0;
            let reader_count = current >> 1;

            if !writer_active {
                // Try to increment reader count
                let new = (reader_count + 1) << 1;
                if lock.state.compare_exchange(current, new, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                    debug!(agent_id = %agent_id, path = %path, "Acquired read lock");
                    return Ok(FileReadGuard {
                        agent_id,
                        path: path.to_string(),
                        lock: lock.clone(),
                    });
                }
                // CAS failed, retry
                continue;
            }

            if start.elapsed() >= timeout {
                return Err(AgentError::Lock(format!("Timeout acquiring read lock for {path}")));
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Acquire an exclusive (write) lock on a file.
    ///
    /// Blocks all readers and other writers.
    /// Returns a guard that must be dropped to release the lock.
    pub async fn acquire_write(&self, agent_id: AgentId, path: &str) -> Result<FileWriteGuard> {
        self.acquire_write_with_timeout(agent_id, path, self.default_timeout).await
    }

    /// Acquire an exclusive (write) lock with custom timeout.
    pub async fn acquire_write_with_timeout(
        &self,
        agent_id: AgentId,
        path: &str,
        timeout: Duration,
    ) -> Result<FileWriteGuard> {
        let lock = self.get_lock(path).await;

        // State encoding: bit 0 = writer active, bits 1+ = reader count
        const WRITER_BIT: usize = 1;

        // Acquire exclusive writer lock using CAS
        let start = Instant::now();
        loop {
            let current = lock.state.load(Ordering::SeqCst);
            let writer_active = (current & WRITER_BIT) != 0;
            let reader_count = current >> 1;

            if writer_active || reader_count > 0 {
                // Writer active or readers exist, wait
                if start.elapsed() >= timeout {
                    return Err(AgentError::Lock(format!("Timeout acquiring write lock for {path}")));
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
                continue;
            }

            // Try to set writer bit
            let new = current | WRITER_BIT;
            if lock.state.compare_exchange(current, new, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                debug!(agent_id = %agent_id, path = %path, "Acquired write lock");
                return Ok(FileWriteGuard {
                    path: path.to_string(),
                    lock: lock.clone(),
                });
            }
            // CAS failed, retry
        }
    }

    /// Validate that the file hasn't been modified since the agent last read it.
    ///
    /// Returns Ok if the file can be safely written to, Err if it may have been
    /// modified by another agent since the last read.
    pub async fn validate_read_freshness(&self, agent_id: AgentId, path: &str) -> Result<()> {
        let cache = self.read_cache.read().await;

        if let Some(agent_cache) = cache.get(&agent_id) {
            if let Some(read_info) = agent_cache.get(path) {
                // Get current file modification time
                let current_modified =
                    tokio::fs::metadata(path)
                        .await
                        .and_then(|m| m.modified())
                        .map_err(|e| AgentError::Io(std::io::Error::other(e.to_string())))?;

                if current_modified > read_info.modified_at {
                    return Err(AgentError::FileModified {
                        path: path.to_string(),
                        read_at: format_system_time(read_info.modified_at),
                        modified_at: format_system_time(current_modified),
                    });
                }
            }
        }

        Ok(())
    }

    /// Cache a file read with its modification time.
    pub async fn cache_read(
        &self,
        agent_id: AgentId,
        path: &str,
        modified_at: SystemTime,
        content_hash: Option<u64>,
    ) {
        let mut cache = self.read_cache.write().await;
        let agent_cache = cache.entry(agent_id).or_insert_with(HashMap::new);

        agent_cache.insert(
            path.to_string(),
            FileReadInfo {
                read_at: Instant::now(),
                modified_at,
                content_hash,
            },
        );

        debug!(agent_id = %agent_id, path = %path, "Cached file read");
    }

    /// Clear the read cache for an agent (useful when agent finishes).
    pub async fn clear_agent_cache(&self, agent_id: AgentId) {
        let mut cache = self.read_cache.write().await;
        cache.remove(&agent_id);
        debug!(agent_id = %agent_id, "Cleared file read cache");
    }

    /// Clear cache for a specific file (useful when file is written).
    pub async fn invalidate_file_cache(&self, path: &str) {
        let mut cache = self.read_cache.write().await;
        for agent_cache in cache.values_mut() {
            agent_cache.remove(path);
        }
        debug!(path = %path, "Invalidated file cache");
    }

    /// Get lock statistics for debugging.
    pub async fn stats(&self) -> FileLockStats {
        let locks = self.locks.read().await;
        let cache = self.read_cache.read().await;

        FileLockStats {
            active_locks: locks.len(),
            agents_with_cache: cache.len(),
        }
    }

    /// Clone the file lock manager if it exists.
    pub fn clone_manager(&self) -> Option<Arc<FileLockManager>> {
        None // Placeholder - Arc is already cloned via clone()
    }

    /// Check if file locking is enabled.
    pub fn is_enabled(&self) -> bool {
        true
    }
}

/// Guard for releasing a read lock.
#[derive(Debug)]
pub struct FileReadGuard {
    agent_id: AgentId,
    path: String,
    lock: Arc<FileLock>,
}

impl FileReadGuard {
    /// Record that we read the file (for modification tracking).
    ///
    /// Call this after successfully reading the file content.
    pub async fn record_read(self, manager: &FileLockManager, modified_at: SystemTime, content_hash: Option<u64>) {
        manager
            .cache_read(self.agent_id, &self.path, modified_at, content_hash)
            .await;
    }
}

impl Drop for FileReadGuard {
    fn drop(&mut self) {
        // Decrement reader count
        loop {
            let current = self.lock.state.load(Ordering::SeqCst);
            let reader_count = current >> 1;
            if reader_count == 0 {
                return;
            }
            let new = (reader_count - 1) << 1;
            if self.lock.state.compare_exchange(current, new, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                return;
            }
        }
    }
}

/// Guard for releasing a write lock.
#[derive(Debug)]
pub struct FileWriteGuard {
    path: String,
    lock: Arc<FileLock>,
}

impl FileWriteGuard {
    /// Invalidate caches for this file after writing.
    ///
    /// Call this after successfully writing to the file.
    pub async fn invalidate_after_write(self, manager: &FileLockManager) {
        manager.invalidate_file_cache(&self.path).await;
    }
}

impl Drop for FileWriteGuard {
    fn drop(&mut self) {
        // Clear writer bit
        loop {
            let current = self.lock.state.load(Ordering::SeqCst);
            let new = current & !1; // Clear bit 0
            if self.lock.state.compare_exchange(current, new, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                return;
            }
        }
    }
}

/// Statistics about the file lock manager.
#[derive(Debug)]
pub struct FileLockStats {
    /// Number of files currently locked.
    pub active_locks: usize,
    /// Number of agents with cached reads.
    pub agents_with_cache: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn test_agent() -> AgentId {
        AgentId(Uuid::new_v4())
    }

    #[tokio::test]
    async fn test_multiple_readers() {
        let manager = FileLockManager::new();
        let agent1 = test_agent();
        let agent2 = test_agent();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        std::fs::write(&file_path, "content").unwrap();

        // Two agents can read simultaneously
        let guard1 = manager
            .acquire_read(agent1, file_path.to_str().unwrap())
            .await;
        assert!(guard1.is_ok());

        let guard2 = manager
            .acquire_read(agent2, file_path.to_str().unwrap())
            .await;
        assert!(guard2.is_ok());

        // Drop guards
        drop(guard1);
        drop(guard2);
    }

    #[tokio::test]
    async fn test_write_blocks_readers() {
        let manager = FileLockManager::new();
        let agent1 = test_agent();
        let agent2 = test_agent();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        std::fs::write(&file_path, "content").unwrap();

        // Acquire write lock
        let _write_guard = manager
            .acquire_write(agent1, file_path.to_str().unwrap())
            .await
            .unwrap();

        // Read should timeout (writer holds exclusive lock)
        let read_result = manager
            .acquire_read_with_timeout(
                agent2,
                file_path.to_str().unwrap(),
                Duration::from_millis(100),
            )
            .await;

        assert!(read_result.is_err());
    }

    #[tokio::test]
    async fn test_read_freshness_validation() {
        let manager = FileLockManager::new();
        let agent = test_agent();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        std::fs::write(&file_path, "original").unwrap();
        let modified_at = std::fs::metadata(&file_path).unwrap().modified().unwrap();

        // Read and cache
        {
            let _guard = manager
                .acquire_read(agent, file_path.to_str().unwrap())
                .await
                .unwrap();
            manager
                .cache_read(agent, file_path.to_str().unwrap(), modified_at, None)
                .await;
        }

        // Modify file externally
        tokio::time::sleep(Duration::from_millis(10)).await;
        std::fs::write(&file_path, "modified").unwrap();

        // Validation should fail
        let result = manager
            .validate_read_freshness(agent, file_path.to_str().unwrap())
            .await;
        assert!(result.is_err());
    }
}
