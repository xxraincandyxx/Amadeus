//! # Concurrency Module
//!
//! Resource-level lock management for coordinating concurrent agent operations.
//!
//! ## Features
//!
//! - Shared/exclusive lock modes
//! - Wait queue with timeout
//! - Lock status inspection
//!
//! ## Example
//!
//! ```rust,ignore
//! use amadeus::concurrency::{LockManager, LockMode};
//!
//! let manager = LockManager::new();
//!
//! // Try acquire without waiting
//! manager.try_acquire("/path/to/file", agent_id, LockMode::Exclusive)?;
//!
//! // Acquire with timeout
//! manager.acquire("/path/to/file", agent_id, LockMode::Shared, Duration::from_secs(30)).await?;
//!
//! // Release
//! manager.release("/path/to/file", agent_id)?;
//! ```

mod lock;
mod file_lock;

pub use lock::{LockEntry, LockError, LockManager, LockMode, LockStatus};
pub use file_lock::{FileLockManager, FileReadGuard, FileWriteGuard, FileReadInfo, FileLockStats};
