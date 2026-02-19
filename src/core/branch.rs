use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::id::CommitId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub parent: Option<(String, CommitId)>,
    pub commits: Vec<CommitId>,
    pub head: CommitId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Branch {
    pub fn new(
        name: String,
        parent_branch: Option<(String, CommitId)>,
        initial_commit: CommitId,
    ) -> Self {
        let now = Utc::now();
        Self {
            name,
            parent: parent_branch,
            commits: vec![initial_commit],
            head: initial_commit,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn add_commit(&mut self, commit_id: CommitId) {
        self.commits.push(commit_id);
        self.head = commit_id;
        self.updated_at = Utc::now();
    }

    pub fn set_head(&mut self, commit_id: CommitId) {
        self.head = commit_id;
        self.updated_at = Utc::now();
    }

    pub fn commit_count(&self) -> usize {
        self.commits.len()
    }

    pub fn is_ancestor(&self, commit_id: &CommitId) -> bool {
        self.commits.contains(commit_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeStrategy {
    Auto,
    Ours,
    Theirs,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

#[derive(Debug, Clone)]
pub struct BranchDiff {
    pub ahead: usize,
    pub behind: usize,
    pub diverged: bool,
}

impl BranchDiff {
    pub fn is_synced(&self) -> bool {
        self.ahead == 0 && self.behind == 0 && !self.diverged
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConflict {
    pub key: String,
    pub ours: serde_json::Value,
    pub theirs: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct MergeResult {
    pub success: bool,
    pub conflicts: Vec<MergeConflict>,
    pub commit_id: Option<CommitId>,
}

impl MergeResult {
    pub fn success(commit_id: CommitId) -> Self {
        Self {
            success: true,
            conflicts: Vec::new(),
            commit_id: Some(commit_id),
        }
    }

    pub fn conflict(conflicts: Vec<MergeConflict>) -> Self {
        Self {
            success: false,
            conflicts,
            commit_id: None,
        }
    }

    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branch_creation() {
        let commit = CommitId::new();
        let branch = Branch::new("feature".to_string(), None, commit);

        assert_eq!(branch.name, "feature");
        assert!(branch.parent.is_none());
        assert_eq!(branch.commit_count(), 1);
    }

    #[test]
    fn test_branch_with_parent() {
        let initial = CommitId::new();
        let branch = Branch::new(
            "feature".to_string(),
            Some(("main".to_string(), initial)),
            initial,
        );

        assert_eq!(branch.parent, Some(("main".to_string(), initial)));
    }

    #[test]
    fn test_branch_add_commit() {
        let initial = CommitId::new();
        let mut branch = Branch::new("main".to_string(), None, initial);

        let new_commit = CommitId::new();
        branch.add_commit(new_commit);

        assert_eq!(branch.head, new_commit);
        assert_eq!(branch.commit_count(), 2);
    }

    #[test]
    fn test_branch_is_ancestor() {
        let initial = CommitId::new();
        let mut branch = Branch::new("main".to_string(), None, initial);

        let new_commit = CommitId::new();
        branch.add_commit(new_commit);

        assert!(branch.is_ancestor(&initial));
        assert!(branch.is_ancestor(&new_commit));

        let unknown = CommitId::new();
        assert!(!branch.is_ancestor(&unknown));
    }

    #[test]
    fn test_merge_result() {
        let commit = CommitId::new();
        let result = MergeResult::success(commit);

        assert!(result.success);
        assert!(!result.has_conflicts());
        assert_eq!(result.commit_id, Some(commit));
    }

    #[test]
    fn test_merge_conflict() {
        let conflicts = vec![MergeConflict {
            key: "foo".to_string(),
            ours: serde_json::json!(1),
            theirs: serde_json::json!(2),
        }];
        let result = MergeResult::conflict(conflicts);

        assert!(!result.success);
        assert!(result.has_conflicts());
    }
}
