use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::core::event::LockMode;
use crate::core::id::AgentId;

#[derive(Debug, Clone)]
pub struct LockEntry {
    pub holder: AgentId,
    pub mode: LockMode,
    pub acquired_at: Instant,
    pub timeout: Duration,
}

#[derive(Debug)]
struct Waiter {
    agent: AgentId,
    mode: LockMode,
    tx: tokio::sync::oneshot::Sender<bool>,
}

#[derive(Debug)]
pub struct LockStatus {
    pub holder: AgentId,
    pub mode: LockMode,
    pub acquired_at: Instant,
    pub waiters: usize,
}

pub struct LockManager {
    locks: HashMap<String, LockEntry>,
    wait_queue: HashMap<String, VecDeque<Waiter>>,
}

impl LockManager {
    pub fn new() -> Self {
        Self {
            locks: HashMap::new(),
            wait_queue: HashMap::new(),
        }
    }

    pub fn try_acquire(
        &mut self,
        resource: &str,
        holder: AgentId,
        mode: LockMode,
        timeout_duration: Duration,
    ) -> bool {
        if let Some(existing) = self.locks.get(resource) {
            match (existing.mode, mode) {
                (LockMode::Exclusive, _) => return false,
                (LockMode::Shared, LockMode::Exclusive) => return false,
                (LockMode::Shared, LockMode::Shared) => {
                    if existing.holder != holder {
                        return false;
                    }
                }
            }
        }

        self.locks.insert(
            resource.to_string(),
            LockEntry {
                holder,
                mode,
                acquired_at: Instant::now(),
                timeout: timeout_duration,
            },
        );
        true
    }

    pub async fn acquire(
        &mut self,
        resource: &str,
        holder: AgentId,
        mode: LockMode,
        timeout_duration: Duration,
    ) -> Result<(), String> {
        if self.try_acquire(resource, holder, mode, timeout_duration) {
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
            Ok(Ok(granted)) => {
                if granted {
                    Ok(())
                } else {
                    Err(format!("Lock acquire failed for resource: {}", resource))
                }
            }
            Ok(Err(_)) => Err(format!("Lock acquire cancelled for resource: {}", resource)),
            Err(_) => {
                self.remove_waiter(resource, holder);
                Err(format!("Lock timeout for resource: {}", resource))
            }
        }
    }

    pub fn release(&mut self, resource: &str, holder: AgentId) -> Result<(), String> {
        if let Some(lock) = self.locks.get(resource) {
            if lock.holder != holder {
                return Err(format!(
                    "Lock not held by {:?} for resource: {}",
                    holder, resource
                ));
            }
            self.locks.remove(resource);
            self.grant_next_waiter(resource);
            Ok(())
        } else {
            Err(format!("No lock found for resource: {}", resource))
        }
    }

    pub fn status(&self, resource: &str) -> Option<LockStatus> {
        self.locks.get(resource).map(|lock| LockStatus {
            holder: lock.holder,
            mode: lock.mode,
            acquired_at: lock.acquired_at,
            waiters: self.wait_queue.get(resource).map(|q| q.len()).unwrap_or(0),
        })
    }

    pub fn force_release(&mut self, resource: &str) -> Result<(), String> {
        if self.locks.remove(resource).is_some() {
            self.grant_next_waiter(resource);
            Ok(())
        } else {
            Err(format!("No lock found for resource: {}", resource))
        }
    }

    pub fn held_locks(&self, agent: AgentId) -> Vec<String> {
        self.locks
            .iter()
            .filter(|(_, entry)| entry.holder == agent)
            .map(|(resource, _)| resource.clone())
            .collect()
    }

    pub fn release_all(&mut self, agent: AgentId) -> Vec<String> {
        let resources: Vec<String> = self.held_locks(agent);
        for resource in &resources {
            self.locks.remove(resource);
            self.grant_next_waiter(resource);
        }
        resources
    }

    fn grant_next_waiter(&mut self, resource: &str) {
        if let Some(queue) = self.wait_queue.get_mut(resource) {
            while let Some(waiter) = queue.pop_front() {
                if waiter.tx.send(true).is_ok() {
                    self.locks.insert(
                        resource.to_string(),
                        LockEntry {
                            holder: waiter.agent,
                            mode: waiter.mode,
                            acquired_at: Instant::now(),
                            timeout: Duration::from_secs(300),
                        },
                    );
                    break;
                }
            }
            if queue.is_empty() {
                self.wait_queue.remove(resource);
            }
        }
    }

    fn remove_waiter(&mut self, resource: &str, agent: AgentId) {
        if let Some(queue) = self.wait_queue.get_mut(resource) {
            queue.retain(|w| w.agent != agent);
            if queue.is_empty() {
                self.wait_queue.remove(resource);
            }
        }
    }
}

impl Default for LockManager {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedLockManager = Arc<Mutex<LockManager>>;

pub fn shared_lock_manager() -> SharedLockManager {
    Arc::new(Mutex::new(LockManager::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_acquire_exclusive() {
        let mut lm = LockManager::new();
        let agent = AgentId::new();

        let result = lm.try_acquire(
            "file.txt",
            agent,
            LockMode::Exclusive,
            Duration::from_secs(60),
        );
        assert!(result);
    }

    #[test]
    fn test_try_acquire_shared() {
        let mut lm = LockManager::new();
        let agent = AgentId::new();

        let result = lm.try_acquire("file.txt", agent, LockMode::Shared, Duration::from_secs(60));
        assert!(result);
    }

    #[test]
    fn test_exclusive_blocks_exclusive() {
        let mut lm = LockManager::new();
        let agent1 = AgentId::new();
        let agent2 = AgentId::new();

        lm.try_acquire(
            "file.txt",
            agent1,
            LockMode::Exclusive,
            Duration::from_secs(60),
        );
        let result = lm.try_acquire(
            "file.txt",
            agent2,
            LockMode::Exclusive,
            Duration::from_secs(60),
        );
        assert!(!result);
    }

    #[test]
    fn test_release() {
        let mut lm = LockManager::new();
        let agent = AgentId::new();

        lm.try_acquire(
            "file.txt",
            agent,
            LockMode::Exclusive,
            Duration::from_secs(60),
        );
        let result = lm.release("file.txt", agent);
        assert!(result.is_ok());
    }

    #[test]
    fn test_held_locks() {
        let mut lm = LockManager::new();
        let agent = AgentId::new();

        lm.try_acquire(
            "file1.txt",
            agent,
            LockMode::Exclusive,
            Duration::from_secs(60),
        );
        lm.try_acquire(
            "file2.txt",
            agent,
            LockMode::Exclusive,
            Duration::from_secs(60),
        );

        let held = lm.held_locks(agent);
        assert_eq!(held.len(), 2);
    }
}
