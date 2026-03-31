//! # Adaptive Scheduler
//!
//! Learns WHEN to attempt improvements based on success/failure history.
//! Shortens intervals on success, backs off exponentially on failure.

use crate::trajectory::AttemptOutcome;
use serde::{Deserialize, Serialize};

/// Adaptive improvement scheduler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveScheduler {
    /// Base interval in seconds.
    pub base_interval_secs: u64,
    /// Current interval in seconds (adapts over time).
    pub current_interval_secs: u64,
    /// Rolling success rate (0.0–1.0), EMA with alpha=0.1.
    pub success_rate: f64,
    /// Consecutive failures since last success.
    pub consecutive_failures: u32,
    /// Backoff multiplier on failure.
    pub backoff_factor: f64,
    /// Minimum interval in seconds.
    pub min_interval_secs: u64,
    /// Maximum interval in seconds.
    pub max_interval_secs: u64,
    /// Timestamp of the last cycle (Unix seconds).
    pub last_cycle_at: u64,
    /// Total cycles executed.
    pub total_cycles: u64,
}

impl AdaptiveScheduler {
    pub fn new() -> Self {
        Self {
            base_interval_secs: 3600, // 1 hour
            current_interval_secs: 3600,
            success_rate: 0.0,
            consecutive_failures: 0,
            backoff_factor: 2.0,
            min_interval_secs: 900,   // 15 minutes
            max_interval_secs: 86400, // 24 hours
            last_cycle_at: 0,
            total_cycles: 0,
        }
    }

    /// Calculate the next cycle time (Unix seconds).
    pub fn next_cycle_time(&self) -> u64 {
        self.last_cycle_at + self.current_interval_secs
    }

    /// Whether a cycle is due now.
    pub fn is_due(&self, now: u64) -> bool {
        now >= self.next_cycle_time()
    }

    /// Record a cycle outcome and adjust the schedule.
    pub fn record_outcome(&mut self, outcome: &AttemptOutcome) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.last_cycle_at = now;
        self.total_cycles += 1;

        match outcome {
            AttemptOutcome::Improved { .. } => {
                self.consecutive_failures = 0;
                self.success_rate = self.success_rate * 0.9 + 0.1;
                // Halve interval on success (down to min)
                self.current_interval_secs =
                    (self.current_interval_secs / 2).max(self.min_interval_secs);
            }
            AttemptOutcome::NoImprovement
            | AttemptOutcome::RolledBack { .. }
            | AttemptOutcome::Rejected { .. } => {
                self.consecutive_failures += 1;
                self.success_rate *= 0.9;

                // Exponential backoff: base * factor^failures, capped at max
                let backoff = self.backoff_factor.powi(self.consecutive_failures as i32);
                let new_interval = (self.base_interval_secs as f64 * backoff) as u64;
                self.current_interval_secs = new_interval
                    .max(self.min_interval_secs)
                    .min(self.max_interval_secs);
            }
        }
    }

    /// Get human-readable status.
    pub fn status_summary(&self) -> String {
        let interval_mins = self.current_interval_secs / 60;
        format!(
            "interval={}m success_rate={:.1}% consecutive_failures={} total_cycles={}",
            interval_mins,
            self.success_rate * 100.0,
            self.consecutive_failures,
            self.total_cycles,
        )
    }
}

impl Default for AdaptiveScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_shortens_on_success() {
        let mut sched = AdaptiveScheduler::new();
        let before = sched.current_interval_secs;
        sched.record_outcome(&AttemptOutcome::Improved { delta: 0.1 });
        assert!(
            sched.current_interval_secs < before,
            "interval should shorten after success"
        );
        assert_eq!(sched.consecutive_failures, 0);
    }

    #[test]
    fn test_scheduler_backoff_on_failure() {
        let mut sched = AdaptiveScheduler::new();
        let before = sched.current_interval_secs;
        sched.record_outcome(&AttemptOutcome::NoImprovement);
        assert!(
            sched.current_interval_secs >= before,
            "interval should increase or stay after failure"
        );
        assert_eq!(sched.consecutive_failures, 1);
    }

    #[test]
    fn test_scheduler_respects_min_interval() {
        let mut sched = AdaptiveScheduler::new();
        sched.min_interval_secs = 600; // 10 minutes
                                       // Many successes should not go below min
        for _ in 0..20 {
            sched.record_outcome(&AttemptOutcome::Improved { delta: 0.1 });
        }
        assert!(
            sched.current_interval_secs >= sched.min_interval_secs,
            "should respect min: {} < {}",
            sched.current_interval_secs,
            sched.min_interval_secs,
        );
    }

    #[test]
    fn test_scheduler_respects_max_interval() {
        let mut sched = AdaptiveScheduler::new();
        sched.max_interval_secs = 7200; // 2 hours
                                        // Many failures should not exceed max
        for _ in 0..20 {
            sched.record_outcome(&AttemptOutcome::NoImprovement);
        }
        assert!(
            sched.current_interval_secs <= sched.max_interval_secs,
            "should respect max: {} > {}",
            sched.current_interval_secs,
            sched.max_interval_secs,
        );
    }

    #[test]
    fn test_scheduler_exponential_backoff() {
        let mut sched = AdaptiveScheduler::new();
        sched.base_interval_secs = 1000;
        sched.backoff_factor = 2.0;
        sched.max_interval_secs = 100_000;

        sched.record_outcome(&AttemptOutcome::NoImprovement); // 1 failure → 2000
        let after_1 = sched.current_interval_secs;
        sched.record_outcome(&AttemptOutcome::NoImprovement); // 2 failures → 4000
        let after_2 = sched.current_interval_secs;

        assert!(
            after_2 > after_1,
            "exponential backoff: {after_2} should > {after_1}"
        );
    }
}
