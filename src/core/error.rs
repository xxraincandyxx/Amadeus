use std::path::PathBuf;

use thiserror::Error;

use super::id::{AgentId, CommitId, TxId};

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Branch not found: {0}")]
    BranchNotFound(String),

    #[error("Branch already exists: {0}")]
    BranchExists(String),

    #[error("Cannot delete main branch")]
    CannotDeleteMain,

    #[error("Cannot delete current branch")]
    CannotDeleteCurrentBranch,

    #[error("Commit not found: {0}")]
    CommitNotFound(CommitId),

    #[error("Agent not found: {0}")]
    AgentNotFound(AgentId),

    #[error("Commit build error: {0}")]
    CommitBuild(String),

    #[error("Lock acquire failed for resource '{resource}': held by {holder:?}")]
    LockAcquireFailed { resource: String, holder: AgentId },

    #[error("Lock timeout for resource: {0}")]
    LockTimeout(String),

    #[error("Transaction not found: {0}")]
    TransactionNotFound(TxId),

    #[error("Transaction aborted: {0}")]
    TransactionAborted(String),

    #[error("Merge conflict: {0} conflicts")]
    MergeConflict(usize),

    #[error("Invalid workspace path: {0}")]
    InvalidWorkspacePath(PathBuf),

    #[error("Workspace not initialized")]
    WorkspaceNotInitialized,

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Path escapes workspace: {0}")]
    PathEscape(PathBuf),

    #[error("Storage error: {0}")]
    Storage(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
