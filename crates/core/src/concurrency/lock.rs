// @amadeus-header
// summary: Concurrency utility code for lock.
// layer: infra
// status: active
// feature_flags:
// - concurrency
// provides:
// - module: crate::concurrency::lock
// - type: crate::concurrency::lock::LockMode
// - type: crate::concurrency::lock::LockEntry
// - type: crate::concurrency::lock::LockStatus
// - type: crate::concurrency::lock::LockError
// - type: crate::concurrency::lock::LockManager
// uses:
// - module: crate::core::id::AgentId
// - runtime: tokio async runtime
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Sends or receives messages across async channels.
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

//! # Resource Lock Manager
//!
//! Provides resource-level locking for coordinating concurrent operations.
//!
//! ## Lock Modes
//!
//! - **Shared**: Multiple readers can hold simultaneously
//! - **Exclusive**: Single writer, blocks all others
//!
//! ## Lock Compatibility
//!
//! | Held Mode | Requested | Allowed? |
//! |-----------|-----------|----------|
//! | Shared    | Shared    | ✅       |
//! | Shared    | Exclusive | ❌       |
//! | Exclusive | Shared    | ❌       |
//! | Exclusive | Exclusive | ❌       |

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use tokio::time::timeout;

use crate::core::id::AgentId;
use thiserror::Error;

/// Lock mode for resource access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    /// Multiple readers can hold simultaneously.
    Shared,
    /// Single writer, blocks all others.
    Exclusive,
}

/// Information about a held lock.
#[derive(Debug, Clone)]
pub struct LockEntry {
    /// Agent holding the lock.
    pub holder: AgentId,
    /// Lock mode (Shared or Exclusive).
    pub mode: LockMode,
    /// When the lock was acquired.
    pub acquired_at: Instant,
    /// Lock timeout duration.
    pub timeout: Duration,
    /// Resource identifier.
    pub resource: String,
}

/// Status of a lock for inspection.
#[derive(Debug, Clone)]
pub struct LockStatus {
    /// Resource identifier.
    pub resource: String,
    /// Current lock holders.
    pub holders: Vec<LockEntry>,
    /// Number of agents waiting for this lock.
    pub waiters: usize,
    /// Whether the resource is locked.
    pub is_locked: bool,
}

/// Errors that can occur during lock operations.
#[derive(Debug, Error)]
pub enum LockError {
    /// Resource is already locked by another agent.
    #[error("Resource '{0}' is already locked")]
    AlreadyLocked(String),

    /// Lock acquisition timed out.
    #[error("Lock acquisition timed out for resource '{0}'")]
    Timeout(String),

    /// Agent does not hold the lock.
    #[error("Agent {0} does not hold lock on '{1}'")]
    NotHeld(AgentId, String),

    /// Deadlock detected.
    #[error("Deadlock detected: {0}")]
    Deadlock(String),
}

/// Internal waiter for lock queue.
struct Waiter {
    agent: AgentId,
    mode: LockMode,
    tx: tokio::sync::oneshot::Sender<bool>,
}

/// Resource-level lock manager.
///
/// Manages locks on named resources with support for:
/// - Shared and exclusive lock modes
/// - Wait queue with timeout
/// - Lock status inspection
///
/// # Example
///
/// ```rust,ignore
/// use amadeus::concurrency::{LockManager, LockMode};
///
/// let mut manager = LockManager::new();
///
/// // Acquire exclusive lock
/// manager.acquire("/file.txt", agent_id, LockMode::Exclusive, Duration::from_secs(30)).await?;
///
/// // ... do work ...
///
/// // Release
/// manager.release("/file.txt", agent_id)?;
/// ```
pub struct LockManager {
    /// Active locks per resource (resource -> list of holders).
    locks: HashMap<String, Vec<LockEntry>>,
    /// Wait queue per resource.
    wait_queue: HashMap<String, VecDeque<Waiter>>,
}

