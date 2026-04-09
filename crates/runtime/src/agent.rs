// @amadeus-header
// summary: Shared agent manager state models reused across API and UI surfaces.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::agent
// - type: crate::agent::AgentStatus
// - type: crate::agent::AgentInfo
// - type: crate::agent::AgentRouteCandidate
// - function: crate::agent::select_agent
// uses:
// - module: amadeus_ids
// - module: amadeus_profiles
// - module: crate::worker
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

use crate::worker::Task;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRouteCandidate {
    pub id: AgentId,
    pub capabilities: Vec<String>,
}

pub fn select_agent(
    candidates: &[AgentRouteCandidate],
    active_agent_id: Option<AgentId>,
    allowed_ids: Option<&[AgentId]>,
    task: &Task,
) -> Option<AgentId> {
    let matches = |candidate: &AgentRouteCandidate| {
        allowed_ids
            .map(|ids| ids.contains(&candidate.id))
            .unwrap_or(true)
            && task
                .required_capabilities
                .iter()
                .all(|capability| candidate.capabilities.contains(capability))
    };

    let preferred = active_agent_id.and_then(|active_id| {
        candidates
            .iter()
            .find(|candidate| candidate.id == active_id && matches(candidate))
            .map(|candidate| candidate.id)
    });

    preferred
        .or_else(|| {
            candidates
                .iter()
                .find(|candidate| matches(candidate))
                .map(|candidate| candidate.id)
        })
        .or_else(|| {
            if task.required_capabilities.is_empty() {
                allowed_ids
                    .and_then(|ids| ids.first().copied())
                    .or_else(|| candidates.first().map(|candidate| candidate.id))
            } else {
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use amadeus_ids::AgentId;

    use super::{select_agent, AgentRouteCandidate};
    use crate::worker::Task;

    fn candidate(capabilities: &[&str]) -> AgentRouteCandidate {
        AgentRouteCandidate {
            id: AgentId::new(),
            capabilities: capabilities.iter().map(|cap| cap.to_string()).collect(),
        }
    }

    #[test]
    fn select_agent_prefers_active_match() {
        let first = candidate(&["rust"]);
        let second = candidate(&["rust", "sql"]);
        let task = Task::new("task-1", "prompt").requires(vec!["rust".to_string()]);

        let selected = select_agent(
            &[first.clone(), second.clone()],
            Some(second.id),
            None,
            &task,
        );

        assert_eq!(selected, Some(second.id));
    }

    #[test]
    fn select_agent_respects_team_membership() {
        let first = candidate(&["rust"]);
        let second = candidate(&["rust"]);
        let allowed = vec![second.id];
        let task = Task::new("task-1", "prompt").requires(vec!["rust".to_string()]);

        let selected = select_agent(&[first, second.clone()], None, Some(&allowed), &task);

        assert_eq!(selected, Some(second.id));
    }

    #[test]
    fn select_agent_falls_back_for_capability_free_tasks() {
        let first = candidate(&[]);
        let second = candidate(&["rust"]);
        let task = Task::new("task-1", "prompt");

        let selected = select_agent(&[first.clone(), second], Some(first.id), None, &task);

        assert_eq!(selected, Some(first.id));
    }

    #[test]
    fn select_agent_returns_none_when_capabilities_do_not_match() {
        let first = candidate(&["rust"]);
        let task = Task::new("task-1", "prompt").requires(vec!["python".to_string()]);

        let selected = select_agent(&[first], None, None, &task);

        assert_eq!(selected, None);
    }
}
