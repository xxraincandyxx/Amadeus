use std::collections::HashMap;
use std::path::PathBuf;

use super::branch::{Branch, MergeConflict, MergeResult, MergeStrategy, ResetMode};
use super::commit::{Commit, CommitBuilder, CommitTrigger};
use super::error::{CoreError, Result};
use super::event::{Event, EventLog};
use super::id::{AgentId, CommitId};
use super::state::VersionedState;
use crate::storage::{Storage, WorkspaceSnapshot};

pub struct Workspace {
    pub id: uuid::Uuid,
    pub workdir: PathBuf,
    branches: HashMap<String, Branch>,
    active_branch: String,
    event_log: EventLog,
    state: VersionedState,
    commits: HashMap<CommitId, Commit>,
}

impl Workspace {
    pub async fn create(path: PathBuf) -> Result<Self> {
        if !path.exists() {
            std::fs::create_dir_all(&path).map_err(CoreError::Io)?;
        }

        let initial_commit = CommitBuilder::new()
            .state(VersionedState::new().snapshot())
            .message("Initial commit")
            .author(AgentId::system())
            .trigger(CommitTrigger::UserRequest)
            .build()
            .map_err(CoreError::CommitBuild)?;

        let commit_id = initial_commit.id;
        let mut commits = HashMap::new();
        commits.insert(commit_id, initial_commit);

        let main_branch = Branch::new("main".to_string(), None, commit_id);

        Ok(Self {
            id: uuid::Uuid::new_v4(),
            workdir: path,
            branches: HashMap::from([("main".to_string(), main_branch)]),
            active_branch: "main".to_string(),
            event_log: EventLog::new(),
            state: VersionedState::new(),
            commits,
        })
    }

    pub fn branch(&self) -> &str {
        &self.active_branch
    }

    pub fn branches(&self) -> impl Iterator<Item = &String> {
        self.branches.keys()
    }

    pub fn create_branch(&mut self, name: &str, from: Option<&str>) -> Result<()> {
        if self.branches.contains_key(name) {
            return Err(CoreError::BranchExists(name.to_string()));
        }

        let from_branch_name = from.unwrap_or(&self.active_branch);
        let (from_head, from_commit) = {
            let from_branch = self
                .branches
                .get(from_branch_name)
                .ok_or_else(|| CoreError::BranchNotFound(from_branch_name.to_string()))?;
            (from_branch.head, from_branch.head)
        };

        let new_branch = Branch::new(
            name.to_string(),
            Some((from_branch_name.to_string(), from_head)),
            from_head,
        );

        self.branches.insert(name.to_string(), new_branch);
        self.event_log.append(Event::BranchCreated {
            name: name.to_string(),
            from: from_branch_name.to_string(),
            from_commit: from_commit,
        });

        Ok(())
    }

    pub fn checkout(&mut self, name: &str) -> Result<()> {
        let branch = self
            .branches
            .get(name)
            .ok_or_else(|| CoreError::BranchNotFound(name.to_string()))?;

        if let Some(commit) = self.commits.get(&branch.head) {
            self.state = VersionedState::from_snapshot(commit.state.clone());
        }

        self.active_branch = name.to_string();
        Ok(())
    }

    pub fn delete_branch(&mut self, name: &str) -> Result<()> {
        if name == "main" {
            return Err(CoreError::CannotDeleteMain);
        }

        if name == self.active_branch {
            return Err(CoreError::CannotDeleteCurrentBranch);
        }

        if self.branches.remove(name).is_none() {
            return Err(CoreError::BranchNotFound(name.to_string()));
        }

        self.event_log.append(Event::BranchDeleted {
            name: name.to_string(),
        });

        Ok(())
    }

    pub fn commit(&mut self, message: &str, author: AgentId) -> Result<CommitId> {
        let parent = self.branches.get(&self.active_branch).map(|b| b.head);

        let commit = CommitBuilder::new()
            .parent(parent)
            .state(self.state.snapshot())
            .message(message)
            .author(author)
            .trigger(CommitTrigger::UserRequest)
            .build()
            .map_err(CoreError::CommitBuild)?;

        let commit_id = commit.id;

        self.commits.insert(commit_id, commit);

        if let Some(branch) = self.branches.get_mut(&self.active_branch) {
            branch.add_commit(commit_id);
        }

        self.event_log.append(Event::CommitCreated {
            id: commit_id,
            message: message.to_string(),
            author,
        });

        Ok(commit_id)
    }

