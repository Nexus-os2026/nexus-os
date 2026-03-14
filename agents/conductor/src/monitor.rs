use crate::types::{AgentAssignment, TaskStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Configuration for the conductor monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    pub global_timeout_secs: u64,
    pub max_retries: u32,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            global_timeout_secs: 300,
            max_retries: 2,
        }
    }
}

/// Decision from the monitor after evaluating assignments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorDecision {
    AllComplete,
    PartialComplete,
    InProgress,
    Retry { ids: Vec<Uuid> },
    PermanentFailure { ids: Vec<Uuid> },
    Timeout,
}

/// Monitors agent assignments and decides next steps.
pub struct Monitor {
    config: MonitorConfig,
    retry_counts: HashMap<Uuid, u32>,
    elapsed_secs: f64,
}

impl Monitor {
    pub fn new(config: MonitorConfig) -> Self {
        Self {
            config,
            retry_counts: HashMap::new(),
            elapsed_secs: 0.0,
        }
    }

    pub fn set_elapsed(&mut self, secs: f64) {
        self.elapsed_secs = secs;
    }

    pub fn evaluate(&mut self, assignments: &HashMap<Uuid, AgentAssignment>) -> MonitorDecision {
        if self.elapsed_secs >= self.config.global_timeout_secs as f64 {
            return MonitorDecision::Timeout;
        }

        if assignments.is_empty() {
            return MonitorDecision::AllComplete;
        }

        let mut completed = 0;
        let mut failed = Vec::new();
        let mut running = 0;
        let mut pending = 0;

        for (id, assignment) in assignments {
            match assignment.status {
                TaskStatus::Completed => completed += 1,
                TaskStatus::Failed => failed.push(*id),
                TaskStatus::Running => running += 1,
                TaskStatus::Pending => pending += 1,
                TaskStatus::Cancelled => completed += 1,
            }
        }

        let total = assignments.len();

        // All done
        if completed == total {
            return MonitorDecision::AllComplete;
        }

        // Check failed tasks
        if !failed.is_empty() {
            let mut retryable = Vec::new();
            let mut permanent = Vec::new();

            for id in &failed {
                let count = self.retry_counts.entry(*id).or_insert(0);
                if *count < self.config.max_retries {
                    *count += 1;
                    retryable.push(*id);
                } else {
                    permanent.push(*id);
                }
            }

            if !permanent.is_empty() {
                return MonitorDecision::PermanentFailure { ids: permanent };
            }

            if !retryable.is_empty() {
                return MonitorDecision::Retry { ids: retryable };
            }
        }

        // Some still running or pending
        if running > 0 || pending > 0 {
            if completed > 0 {
                return MonitorDecision::PartialComplete;
            }
            return MonitorDecision::InProgress;
        }

        // Completed + some cancelled
        if completed > 0 {
            MonitorDecision::PartialComplete
        } else {
            MonitorDecision::AllComplete
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentAssignment, AgentRole, TaskStatus};

    fn make_assignment(status: TaskStatus) -> (Uuid, AgentAssignment) {
        let id = Uuid::new_v4();
        (
            id,
            AgentAssignment {
                subtask_id: id,
                agent_id: Uuid::new_v4(),
                role: AgentRole::Coder,
                status,
                fuel_allocated: 3000,
                fuel_used: 0,
                output_files: vec![],
                error: None,
            },
        )
    }

    #[test]
    fn test_all_complete() {
        let mut monitor = Monitor::new(MonitorConfig::default());
        let mut assignments = HashMap::new();
        let (id, a) = make_assignment(TaskStatus::Completed);
        assignments.insert(id, a);

        assert_eq!(monitor.evaluate(&assignments), MonitorDecision::AllComplete);
    }

    #[test]
    fn test_in_progress() {
        let mut monitor = Monitor::new(MonitorConfig::default());
        let mut assignments = HashMap::new();
        let (id, a) = make_assignment(TaskStatus::Running);
        assignments.insert(id, a);

        assert_eq!(monitor.evaluate(&assignments), MonitorDecision::InProgress);
    }

    #[test]
    fn test_retry_on_failure() {
        let mut monitor = Monitor::new(MonitorConfig::default());
        let mut assignments = HashMap::new();
        let (id, a) = make_assignment(TaskStatus::Failed);
        assignments.insert(id, a);

        match monitor.evaluate(&assignments) {
            MonitorDecision::Retry { ids } => {
                assert!(ids.contains(&id));
            }
            other => panic!("expected Retry, got {other:?}"),
        }
    }

    #[test]
    fn test_permanent_failure_after_max_retries() {
        let config = MonitorConfig {
            max_retries: 2,
            ..Default::default()
        };
        let mut monitor = Monitor::new(config);
        let mut assignments = HashMap::new();
        let (id, a) = make_assignment(TaskStatus::Failed);
        assignments.insert(id, a);

        // First two evaluations should return Retry
        assert!(matches!(
            monitor.evaluate(&assignments),
            MonitorDecision::Retry { .. }
        ));
        assert!(matches!(
            monitor.evaluate(&assignments),
            MonitorDecision::Retry { .. }
        ));
        // Third should be PermanentFailure
        match monitor.evaluate(&assignments) {
            MonitorDecision::PermanentFailure { ids } => {
                assert!(ids.contains(&id));
            }
            other => panic!("expected PermanentFailure, got {other:?}"),
        }
    }

    #[test]
    fn test_timeout() {
        let config = MonitorConfig {
            global_timeout_secs: 300,
            ..Default::default()
        };
        let mut monitor = Monitor::new(config);
        monitor.set_elapsed(301.0);

        let mut assignments = HashMap::new();
        let (id, a) = make_assignment(TaskStatus::Running);
        assignments.insert(id, a);

        assert_eq!(monitor.evaluate(&assignments), MonitorDecision::Timeout);
    }

    #[test]
    fn test_partial_complete() {
        let mut monitor = Monitor::new(MonitorConfig::default());
        let mut assignments = HashMap::new();
        let (id1, a1) = make_assignment(TaskStatus::Completed);
        let (id2, a2) = make_assignment(TaskStatus::Running);
        assignments.insert(id1, a1);
        assignments.insert(id2, a2);

        assert_eq!(
            monitor.evaluate(&assignments),
            MonitorDecision::PartialComplete
        );
    }
}
