use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::branch::Branch;
use crate::core::commit::Commit;
use crate::core::error::{CoreError, Result};
use crate::core::event::Event;
use crate::core::id::CommitId;
use crate::core::state::StateSnapshot;

const WORKSPACE_DIR: &str = ".amadeus";
const SNAPSHOT_FILE: &str = "snapshot.json";
const EVENTS_FILE: &str = "events.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub id: uuid::Uuid,
    pub workdir: PathBuf,
    pub branches: HashMap<String, Branch>,
    pub active_branch: String,
    pub commits: HashMap<String, Commit>,
    pub state: StateSnapshot,
}

pub struct Storage {
    path: PathBuf,
}

impl Storage {
    pub fn new(workspace_path: &Path) -> Self {
        Self {
            path: workspace_path.join(WORKSPACE_DIR),
        }
    }

    pub fn init(&self) -> Result<()> {
        std::fs::create_dir_all(&self.path).map_err(CoreError::Io)?;
        Ok(())
    }

    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    pub fn save_snapshot(&self, snapshot: &WorkspaceSnapshot) -> Result<()> {
        let file = self.path.join(SNAPSHOT_FILE);
        let content = serde_json::to_string_pretty(snapshot)
            .map_err(|e| CoreError::Serialization(e.to_string()))?;
        std::fs::write(&file, content).map_err(CoreError::Io)?;
        Ok(())
    }

    pub fn load_snapshot(&self) -> Result<Option<WorkspaceSnapshot>> {
        let file = self.path.join(SNAPSHOT_FILE);
        if !file.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&file).map_err(CoreError::Io)?;
        let snapshot: WorkspaceSnapshot =
            serde_json::from_str(&content).map_err(|e| CoreError::Serialization(e.to_string()))?;
        Ok(Some(snapshot))
    }

    pub fn append_event(&self, event: &Event) -> Result<()> {
        let file = self.path.join(EVENTS_FILE);
        let line =
            serde_json::to_string(event).map_err(|e| CoreError::Serialization(e.to_string()))?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)
            .map_err(CoreError::Io)?;
        use std::io::Write;
        writeln!(file, "{}", line).map_err(CoreError::Io)?;
        Ok(())
    }

    pub fn load_events(&self) -> Result<Vec<Event>> {
        let file = self.path.join(EVENTS_FILE);
        if !file.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&file).map_err(CoreError::Io)?;
        content
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                serde_json::from_str(line).map_err(|e| CoreError::Serialization(e.to_string()))
            })
            .collect()
    }

    pub fn workspace_dir(&self) -> &Path {
        &self.path
    }

    pub fn snapshot_path(&self) -> PathBuf {
        self.path.join(SNAPSHOT_FILE)
    }

    pub fn events_path(&self) -> PathBuf {
        self.path.join(EVENTS_FILE)
    }
}

impl WorkspaceSnapshot {
    pub fn commit_map(&self) -> HashMap<CommitId, Commit> {
        self.commits
            .iter()
            .map(|(k, v)| {
                let id: CommitId = k.parse().unwrap_or_else(|_| CommitId::nil());
                (id, v.clone())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::id::AgentId;
    use std::collections::BTreeMap;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("amadeus-test-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn test_storage_init() {
        let dir = temp_dir();
        let storage = Storage::new(&dir);
        storage.init().unwrap();
        assert!(storage.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_storage_snapshot_roundtrip() {
        let dir = temp_dir();
        let storage = Storage::new(&dir);
        storage.init().unwrap();

        let mut commits = HashMap::new();
        let commit_id = CommitId::new().to_string();
        commits.insert(
            commit_id.clone(),
            Commit::new(
                None,
                StateSnapshot::empty(),
                "test".to_string(),
                AgentId::system(),
                crate::core::commit::CommitTrigger::UserRequest,
            ),
        );

        let snapshot = WorkspaceSnapshot {
            id: uuid::Uuid::new_v4(),
            workdir: dir.clone(),
            branches: HashMap::new(),
            active_branch: "main".to_string(),
            commits,
            state: StateSnapshot::empty(),
        };

        storage.save_snapshot(&snapshot).unwrap();
        let loaded = storage.load_snapshot().unwrap().unwrap();

        assert_eq!(snapshot.id, loaded.id);
        assert_eq!(snapshot.active_branch, loaded.active_branch);

        std::fs::remove_dir_all(&dir).ok();
    }
}