    pub fn reset(&mut self, to: CommitId, mode: ResetMode) -> Result<()> {
        let commit = self
            .commits
            .get(&to)
            .ok_or_else(|| CoreError::CommitNotFound(to))?
            .clone();

        match mode {
            ResetMode::Hard => {
                self.state = VersionedState::from_snapshot(commit.state);
                if let Some(branch) = self.branches.get_mut(&self.active_branch) {
                    branch.set_head(to);
                }
            }
            ResetMode::Mixed => {
                self.state = VersionedState::from_snapshot(commit.state);
            }
            ResetMode::Soft => {}
        }

        self.event_log.append(Event::ResetCompleted { to, mode });
        Ok(())
    }

    pub fn merge(
        &mut self,
        from: &str,
        into: &str,
        strategy: MergeStrategy,
    ) -> Result<MergeResult> {
        let from_branch = self
            .branches
            .get(from)
            .ok_or_else(|| CoreError::BranchNotFound(from.to_string()))?
            .clone();

        let into_branch = self
            .branches
            .get(into)
            .ok_or_else(|| CoreError::BranchNotFound(into.to_string()))?;

        let from_commit = self
            .commits
            .get(&from_branch.head)
            .ok_or_else(|| CoreError::CommitNotFound(from_branch.head))?;

        let into_commit = self
            .commits
            .get(&into_branch.head)
            .ok_or_else(|| CoreError::CommitNotFound(into_branch.head))?;

        let result = match strategy {
            MergeStrategy::Theirs => {
                self.state = VersionedState::from_snapshot(from_commit.state.clone());
                let merge_commit = self.create_merge_commit(
                    vec![into_branch.head, from_branch.head],
                    format!("Merge '{}' into '{}'", from, into),
                )?;
                MergeResult::success(merge_commit)
            }
            MergeStrategy::Ours => {
                let merge_commit = self.create_merge_commit(
                    vec![into_branch.head, from_branch.head],
                    format!("Merge '{}' into '{}'", from, into),
                )?;
                MergeResult::success(merge_commit)
            }
            MergeStrategy::Auto => {
                let conflicts = self.find_conflicts(&from_commit.state, &into_commit.state);
                if conflicts.is_empty() {
                    self.state = VersionedState::from_snapshot(from_commit.state.clone());
                    let merge_commit = self.create_merge_commit(
                        vec![into_branch.head, from_branch.head],
                        format!("Merge '{}' into '{}'", from, into),
                    )?;
                    MergeResult::success(merge_commit)
                } else {
                    MergeResult::conflict(conflicts)
                }
            }
            MergeStrategy::Manual => {
                let conflicts = self.find_conflicts(&from_commit.state, &into_commit.state);
                if conflicts.is_empty() {
                    MergeResult::success(into_branch.head)
                } else {
                    MergeResult::conflict(conflicts)
                }
            }
        };

        if result.success {
            self.event_log.append(Event::MergeCompleted {
                from: from.to_string(),
                into: into.to_string(),
                result: if result.has_conflicts() {
                    super::event::MergeResult::Conflict {
                        conflicts: result.conflicts.len() as u32,
                    }
                } else {
                    super::event::MergeResult::Success
                },
            });
        }

        Ok(result)
    }

    fn create_merge_commit(
        &mut self,
        parents: Vec<CommitId>,
        message: String,
    ) -> Result<CommitId> {
        let commit = Commit {
            id: CommitId::new(),
            parent: parents.first().copied(),
            state: self.state.snapshot(),
            message,
            author: AgentId::system(),
            timestamp: chrono::Utc::now(),
            trigger: CommitTrigger::UserRequest,
        };

        let commit_id = commit.id;
        self.commits.insert(commit_id, commit);

        if let Some(branch) = self.branches.get_mut(&self.active_branch) {
            branch.add_commit(commit_id);
        }

        Ok(commit_id)
    }

    fn find_conflicts(
        &self,
        from_state: &super::state::StateSnapshot,
        into_state: &super::state::StateSnapshot,
    ) -> Vec<MergeConflict> {
        let mut conflicts = Vec::new();

        for (key, from_value) in &from_state.data {
            if let Some(into_value) = into_state.data.get(key) {
                if from_value != into_value {
                    conflicts.push(MergeConflict {
                        key: key.clone(),
                        ours: into_value.clone(),
                        theirs: from_value.clone(),
                    });
                }
            }
        }

        conflicts
    }

    pub fn state(&self) -> &VersionedState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut VersionedState {
        &mut self.state
    }

    pub fn head(&self) -> Option<CommitId> {
        self.branches.get(&self.active_branch).map(|b| b.head)
    }

