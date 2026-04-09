// @amadeus-header
// summary: Shared agent manager state models reused across API and UI surfaces.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::agent
// - type: crate::agent::AgentStatus
// - type: crate::agent::AgentInfo
// uses:
// - module: amadeus_ids
// - module: amadeus_profiles
// - protocol: serde serialization
// invariants:
// - Agent state models stay transport-agnostic and stable across frontends.
// side_effects: none
// tests:
// - cmd: cargo test -p runtime
// @end-amadeus-header

use amadeus_ids::AgentId;
use amadeus_profiles::AgentProfile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Idle,
    Running,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: AgentId,
    pub name: String,
    pub profile: AgentProfile,
    pub status: AgentStatus,
    pub task_count: usize,
}