impl LockManager {
    /// Create a new lock manager.
    pub fn new() -> Self {
        Self {
            locks: HashMap::new(),
            wait_queue: HashMap::new(),
        }
    }

    /// Try to acquire a lock without waiting.
    ///
    /// Returns immediately with success or failure.
    ///
    /// # Arguments
    ///
    /// * `resource` - Resource identifier (e.g., file path, key name).
    /// * `holder` - Agent requesting the lock.
    /// * `mode` - Lock mode (Shared or Exclusive).
    ///
    /// # Returns
    ///
    /// `Ok(())` if lock was acquired, `Err(LockError)` otherwise.
    pub fn try_acquire(
        &mut self,
        resource: &str,
        holder: AgentId,
        mode: LockMode,
    ) -> Result<(), LockError> {
        if self.can_acquire(resource, holder, mode) {
            let entry = LockEntry {
                holder,
                mode,
                acquired_at: Instant::now(),
                timeout: Duration::from_secs(300),
                resource: resource.to_string(),
            };
            self.locks
                .entry(resource.to_string())
                .or_default()
                .push(entry);
            Ok(())
        } else {
            Err(LockError::AlreadyLocked(resource.to_string()))
        }
    }

    /// Acquire a lock with timeout.
    ///
    /// If the lock cannot be acquired immediately, waits in queue
    /// until either the lock becomes available or timeout expires.
    ///
    /// # Arguments
    ///
    /// * `resource` - Resource identifier.
    /// * `holder` - Agent requesting the lock.
    /// * `mode` - Lock mode (Shared or Exclusive).
    /// * `timeout_duration` - Maximum time to wait.
    ///
    /// # Returns
    ///
    /// `Ok(())` if lock was acquired, `Err(LockError)` on timeout or error.
    pub async fn acquire(
        &mut self,
        resource: &str,
        holder: AgentId,
        mode: LockMode,
        timeout_duration: Duration,
    ) -> Result<(), LockError> {
        if self.can_acquire(resource, holder, mode) {
            let entry = LockEntry {
                holder,
                mode,
                acquired_at: Instant::now(),
                timeout: timeout_duration,
                resource: resource.to_string(),
            };
            self.locks
                .entry(resource.to_string())
                .or_default()
                .push(entry);
            return Ok(());
        }

        let (tx, rx) = tokio::sync::oneshot::channel();

        self.wait_queue
            .entry(resource.to_string())
            .or_default()
            .push_back(Waiter {
                agent: holder,
                mode,
                tx,
            });

        match timeout(timeout_duration, rx).await {
            Ok(Ok(true)) => {
                let entry = LockEntry {
                    holder,
                    mode,
                    acquired_at: Instant::now(),
                    timeout: timeout_duration,
                    resource: resource.to_string(),
                };
                self.locks
                    .entry(resource.to_string())
                    .or_default()
                    .push(entry);
                Ok(())
            }
            Ok(Ok(false)) | Ok(Err(_)) => Err(LockError::Timeout(resource.to_string())),
            Err(_) => {
                self.remove_waiter(resource, holder);
                Err(LockError::Timeout(resource.to_string()))
            }
        }
    }

    /// Release a held lock.
    ///
    /// # Arguments
    ///
    /// * `resource` - Resource identifier.
    /// * `holder` - Agent releasing the lock.
    ///
    /// # Returns
    ///
    /// `Ok(())` if released, `Err(LockError)` if agent didn't hold the lock.
    pub fn release(&mut self, resource: &str, holder: AgentId) -> Result<(), LockError> {
        let holders = self.locks.get_mut(resource);

        if let Some(holders) = holders {
            let initial_len = holders.len();
            holders.retain(|e| e.holder != holder);

            if holders.len() == initial_len {
                return Err(LockError::NotHeld(holder, resource.to_string()));
            }

            if holders.is_empty() {
                self.locks.remove(resource);
            }

            self.notify_waiters(resource);
            Ok(())
        } else {
            Err(LockError::NotHeld(holder, resource.to_string()))
        }
    }

