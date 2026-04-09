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
// - function: crate::agent::list_agent_info
// - function: crate::agent::get_agent_info
// - function: crate::agent::find_agent_index
// - function: crate::agent::next_agent_index
// - function: crate::agent::previous_agent_index
// - function: crate::agent::normalize_active_index_after_removal
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

pub fn list_agent_info(entries: &[AgentInfo], active_agent_id: Option<AgentId>) -> Vec<AgentInfo> {
    entries
        .iter()
        .cloned()
        .map(|mut entry| {
            if Some(entry.id) == active_agent_id {
                entry.status = AgentStatus::Running;
            }
            entry
        })
        .collect()
}

pub fn get_agent_info(entries: &[AgentInfo], agent_id: AgentId) -> Option<AgentInfo> {
    entries.iter().find(|entry| entry.id == agent_id).cloned()
}

pub fn find_agent_index(entries: &[AgentInfo], agent_id: AgentId) -> Option<usize> {
    entries.iter().position(|entry| entry.id == agent_id)
}

pub fn next_agent_index(len: usize, current: usize) -> Option<usize> {
    if len > 0 {
        Some((current + 1) % len)
    } else {
        None
    }
}

pub fn previous_agent_index(len: usize, current: usize) -> Option<usize> {
    if len > 0 {
        Some(if current == 0 { len - 1 } else { current - 1 })
    } else {
        None
    }
}

pub fn normalize_active_index_after_removal(len: usize, current: usize) -> usize {
    if len == 0 {
        0
    } else if current < len {
        current
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use amadeus_ids::AgentId;
    use amadeus_profiles::AgentProfile;

    use super::{
        find_agent_index, get_agent_info, list_agent_info, next_agent_index,
        normalize_active_index_after_removal, previous_agent_index, select_agent, AgentInfo,
        AgentRouteCandidate, AgentStatus,
    };
    use crate::worker::Task;

    fn candidate(capabilities: &[&str]) -> AgentRouteCandidate {
        AgentRouteCandidate {
            id: AgentId::new(),
            capabilities: capabilities.iter().map(|cap| cap.to_string()).collect(),
        }
    }

    fn info(name: &str, status: AgentStatus) -> AgentInfo {
        AgentInfo {
            id: AgentId::new(),
            name: name.to_string(),
            profile: AgentProfile::Default,
            status,
            task_count: 0,
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

    #[test]
    fn list_agent_info_marks_active_as_running() {
        let first = info("one", AgentStatus::Idle);
        let second = info("two", AgentStatus::Error);

        let listed = list_agent_info(&[first.clone(), second.clone()], Some(first.id));

        assert_eq!(listed[0].status, AgentStatus::Running);
        assert_eq!(listed[1].status, AgentStatus::Error);
    }

    #[test]
    fn get_agent_info_returns_matching_entry() {
        let first = info("one", AgentStatus::Idle);
        let second = info("two", AgentStatus::Error);

        let found = get_agent_info(&[first.clone(), second.clone()], second.id);

        assert_eq!(found.map(|entry| entry.name), Some("two".to_string()));
    }

    #[test]
    fn find_agent_index_returns_matching_position() {
        let first = info("one", AgentStatus::Idle);
        let second = info("two", AgentStatus::Error);

        let found = find_agent_index(&[first, second.clone()], second.id);

        assert_eq!(found, Some(1));
    }

    #[test]
    fn next_and_previous_agent_index_wrap() {
        assert_eq!(next_agent_index(3, 2), Some(0));
        assert_eq!(previous_agent_index(3, 0), Some(2));
        assert_eq!(next_agent_index(0, 0), None);
        assert_eq!(previous_agent_index(0, 0), None);
    }

    #[test]
    fn normalize_active_index_after_removal_resets_out_of_bounds() {
        assert_eq!(normalize_active_index_after_removal(2, 3), 0);
        assert_eq!(normalize_active_index_after_removal(2, 1), 1);
        assert_eq!(normalize_active_index_after_removal(0, 0), 0);
    }
}
