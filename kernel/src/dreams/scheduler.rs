//! Dream scheduler — manages the priority queue and budget enforcement.

use super::types::{DreamResult, DreamTask};
use serde::{Deserialize, Serialize};

/// Configuration and runtime state for the dream scheduling system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamScheduler {
    pub enabled: bool,
    /// Enter dream state after N minutes idle (default: 15).
    pub idle_trigger_minutes: u32,
    /// Max tokens to spend per dream cycle.
    pub dream_budget_tokens: u64,
    /// Max API calls per dream cycle.
    pub dream_budget_api_calls: u32,
    /// Tasks waiting to be dreamed.
    pub priority_queue: Vec<DreamTask>,
    /// Completed dream results (rolling history).
    pub completed_dreams: Vec<DreamResult>,
    /// Timestamp of last dream cycle start.
    pub last_dream_at: Option<u64>,
    /// Timestamp of last user activity (for idle detection).
    pub last_activity_at: u64,
}

/// Max completed dreams to keep in history.
const MAX_DREAM_HISTORY: usize = 200;

impl Default for DreamScheduler {
    fn default() -> Self {
        Self {
            enabled: true,
            idle_trigger_minutes: 15,
            dream_budget_tokens: 50_000,
            dream_budget_api_calls: 20,
            priority_queue: Vec::new(),
            completed_dreams: Vec::new(),
            last_dream_at: None,
            last_activity_at: crate::consciousness::state::now_secs(),
        }
    }
}

impl DreamScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record user activity to reset idle timer.
    pub fn touch_activity(&mut self) {
        self.last_activity_at = crate::consciousness::state::now_secs();
    }

    /// Check if idle long enough to trigger dream state.
    pub fn should_enter_dream(&self) -> bool {
        if !self.enabled || self.priority_queue.is_empty() {
            return false;
        }
        let now = crate::consciousness::state::now_secs();
        let idle_secs = now.saturating_sub(self.last_activity_at);
        idle_secs >= (self.idle_trigger_minutes as u64) * 60
    }

    /// Enqueue a dream task (deduplicates by task id).
    pub fn enqueue(&mut self, task: DreamTask) {
        if !self.priority_queue.iter().any(|t| t.id == task.id) {
            self.priority_queue.push(task);
        }
    }

    /// Sort queue by priority (highest first).
    pub fn sort_queue(&mut self) {
        self.priority_queue.sort_by(|a, b| {
            b.priority
                .partial_cmp(&a.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Record a completed dream result and remove from queue.
    pub fn record_result(&mut self, result: DreamResult) {
        self.priority_queue.retain(|t| t.id != result.task_id);
        self.completed_dreams.push(result);
        if self.completed_dreams.len() > MAX_DREAM_HISTORY {
            self.completed_dreams.remove(0);
        }
    }

    /// Get the N most recent completed dreams.
    pub fn recent_dreams(&self, limit: u32) -> Vec<DreamResult> {
        let history = &self.completed_dreams;
        let start = if (limit as usize) < history.len() {
            history.len() - limit as usize
        } else {
            0
        };
        history[start..].to_vec()
    }

    /// Update scheduler configuration.
    pub fn configure(
        &mut self,
        enabled: bool,
        idle_trigger_minutes: u32,
        budget_tokens: u64,
        budget_calls: u32,
    ) {
        self.enabled = enabled;
        self.idle_trigger_minutes = idle_trigger_minutes;
        self.dream_budget_tokens = budget_tokens;
        self.dream_budget_api_calls = budget_calls;
    }

    /// Get queue size.
    pub fn queue_len(&self) -> usize {
        self.priority_queue.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dreams::types::DreamType;

    fn make_task(id: &str, priority: f64, dtype: DreamType) -> DreamTask {
        DreamTask {
            id: id.to_string(),
            task_type: dtype,
            priority,
            source_agent: "test-agent".into(),
            ..DreamTask::default()
        }
    }

    #[test]
    fn default_scheduler() {
        let s = DreamScheduler::new();
        assert!(s.enabled);
        assert_eq!(s.idle_trigger_minutes, 15);
        assert_eq!(s.dream_budget_tokens, 50_000);
        assert!(s.priority_queue.is_empty());
    }

    #[test]
    fn enqueue_deduplicates() {
        let mut s = DreamScheduler::new();
        let t = make_task("t1", 0.5, DreamType::Replay);
        s.enqueue(t.clone());
        s.enqueue(t);
        assert_eq!(s.queue_len(), 1);
    }

    #[test]
    fn sort_by_priority() {
        let mut s = DreamScheduler::new();
        s.enqueue(make_task("low", 0.2, DreamType::Explore));
        s.enqueue(make_task("high", 0.9, DreamType::Experiment));
        s.enqueue(make_task("mid", 0.5, DreamType::Replay));
        s.sort_queue();
        assert_eq!(s.priority_queue[0].id, "high");
        assert_eq!(s.priority_queue[1].id, "mid");
        assert_eq!(s.priority_queue[2].id, "low");
    }

    #[test]
    fn record_result_removes_from_queue() {
        let mut s = DreamScheduler::new();
        s.enqueue(make_task("t1", 0.5, DreamType::Replay));
        s.enqueue(make_task("t2", 0.8, DreamType::Experiment));

        let result = crate::dreams::types::DreamResult {
            task_id: "t1".into(),
            dream_type: DreamType::Replay,
            agent_id: "a".into(),
            started_at: 0,
            completed_at: 1,
            tokens_used: 100,
            outcome: crate::dreams::types::DreamOutcome::NoResult {
                reason: "test".into(),
            },
        };
        s.record_result(result);
        assert_eq!(s.queue_len(), 1);
        assert_eq!(s.priority_queue[0].id, "t2");
        assert_eq!(s.completed_dreams.len(), 1);
    }

    #[test]
    fn should_not_dream_when_disabled() {
        let mut s = DreamScheduler::new();
        s.enabled = false;
        s.enqueue(make_task("t1", 0.5, DreamType::Replay));
        s.last_activity_at = 0; // very old
        assert!(!s.should_enter_dream());
    }

    #[test]
    fn should_not_dream_with_empty_queue() {
        let mut s = DreamScheduler::new();
        s.last_activity_at = 0;
        assert!(!s.should_enter_dream());
    }

    #[test]
    fn configure_updates_settings() {
        let mut s = DreamScheduler::new();
        s.configure(false, 30, 100_000, 50);
        assert!(!s.enabled);
        assert_eq!(s.idle_trigger_minutes, 30);
        assert_eq!(s.dream_budget_tokens, 100_000);
        assert_eq!(s.dream_budget_api_calls, 50);
    }

    #[test]
    fn recent_dreams_respects_limit() {
        let mut s = DreamScheduler::new();
        for i in 0..10 {
            s.completed_dreams.push(crate::dreams::types::DreamResult {
                task_id: format!("t{i}"),
                dream_type: DreamType::Replay,
                agent_id: "a".into(),
                started_at: 0,
                completed_at: 1,
                tokens_used: 100,
                outcome: crate::dreams::types::DreamOutcome::NoResult {
                    reason: "test".into(),
                },
            });
        }
        let recent = s.recent_dreams(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].task_id, "t7");
    }
}
