use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::id::AgentId;

pub type Version = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub version: Version,
    pub data: BTreeMap<String, serde_json::Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl StateSnapshot {
    pub fn new(version: Version, data: BTreeMap<String, serde_json::Value>) -> Self {
        Self {
            version,
            data,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn empty() -> Self {
        Self {
            version: 0,
            data: BTreeMap::new(),
            timestamp: chrono::Utc::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VersionedState {
    data: BTreeMap<String, (serde_json::Value, Version)>,
    version: Version,
}

impl VersionedState {
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
            version: 0,
        }
    }

    pub fn from_snapshot(snapshot: StateSnapshot) -> Self {
        let data = snapshot
            .data
            .into_iter()
            .map(|(k, v)| (k, (v, snapshot.version)))
            .collect();
        Self {
            data,
            version: snapshot.version,
        }
    }

    pub fn version(&self) -> Version {
        self.version
    }

    pub fn read(&self, key: &str) -> Option<&serde_json::Value> {
        self.data.get(key).map(|(v, _)| v)
    }

    pub fn write(&mut self, key: &str, value: serde_json::Value) -> Version {
        self.version += 1;
        self.data.insert(key.to_string(), (value, self.version));
        self.version
    }

    pub fn delete(&mut self, key: &str) -> Option<(serde_json::Value, Version)> {
        self.data.remove(key)
    }

    pub fn cas(
        &mut self,
        key: &str,
        expected: Option<&serde_json::Value>,
        new: serde_json::Value,
    ) -> Result<bool, String> {
        let current = self.data.get(key).map(|(v, _)| v);

        match (current, expected) {
            (Some(current), Some(expected)) if current == expected => {
                self.version += 1;
                self.data.insert(key.to_string(), (new, self.version));
                Ok(true)
            }
            (None, None) => {
                self.version += 1;
                self.data.insert(key.to_string(), (new, self.version));
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub fn update<F>(&mut self, key: &str, f: F) -> serde_json::Value
    where
        F: FnOnce(Option<&serde_json::Value>) -> serde_json::Value,
    {
        let current = self.data.get(key).map(|(v, _)| v);
        let new_value = f(current);
        self.version += 1;
        self.data
            .insert(key.to_string(), (new_value.clone(), self.version));
        new_value
    }

    pub fn read_batch(&self, keys: &[&str]) -> BTreeMap<String, Option<serde_json::Value>> {
        keys.iter()
            .map(|k| (k.to_string(), self.read(k).cloned()))
            .collect()
    }

    pub fn write_batch(&mut self, updates: BTreeMap<String, serde_json::Value>) {
        self.version += 1;
        for (key, value) in updates {
            self.data.insert(key, (value, self.version));
        }
    }

    pub fn snapshot(&self) -> StateSnapshot {
        let data: BTreeMap<String, serde_json::Value> = self
            .data
            .iter()
            .map(|(k, (v, _))| (k.clone(), v.clone()))
            .collect();
        StateSnapshot::new(self.version, data)
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.data.keys()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl Default for VersionedState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    pub key: String,
    pub old: Option<serde_json::Value>,
    pub new: serde_json::Value,
    pub author: AgentId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_state_write_read() {
        let mut state = VersionedState::new();
        state.write("key1", json!("value1"));

        assert_eq!(state.read("key1"), Some(&json!("value1")));
        assert_eq!(state.version(), 1);
    }

    #[test]
    fn test_state_cas_success() {
        let mut state = VersionedState::new();
        state.write("key1", json!("value1"));

        let result = state.cas("key1", Some(&json!("value1")), json!("value2"));
        assert!(result.unwrap());
        assert_eq!(state.read("key1"), Some(&json!("value2")));
    }

    #[test]
    fn test_state_cas_failure() {
        let mut state = VersionedState::new();
        state.write("key1", json!("value1"));

        let result = state.cas("key1", Some(&json!("wrong")), json!("value2"));
        assert!(!result.unwrap());
        assert_eq!(state.read("key1"), Some(&json!("value1")));
    }

    #[test]
    fn test_state_cas_new_key() {
        let mut state = VersionedState::new();

        let result = state.cas("new_key", None, json!("value"));
        assert!(result.unwrap());
        assert_eq!(state.read("new_key"), Some(&json!("value")));
    }

    #[test]
    fn test_state_snapshot() {
        let mut state = VersionedState::new();
        state.write("key1", json!("value1"));
        state.write("key2", json!(42));

        let snapshot = state.snapshot();
        assert_eq!(snapshot.version, 2);
        assert_eq!(snapshot.data.len(), 2);
    }

    #[test]
    fn test_state_from_snapshot() {
        let mut state = VersionedState::new();
        state.write("key1", json!("value1"));

        let snapshot = state.snapshot();
        let restored = VersionedState::from_snapshot(snapshot);

        assert_eq!(restored.read("key1"), Some(&json!("value1")));
    }

    #[test]
    fn test_state_update() {
        let mut state = VersionedState::new();

        let result = state.update("counter", |v| {
            json!(v.and_then(|v| v.as_u64()).unwrap_or(0) + 1)
        });

        assert_eq!(result, json!(1));
        assert_eq!(state.read("counter"), Some(&json!(1)));

        state.update("counter", |v| {
            json!(v.and_then(|v| v.as_u64()).unwrap_or(0) + 1)
        });

        assert_eq!(state.read("counter"), Some(&json!(2)));
    }

    #[test]
    fn test_state_batch_operations() {
        let mut state = VersionedState::new();

        let mut batch = BTreeMap::new();
        batch.insert("key1".to_string(), json!("value1"));
        batch.insert("key2".to_string(), json!("value2"));

        state.write_batch(batch);

        assert_eq!(state.len(), 2);
        assert_eq!(state.version(), 1);
    }
}