    /// Release all locks held by an agent.
    ///
    /// Useful when an agent is terminated or errors out.
    pub fn release_all(&mut self, holder: AgentId) {
        let mut resources_to_clean = Vec::new();

        for (resource, holders) in self.locks.iter_mut() {
            holders.retain(|e| e.holder != holder);
            if holders.is_empty() {
                resources_to_clean.push(resource.clone());
            }
        }

        for resource in resources_to_clean {
            self.locks.remove(&resource);
            self.notify_waiters(&resource);
        }

        for (_, queue) in self.wait_queue.iter_mut() {
            queue.retain(|w| w.agent != holder);
        }
    }

    /// Check if a resource is locked.
    pub fn is_locked(&self, resource: &str) -> bool {
        self.locks.get(resource).is_some_and(|h| !h.is_empty())
    }

    /// Check if a specific agent holds a lock on a resource.
    pub fn is_held_by(&self, resource: &str, holder: AgentId) -> bool {
        self.locks
            .get(resource)
            .is_some_and(|holders| holders.iter().any(|e| e.holder == holder))
    }

    /// Get lock status for a resource.
    pub fn status(&self, resource: &str) -> Option<LockStatus> {
        let holders = self.locks.get(resource)?;
        let waiters = self.wait_queue.get(resource).map_or(0, |q| q.len());

        Some(LockStatus {
            resource: resource.to_string(),
            holders: holders.clone(),
            waiters,
            is_locked: !holders.is_empty(),
        })
    }

    /// List all held locks.
    pub fn all_locks(&self) -> Vec<LockStatus> {
        let mut statuses = Vec::new();

        for resource in self.locks.keys() {
            if let Some(status) = self.status(resource) {
                statuses.push(status);
            }
        }

        statuses
    }

    /// Get count of active locks.
    pub fn lock_count(&self) -> usize {
        self.locks.values().filter(|h| !h.is_empty()).count()
    }

    /// Get count of waiting agents.
    pub fn waiter_count(&self) -> usize {
        self.wait_queue.values().map(|q| q.len()).sum()
    }

    /// Check if a lock can be acquired.
    fn can_acquire(&self, resource: &str, _holder: AgentId, mode: LockMode) -> bool {
        let holders = match self.locks.get(resource) {
            Some(h) if !h.is_empty() => h,
            _ => return true,
        };

        for entry in holders {
            match (entry.mode, mode) {
                // Exclusive lock blocks everything
                (LockMode::Exclusive, _) => return false,
                // Shared + Exclusive request is blocked
                (LockMode::Shared, LockMode::Exclusive) => return false,
                // Shared + Shared is allowed for different holders
                (LockMode::Shared, LockMode::Shared) => {}
            }
        }

        true
    }

    /// Remove a waiter from the queue.
    fn remove_waiter(&mut self, resource: &str, holder: AgentId) {
        if let Some(queue) = self.wait_queue.get_mut(resource) {
            queue.retain(|w| w.agent != holder);
        }
    }

    /// Notify waiting agents that a lock was released.
    fn notify_waiters(&mut self, resource: &str) {
        let queue = self.wait_queue.remove(resource);
        let mut queue = match queue {
            Some(q) => q,
            None => return,
        };

        while let Some(waiter) = queue.front() {
            if self.can_acquire(resource, waiter.agent, waiter.mode) {
                let waiter = queue.pop_front().unwrap();
                let _ = waiter.tx.send(true);
            } else {
                break;
            }
        }

        if !queue.is_empty() {
            self.wait_queue.insert(resource.to_string(), queue);
        }
    }
}

