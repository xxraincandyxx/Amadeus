// @amadeus-header
// summary: Transport-agnostic supervisor scheduling policy and worker selection logic.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::scheduler
// - type: crate::scheduler::DispatchStrategy
// - type: crate::scheduler::SupervisorConfig
// - function: crate::scheduler::select_worker
// uses:
// - module: crate::worker
// - runtime: std time duration utilities
// invariants:
// - Worker selection semantics stay deterministic for identical worker snapshots and cursor state.
// side_effects: none
// tests:
// - cmd: cargo test -p runtime
// @end-amadeus-header

use std::time::Duration;

use amadeus_ids::AgentId;

use crate::worker::{Task, WorkerInfo};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DispatchStrategy {
    #[default]
    RoundRobin,
    LeastLoaded,
    CapabilityMatch,
}

#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    pub strategy: DispatchStrategy,
    pub max_pending_tasks: usize,
    pub task_timeout: Duration,
    pub retry_failed_tasks: bool,
    pub max_retries: u8,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            strategy: DispatchStrategy::default(),
            max_pending_tasks: 100,
            task_timeout: Duration::from_secs(300),
            retry_failed_tasks: true,
            max_retries: 3,
        }
    }
}

pub fn select_worker(
    workers: &[WorkerInfo],
    task: &Task,
    strategy: DispatchStrategy,
    next_index: &mut usize,
) -> Option<AgentId> {
    let candidates: Vec<&WorkerInfo> = workers.iter().filter(|info| info.is_available()).collect();

    match strategy {
        DispatchStrategy::RoundRobin => {
            if candidates.is_empty() {
                None
            } else {
                let index = *next_index % candidates.len();
                *next_index += 1;
                Some(candidates[index].id)
            }
        }
        DispatchStrategy::LeastLoaded => candidates
            .iter()
            .min_by_key(|info| info.active_tasks)
            .map(|info| info.id),
        DispatchStrategy::CapabilityMatch => candidates
            .iter()
            .filter(|info| info.has_capabilities(&task.required_capabilities))
            .min_by_key(|info| info.active_tasks)
            .map(|info| info.id),
    }
}

#[cfg(test)]
mod tests {
    use amadeus_ids::AgentId;

    use super::{select_worker, DispatchStrategy};
    use crate::worker::{Task, WorkerInfo, WorkerStatus};

    fn worker(active_tasks: usize, max_concurrent: usize, capabilities: &[&str]) -> WorkerInfo {
        WorkerInfo {
            id: AgentId::new(),
            name: "worker".to_string(),
            capabilities: capabilities.iter().map(|cap| cap.to_string()).collect(),
            status: WorkerStatus::Idle,
            active_tasks,
            max_concurrent,
            completed_tasks: 0,
            total_errors: 0,
        }
    }

    #[test]
    fn round_robin_advances_cursor() {
        let workers = vec![
            worker(0, 1, &["rust"]),
            worker(0, 1, &["python"]),
            worker(0, 1, &["go"]),
        ];
        let task = Task::new("task-1", "prompt");
        let mut next_index = 1;

        let selected = select_worker(
            &workers,
            &task,
            DispatchStrategy::RoundRobin,
            &mut next_index,
        );

        assert_eq!(selected, Some(workers[1].id));
        assert_eq!(next_index, 2);
    }

    #[test]
    fn least_loaded_prefers_smallest_active_count() {
        let workers = vec![worker(3, 4, &[]), worker(1, 4, &[]), worker(2, 4, &[])];
        let task = Task::new("task-1", "prompt");

        let selected = select_worker(&workers, &task, DispatchStrategy::LeastLoaded, &mut 0);

        assert_eq!(selected, Some(workers[1].id));
    }

    #[test]
    fn capability_match_filters_before_comparing_load() {
        let workers = vec![
            worker(0, 1, &["rust"]),
            worker(2, 4, &["python", "sql"]),
            worker(1, 4, &["python", "sql", "ops"]),
        ];
        let task =
            Task::new("task-1", "prompt").requires(vec!["python".to_string(), "sql".to_string()]);

        let selected = select_worker(&workers, &task, DispatchStrategy::CapabilityMatch, &mut 0);

        assert_eq!(selected, Some(workers[2].id));
    }

    #[test]
    fn selection_skips_unavailable_workers() {
        let mut busy = worker(1, 1, &["rust"]);
        busy.status = WorkerStatus::Busy;
        let mut offline = worker(0, 1, &["rust"]);
        offline.status = WorkerStatus::Offline;
        let workers = vec![busy, offline];
        let task = Task::new("task-1", "prompt");

        let selected = select_worker(&workers, &task, DispatchStrategy::RoundRobin, &mut 0);

        assert_eq!(selected, None);
    }
}
