//! # Optimization Trajectory
//!
//! Tracks the history of all optimization attempts per agent and domain.
//! Feeds into OPRO-style meta-prompts so future attempts learn from past
//! successes and failures.

use crate::types::ImprovementDomain;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Maximum attempts retained per trajectory.
const MAX_ATTEMPTS: usize = 100;

/// Outcome of an optimization attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttemptOutcome {
    /// Score improved by `delta`.
    Improved { delta: f64 },
    /// No variant exceeded the improvement threshold.
    NoImprovement,
    /// Applied but rolled back during canary period.
    RolledBack { reason: String },
    /// Rejected by validator or HITL.
    Rejected { reason: String },
}

/// A single optimization attempt record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryAttempt {
    pub id: Uuid,
    pub timestamp: u64,
    pub variants_tried: Vec<(String, f64)>,
    pub selected: Option<String>,
    pub outcome: AttemptOutcome,
    pub baseline_score_before: f64,
    pub baseline_score_after: Option<f64>,
}

/// Full optimization history for one agent + domain combination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationTrajectory {
    pub agent_id: String,
    pub domain: ImprovementDomain,
    pub attempts: Vec<TrajectoryAttempt>,
}

impl OptimizationTrajectory {
    pub fn new(agent_id: impl Into<String>, domain: ImprovementDomain) -> Self {
        Self {
            agent_id: agent_id.into(),
            domain,
            attempts: Vec::new(),
        }
    }

    /// Record a new attempt.
    pub fn record(
        &mut self,
        variants_tried: Vec<(String, f64)>,
        selected: Option<String>,
        outcome: AttemptOutcome,
        baseline_score_before: f64,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.attempts.push(TrajectoryAttempt {
            id,
            timestamp: now,
            variants_tried,
            selected,
            outcome,
            baseline_score_before,
            baseline_score_after: None,
        });

        // Trim oldest if over capacity
        if self.attempts.len() > MAX_ATTEMPTS {
            self.attempts.drain(..self.attempts.len() - MAX_ATTEMPTS);
        }

        id
    }

    /// Update the after-score for a completed attempt (post-canary).
    pub fn update_after_score(&mut self, attempt_id: Uuid, score: f64) {
        if let Some(attempt) = self.attempts.iter_mut().find(|a| a.id == attempt_id) {
            attempt.baseline_score_after = Some(score);
        }
    }

    /// Rolling success rate over the last `window` attempts.
    pub fn success_rate(&self, window: usize) -> f64 {
        let recent: Vec<_> = self.attempts.iter().rev().take(window).collect();
        if recent.is_empty() {
            return 0.0;
        }
        let successes = recent
            .iter()
            .filter(|a| matches!(a.outcome, AttemptOutcome::Improved { .. }))
            .count();
        successes as f64 / recent.len() as f64
    }

    /// Build a summary string suitable for inclusion in OPRO meta-prompts.
    pub fn meta_prompt_summary(&self, max_entries: usize) -> String {
        if self.attempts.is_empty() {
            return "No previous optimization attempts for this agent/domain.".to_string();
        }

        let recent: Vec<_> = self.attempts.iter().rev().take(max_entries).collect();
        let mut lines = Vec::new();
        for a in recent.iter().rev() {
            let outcome_str = match &a.outcome {
                AttemptOutcome::Improved { delta } => format!("IMPROVED +{delta:.3}"),
                AttemptOutcome::NoImprovement => "NO_IMPROVEMENT".to_string(),
                AttemptOutcome::RolledBack { reason } => format!("ROLLED_BACK: {reason}"),
                AttemptOutcome::Rejected { reason } => format!("REJECTED: {reason}"),
            };
            let selected_str = a.selected.as_deref().unwrap_or("none");
            lines.push(format!(
                "- score_before={:.3} selected={} outcome={}",
                a.baseline_score_before, selected_str, outcome_str,
            ));
        }
        lines.join("\n")
    }

    /// Total number of attempts.
    pub fn len(&self) -> usize {
        self.attempts.len()
    }

    /// Whether there are no attempts.
    pub fn is_empty(&self) -> bool {
        self.attempts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trajectory() -> OptimizationTrajectory {
        OptimizationTrajectory::new("agent-1", ImprovementDomain::PromptOptimization)
    }

    #[test]
    fn test_trajectory_records_attempt() {
        let mut traj = make_trajectory();
        let id = traj.record(
            vec![("hash_a".into(), 0.8), ("hash_b".into(), 0.75)],
            Some("hash_a".into()),
            AttemptOutcome::Improved { delta: 0.05 },
            0.75,
        );
        assert_eq!(traj.len(), 1);
        assert_eq!(traj.attempts[0].id, id);
        assert_eq!(traj.attempts[0].variants_tried.len(), 2);
    }

    #[test]
    fn test_trajectory_feeds_meta_prompt() {
        let mut traj = make_trajectory();
        traj.record(
            vec![("h1".into(), 0.8)],
            Some("h1".into()),
            AttemptOutcome::Improved { delta: 0.05 },
            0.75,
        );
        traj.record(
            vec![("h2".into(), 0.7)],
            None,
            AttemptOutcome::NoImprovement,
            0.80,
        );

        let summary = traj.meta_prompt_summary(5);
        assert!(summary.contains("IMPROVED"), "should mention improvement");
        assert!(summary.contains("NO_IMPROVEMENT"), "should mention failure");
        assert!(summary.contains("0.750"), "should include score");
    }

    #[test]
    fn test_trajectory_tracks_outcomes() {
        let mut traj = make_trajectory();
        let id = traj.record(vec![], None, AttemptOutcome::Improved { delta: 0.1 }, 0.7);
        traj.update_after_score(id, 0.8);
        assert_eq!(traj.attempts[0].baseline_score_after, Some(0.8));
    }

    #[test]
    fn test_trajectory_calculates_success_rate() {
        let mut traj = make_trajectory();
        // 2 successes, 1 failure
        traj.record(vec![], None, AttemptOutcome::Improved { delta: 0.1 }, 0.7);
        traj.record(vec![], None, AttemptOutcome::NoImprovement, 0.8);
        traj.record(vec![], None, AttemptOutcome::Improved { delta: 0.05 }, 0.8);

        let rate = traj.success_rate(10);
        assert!(
            (rate - 2.0 / 3.0).abs() < 1e-9,
            "expected ~0.667, got {rate}"
        );
    }

    #[test]
    fn test_trajectory_limits_history_size() {
        let mut traj = make_trajectory();
        for _ in 0..150 {
            traj.record(vec![], None, AttemptOutcome::NoImprovement, 0.5);
        }
        assert!(
            traj.len() <= MAX_ATTEMPTS,
            "should cap at {MAX_ATTEMPTS}, got {}",
            traj.len()
        );
    }
}
