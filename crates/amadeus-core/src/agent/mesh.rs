// @amadeus-header
// summary: Agent subsystem code for mesh.
// layer: agent
// status: active
// feature_flags: none
// provides:
// - module: crate::agent::mesh
// - type: crate::agent::mesh::MeshInfo
// - type: crate::agent::mesh::MeshManager
// uses:
// - protocol: serde serialization
// - runtime: tracing instrumentation
// - artifact: filesystem paths and files
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Reads or writes filesystem state.
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

//! # Mesh Management
//!
//! Coordination logic for multiple Amadeus instances in the same workspace.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::warn;

#[derive(Debug, Serialize, Deserialize)]
pub struct MeshInfo {
    pub supervisor_addr: String,
    pub supervisor_pid: u32,
    pub last_seen: u64,
    pub workdir: PathBuf,
}

pub struct MeshManager {
    lock_path: PathBuf,
}

impl MeshManager {
    pub fn new(workdir: PathBuf) -> Self {
        Self {
            lock_path: workdir.join(".amadeus_mesh"),
        }
    }

    pub fn get_supervisor_info(&self) -> Option<MeshInfo> {
        if !self.lock_path.exists() {
            return None;
        }

        let data = fs::read_to_string(&self.lock_path).ok()?;
        let info: MeshInfo = serde_json::from_str(&data).ok()?;

        // Check if process is still alive (simple check for same OS)
        if !self.is_pid_alive(info.supervisor_pid) {
            warn!(
                "Stale supervisor lock found (PID {}), cleaning up",
                info.supervisor_pid
            );
            let _ = fs::remove_file(&self.lock_path);
            return None;
        }

        Some(info)
    }

    pub fn register_supervisor(&self, addr: &str) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let info = MeshInfo {
            supervisor_addr: addr.to_string(),
            supervisor_pid: std::process::id(),
            last_seen: now,
            workdir: self.lock_path.parent().unwrap().to_path_buf(),
        };

        match serde_json::to_string(&info) {
            Ok(data) => fs::write(&self.lock_path, data).is_ok(),
            Err(_) => false,
        }
    }

    pub fn cleanup(&self) {
        if let Some(info) = self.get_supervisor_info() {
            if info.supervisor_pid == std::process::id() {
                let _ = fs::remove_file(&self.lock_path);
            }
        }
    }

    fn is_pid_alive(&self, pid: u32) -> bool {
        #[cfg(unix)]
        {
            let res = unsafe { libc::kill(pid as libc::pid_t, 0) };
            res == 0
        }
        #[cfg(not(unix))]
        {
            // Fallback for non-unix or simple check
            true
        }
    }
}