impl Default for LockManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn test_agent(_id: u8) -> AgentId {
        AgentId::new()
    }

    #[test]
    fn test_try_acquire_exclusive() {
        let mut manager = LockManager::new();
        let agent = test_agent(1);

        let result = manager.try_acquire("/file.txt", agent, LockMode::Exclusive);
        assert!(result.is_ok());
        assert!(manager.is_locked("/file.txt"));
    }

    #[test]
    fn test_try_acquire_shared_multiple() {
        let mut manager = LockManager::new();
        let agent1 = test_agent(1);
        let agent2 = test_agent(2);

        let result1 = manager.try_acquire("/file.txt", agent1, LockMode::Shared);
        let result2 = manager.try_acquire("/file.txt", agent2, LockMode::Shared);

        assert!(result1.is_ok());
        assert!(result2.is_ok(), "Multiple shared locks should be allowed");

        let status = manager.status("/file.txt").unwrap();
        assert_eq!(status.holders.len(), 2);
    }

    #[test]
    fn test_try_acquire_exclusive_blocks_shared() {
        let mut manager = LockManager::new();
        let agent1 = test_agent(1);
        let agent2 = test_agent(2);

        manager
            .try_acquire("/file.txt", agent1, LockMode::Exclusive)
            .unwrap();
        let result = manager.try_acquire("/file.txt", agent2, LockMode::Shared);

        assert!(result.is_err());
    }

    #[test]
    fn test_release() {
        let mut manager = LockManager::new();
        let agent = test_agent(1);

        manager
            .try_acquire("/file.txt", agent, LockMode::Exclusive)
            .unwrap();
        let result = manager.release("/file.txt", agent);

        assert!(result.is_ok());
        assert!(!manager.is_locked("/file.txt"));
    }

    #[test]
    fn test_release_not_held() {
        let mut manager = LockManager::new();
        let agent1 = test_agent(1);
        let agent2 = test_agent(2);

        manager
            .try_acquire("/file.txt", agent1, LockMode::Exclusive)
            .unwrap();
        let result = manager.release("/file.txt", agent2);

        assert!(matches!(result, Err(LockError::NotHeld(_, _))));
    }

    #[test]
    fn test_release_all() {
        let mut manager = LockManager::new();
        let agent = test_agent(1);

        manager
            .try_acquire("/file1.txt", agent, LockMode::Exclusive)
            .unwrap();
        manager
            .try_acquire("/file2.txt", agent, LockMode::Exclusive)
            .unwrap();

        manager.release_all(agent);

        assert!(!manager.is_locked("/file1.txt"));
        assert!(!manager.is_locked("/file2.txt"));
    }

    #[test]
    fn test_lock_status() {
        let mut manager = LockManager::new();
        let agent = test_agent(1);

        manager
            .try_acquire("/file.txt", agent, LockMode::Exclusive)
            .unwrap();

        let status = manager.status("/file.txt").unwrap();
        assert!(status.is_locked);
        assert_eq!(status.holders.len(), 1);
        assert_eq!(status.holders[0].holder, agent);
    }

    #[test]
    fn test_all_locks() {
        let mut manager = LockManager::new();
        let agent = test_agent(1);

        manager
            .try_acquire("/file1.txt", agent, LockMode::Exclusive)
            .unwrap();
        manager
            .try_acquire("/file2.txt", agent, LockMode::Exclusive)
            .unwrap();

        let locks = manager.all_locks();
        assert_eq!(locks.len(), 2);
    }

    #[tokio::test]
    async fn test_acquire_with_timeout() {
        let mut manager = LockManager::new();
        let agent1 = test_agent(1);
        let agent2 = test_agent(2);

        manager
            .try_acquire("/file.txt", agent1, LockMode::Exclusive)
            .unwrap();

        let result = manager
            .acquire(
                "/file.txt",
                agent2,
                LockMode::Exclusive,
                Duration::from_millis(100),
            )
            .await;

        assert!(matches!(result, Err(LockError::Timeout(_))));
    }
}
