use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use crate::core::id::{AgentId, TxId};
use crate::core::state::VersionedState;
use crate::error::Result;

#[derive(Debug, Clone)]
pub enum Operation {
    Write {
        key: String,
        value: serde_json::Value,
    },
    Delete {
        key: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxState {
    Active,
    Committed,
    RolledBack,
}

#[derive(Debug)]
pub struct Transaction {
    pub id: TxId,
    pub initiator: AgentId,
    pub operations: Vec<Operation>,
    pub state: TxState,
    pub started_at: Instant,
    pub timeout: Duration,
    pub snapshot: HashMap<String, serde_json::Value>,
}

impl Transaction {
    pub fn new(id: TxId, initiator: AgentId, timeout: Duration) -> Self {
        Self {
            id,
            initiator,
            operations: Vec::new(),
            state: TxState::Active,
            started_at: Instant::now(),
            timeout,
            snapshot: HashMap::new(),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.started_at.elapsed() > self.timeout
    }

    pub fn is_active(&self) -> bool {
        self.state == TxState::Active && !self.is_expired()
    }
}

pub struct TransactionManager {
    active: HashMap<TxId, Transaction>,
    state: Arc<RwLock<VersionedState>>,
    default_timeout: Duration,
}

impl TransactionManager {
    pub fn new(state: Arc<RwLock<VersionedState>>) -> Self {
        Self {
            active: HashMap::new(),
            state,
            default_timeout: Duration::from_secs(300),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    pub async fn begin(&mut self, initiator: AgentId) -> TxId {
        self.begin_with_timeout(initiator, self.default_timeout)
            .await
    }

    pub async fn begin_with_timeout(&mut self, initiator: AgentId, timeout: Duration) -> TxId {
        let id = TxId::new();
        let mut tx = Transaction::new(id, initiator, timeout);

        {
            let state = self.state.read().await;
            for key in state.keys() {
                if let Some(value) = state.read(key) {
                    tx.snapshot.insert(key.clone(), value.clone());
                }
            }
        }

        self.active.insert(id, tx);
        id
    }

    pub async fn write(&mut self, tx_id: TxId, key: &str, value: serde_json::Value) -> Result<()> {
        let tx = self.active.get_mut(&tx_id).ok_or_else(|| {
            crate::error::AgentError::Api(format!("Transaction {:?} not found", tx_id))
        })?;

        if !tx.is_active() {
            return Err(crate::error::AgentError::Api(format!(
                "Transaction {:?} is not active",
                tx_id
            )));
        }

        tx.operations.push(Operation::Write {
            key: key.to_string(),
            value,
        });

        Ok(())
    }

    pub async fn delete(&mut self, tx_id: TxId, key: &str) -> Result<()> {
        let tx = self.active.get_mut(&tx_id).ok_or_else(|| {
            crate::error::AgentError::Api(format!("Transaction {:?} not found", tx_id))
        })?;

        if !tx.is_active() {
            return Err(crate::error::AgentError::Api(format!(
                "Transaction {:?} is not active",
                tx_id
            )));
        }

        tx.operations.push(Operation::Delete {
            key: key.to_string(),
        });

        Ok(())
    }

    pub async fn commit(&mut self, tx_id: TxId) -> Result<()> {
        let tx = self.active.get_mut(&tx_id).ok_or_else(|| {
            crate::error::AgentError::Api(format!("Transaction {:?} not found", tx_id))
        })?;

        if !tx.is_active() {
            return Err(crate::error::AgentError::Api(format!(
                "Transaction {:?} is not active",
                tx_id
            )));
        }

        let operations = tx.operations.clone();
        tx.state = TxState::Committed;

        {
            let mut state = self.state.write().await;
            for op in operations {
                match op {
                    Operation::Write { key, value } => {
                        state.write(&key, value);
                    }
                    Operation::Delete { key } => {
                        state.delete(&key);
                    }
                }
            }
        }

        self.active.remove(&tx_id);
        Ok(())
    }

    pub async fn rollback(&mut self, tx_id: TxId, _reason: &str) -> Result<()> {
        let tx = self.active.get_mut(&tx_id).ok_or_else(|| {
            crate::error::AgentError::Api(format!("Transaction {:?} not found", tx_id))
        })?;

        tx.state = TxState::RolledBack;
        self.active.remove(&tx_id);

        Ok(())
    }

    pub fn status(&self, tx_id: TxId) -> Option<TxState> {
        self.active.get(&tx_id).map(|tx| tx.state)
    }

    pub fn active_count(&self) -> usize {
        self.active.iter().filter(|(_, tx)| tx.is_active()).count()
    }

    pub fn list_active(&self) -> Vec<(TxId, AgentId, Duration)> {
        self.active
            .iter()
            .filter(|(_, tx)| tx.is_active())
            .map(|(id, tx)| (*id, tx.initiator, tx.started_at.elapsed()))
            .collect()
    }

    pub async fn cleanup_expired(&mut self) -> Vec<TxId> {
        let expired: Vec<TxId> = self
            .active
            .iter()
            .filter(|(_, tx)| tx.is_expired())
            .map(|(id, _)| *id)
            .collect();

        for tx_id in &expired {
            self.active.remove(tx_id);
        }

        expired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transaction_begin() {
        let state = Arc::new(RwLock::new(VersionedState::new()));
        let mut tm = TransactionManager::new(state);
        let agent = AgentId::new();

        let tx_id = tm.begin(agent).await;
        assert!(tm.status(tx_id).is_some());
    }

    #[tokio::test]
    async fn test_transaction_commit() {
        let state = Arc::new(RwLock::new(VersionedState::new()));
        let mut tm = TransactionManager::new(state.clone());
        let agent = AgentId::new();

        let tx_id = tm.begin(agent).await;
        tm.write(tx_id, "key", serde_json::json!("value"))
            .await
            .unwrap();
        tm.commit(tx_id).await.unwrap();

        let state_guard = state.read().await;
        assert_eq!(state_guard.read("key"), Some(&serde_json::json!("value")));
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        let state = Arc::new(RwLock::new(VersionedState::new()));
        let mut tm = TransactionManager::new(state.clone());
        let agent = AgentId::new();

        let tx_id = tm.begin(agent).await;
        tm.write(tx_id, "key", serde_json::json!("value"))
            .await
            .unwrap();
        tm.rollback(tx_id, "test rollback").await.unwrap();

        let state_guard = state.read().await;
        assert_eq!(state_guard.read("key"), None);
    }

    #[tokio::test]
    async fn test_transaction_not_found() {
        let state = Arc::new(RwLock::new(VersionedState::new()));
        let mut tm = TransactionManager::new(state);

        let result = tm
            .write(TxId::new(), "key", serde_json::json!("value"))
            .await;
        assert!(result.is_err());
    }
}