    pub fn get_commit(&self, id: CommitId) -> Option<&Commit> {
        self.commits.get(&id)
    }

    pub fn log(&self, limit: usize) -> Vec<&Commit> {
        let branch = self.branches.get(&self.active_branch);
        let mut commits = Vec::new();

        if let Some(branch) = branch {
            let mut current_id = Some(branch.head);
            while let Some(id) = current_id {
                if commits.len() >= limit {
                    break;
                }
                if let Some(commit) = self.commits.get(&id) {
                    current_id = commit.parent;
                    commits.push(commit);
                } else {
                    break;
                }
            }
        }

        commits
    }

    pub fn event_log(&self) -> &EventLog {
        &self.event_log
    }

    pub fn workdir(&self) -> &PathBuf {
        &self.workdir
    }

    pub fn storage(&self) -> Storage {
        Storage::new(&self.workdir)
    }

    pub fn save(&self) -> Result<()> {
        let storage = self.storage();
        storage.init()?;

        let commits_str: HashMap<String, Commit> = self
            .commits
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();

        let snapshot = WorkspaceSnapshot {
            id: self.id,
            workdir: self.workdir.clone(),
            branches: self.branches.clone(),
            active_branch: self.active_branch.clone(),
            commits: commits_str,
            state: self.state.snapshot(),
        };

        storage.save_snapshot(&snapshot)?;

        for entry in self.event_log.all() {
            storage.append_event(&entry.event)?;
        }

        Ok(())
    }

    pub fn load(path: PathBuf) -> Result<Self> {
        let storage = Storage::new(&path);
        let snapshot = storage
            .load_snapshot()?
            .ok_or(CoreError::WorkspaceNotInitialized)?;

        let commits: HashMap<CommitId, Commit> = snapshot
            .commits
            .into_iter()
            .filter_map(|(k, v)| k.parse().ok().map(|id| (id, v)))
            .collect();

        let mut event_log = EventLog::new();
        for event in storage.load_events()? {
            event_log.append(event);
        }

        Ok(Self {
            id: snapshot.id,
            workdir: snapshot.workdir,
            branches: snapshot.branches,
            active_branch: snapshot.active_branch,
            event_log,
            state: VersionedState::from_snapshot(snapshot.state),
            commits,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workspace_create() {
        let ws = Workspace::create(std::env::temp_dir().join("test-ws-create"))
            .await
            .unwrap();

        assert_eq!(ws.branch(), "main");
        assert!(ws.head().is_some());
    }

    #[tokio::test]
    async fn test_workspace_branch() {
        let mut ws = Workspace::create(std::env::temp_dir().join("test-ws-branch"))
            .await
            .unwrap();

        ws.create_branch("feature", None).unwrap();
        assert!(ws.branches().any(|b| b == "feature"));

        ws.checkout("feature").unwrap();
        assert_eq!(ws.branch(), "feature");
    }

    #[tokio::test]
    async fn test_workspace_commit() {
        let mut ws = Workspace::create(std::env::temp_dir().join("test-ws-commit"))
            .await
            .unwrap();

        ws.state_mut()
            .write("test_key", serde_json::json!("test_value"));

        let commit_id = ws.commit("Test commit", AgentId::system()).unwrap();

        assert!(ws.get_commit(commit_id).is_some());
        assert_eq!(ws.log(10).len(), 2);
    }

    #[tokio::test]
    async fn test_workspace_reset() {
        let mut ws = Workspace::create(std::env::temp_dir().join("test-ws-reset"))
            .await
            .unwrap();

        let initial_commit = ws.head().unwrap();

        ws.state_mut().write("key", serde_json::json!("value"));
        ws.commit("Add key", AgentId::system()).unwrap();

        assert!(ws.state().read("key").is_some());

        ws.reset(initial_commit, ResetMode::Hard).unwrap();

        assert!(ws.state().read("key").is_none());
    }

    #[tokio::test]
    async fn test_workspace_merge() {
        let mut ws = Workspace::create(std::env::temp_dir().join("test-ws-merge"))
            .await
            .unwrap();

        ws.create_branch("feature", None).unwrap();
        ws.checkout("feature").unwrap();

        ws.state_mut()
            .write("feature_key", serde_json::json!("feature_value"));
        ws.commit("Add feature key", AgentId::system()).unwrap();

        ws.checkout("main").unwrap();

        let result = ws.merge("feature", "main", MergeStrategy::Auto).unwrap();

        assert!(result.success);
    }
}
