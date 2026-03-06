//! Adaptive governance: trust scores, autonomy promotion/demotion with human approval gates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RunOutcome {
    Success,
    Failed { reason: String },
    PolicyViolation { violation: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTrackRecord {
    pub agent_id: Uuid,
    pub total_runs: u64,
    pub successful_runs: u64,
    pub failed_runs: u64,
    pub policy_violations: u64,
    pub approval_overrides: u64,
    pub fuel_efficiency: f64,
    pub last_violation_at: Option<u64>,
    pub trust_score: f64,
}

impl AgentTrackRecord {
    fn new(agent_id: Uuid) -> Self {
        Self {
            agent_id,
            total_runs: 0,
            successful_runs: 0,
            failed_runs: 0,
            policy_violations: 0,
            approval_overrides: 0,
            fuel_efficiency: 1.0,
            last_violation_at: None,
            trust_score: 0.5,
        }
    }

    fn recalculate_trust_score(&mut self) {
        if self.total_runs == 0 {
            self.trust_score = 0.5;
            return;
        }
        let success_ratio = self.successful_runs as f64 / self.total_runs as f64;
        let violation_penalty = 1.0 - (self.policy_violations as f64 * 0.2);
        self.trust_score = (success_ratio * violation_penalty).clamp(0.0, 1.0);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AutonomyChange {
    Promote { from: u8, to: u8 },
    Demote { from: u8, to: u8, reason: String },
    NoChange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptivePolicy {
    pub agent_id: Uuid,
    pub base_autonomy: u8,
    pub current_autonomy: u8,
    pub promotion_threshold: f64,
    pub demotion_threshold: f64,
    pub max_autonomy: u8,
    pub cooldown_after_violation_secs: u64,
}

impl AdaptivePolicy {
    fn new(agent_id: Uuid, base_autonomy: u8, max_autonomy: u8) -> Self {
        Self {
            agent_id,
            base_autonomy,
            current_autonomy: base_autonomy,
            promotion_threshold: 0.85,
            demotion_threshold: 0.3,
            max_autonomy,
            cooldown_after_violation_secs: 86400,
        }
    }
}

#[derive(Debug)]
pub struct AdaptiveGovernor {
    records: HashMap<Uuid, AgentTrackRecord>,
    policies: HashMap<Uuid, AdaptivePolicy>,
}

impl AdaptiveGovernor {
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
            policies: HashMap::new(),
        }
    }

    pub fn register(&mut self, agent_id: Uuid, base_autonomy: u8, max_autonomy: u8) {
        self.records
            .insert(agent_id, AgentTrackRecord::new(agent_id));
        self.policies.insert(
            agent_id,
            AdaptivePolicy::new(agent_id, base_autonomy, max_autonomy),
        );
    }

    pub fn record_run(
        &mut self,
        agent_id: Uuid,
        outcome: RunOutcome,
        fuel_used: u64,
        fuel_budget: u64,
    ) {
        let Some(record) = self.records.get_mut(&agent_id) else {
            return;
        };

        record.total_runs += 1;

        match outcome {
            RunOutcome::Success => {
                record.successful_runs += 1;
            }
            RunOutcome::Failed { .. } => {
                record.failed_runs += 1;
            }
            RunOutcome::PolicyViolation { .. } => {
                record.policy_violations += 1;
                record.last_violation_at = Some(unix_now());
            }
        }

        // Update fuel efficiency as running average
        if fuel_budget > 0 {
            let this_efficiency = fuel_used as f64 / fuel_budget as f64;
            let n = record.total_runs as f64;
            record.fuel_efficiency =
                record.fuel_efficiency * ((n - 1.0) / n) + this_efficiency * (1.0 / n);
        }

        record.recalculate_trust_score();
    }

    pub fn evaluate(&self, agent_id: Uuid) -> AutonomyChange {
        let Some(record) = self.records.get(&agent_id) else {
            return AutonomyChange::NoChange;
        };
        let Some(policy) = self.policies.get(&agent_id) else {
            return AutonomyChange::NoChange;
        };

        // Demotion check (auto-apply, no approval)
        if record.trust_score < policy.demotion_threshold && policy.current_autonomy > 0 {
            let new_level = policy.current_autonomy.saturating_sub(1);
            return AutonomyChange::Demote {
                from: policy.current_autonomy,
                to: new_level,
                reason: format!(
                    "trust score {:.2} below demotion threshold {:.2}",
                    record.trust_score, policy.demotion_threshold
                ),
            };
        }

        // Promotion check (requires human approval)
        if record.trust_score >= policy.promotion_threshold
            && record.policy_violations == 0
            && policy.current_autonomy < policy.max_autonomy
        {
            return AutonomyChange::Promote {
                from: policy.current_autonomy,
                to: policy.current_autonomy + 1,
            };
        }

        AutonomyChange::NoChange
    }

    /// Apply a promotion — only if approved by a human.
    pub fn apply_promotion(&mut self, agent_id: Uuid, approved: bool) -> bool {
        if !approved {
            return false;
        }
        let Some(policy) = self.policies.get_mut(&agent_id) else {
            return false;
        };
        if policy.current_autonomy < policy.max_autonomy {
            policy.current_autonomy += 1;
            if let Some(record) = self.records.get_mut(&agent_id) {
                record.approval_overrides += 1;
            }
            true
        } else {
            false
        }
    }

    /// Apply a demotion — automatic, no approval needed.
    pub fn apply_demotion(&mut self, agent_id: Uuid) -> bool {
        let Some(policy) = self.policies.get_mut(&agent_id) else {
            return false;
        };
        if policy.current_autonomy > 0 {
            policy.current_autonomy -= 1;
            true
        } else {
            false
        }
    }

    pub fn get_record(&self, agent_id: Uuid) -> Option<&AgentTrackRecord> {
        self.records.get(&agent_id)
    }

    pub fn get_policy(&self, agent_id: Uuid) -> Option<&AdaptivePolicy> {
        self.policies.get(&agent_id)
    }
}

impl Default for AdaptiveGovernor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_score_increases_with_successes() {
        let mut gov = AdaptiveGovernor::new();
        let agent = Uuid::new_v4();
        gov.register(agent, 1, 4);

        // Run 10 successful runs
        for _ in 0..10 {
            gov.record_run(agent, RunOutcome::Success, 50, 100);
        }

        let record = gov.get_record(agent).unwrap();
        assert_eq!(record.total_runs, 10);
        assert_eq!(record.successful_runs, 10);
        // trust = (10/10) * (1.0 - 0*0.2) = 1.0
        assert!((record.trust_score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn trust_score_decreases_with_violations() {
        let mut gov = AdaptiveGovernor::new();
        let agent = Uuid::new_v4();
        gov.register(agent, 2, 4);

        // 5 successes, 5 violations
        for _ in 0..5 {
            gov.record_run(agent, RunOutcome::Success, 50, 100);
        }
        for _ in 0..5 {
            gov.record_run(
                agent,
                RunOutcome::PolicyViolation {
                    violation: "test".to_string(),
                },
                50,
                100,
            );
        }

        let record = gov.get_record(agent).unwrap();
        assert_eq!(record.policy_violations, 5);
        // trust = (5/10) * (1.0 - 5*0.2) = 0.5 * 0.0 = 0.0
        assert!(record.trust_score < 0.01);
    }

    #[test]
    fn promotion_suggested_at_threshold() {
        let mut gov = AdaptiveGovernor::new();
        let agent = Uuid::new_v4();
        gov.register(agent, 1, 3);

        // Get trust score above 0.85 with all successes
        for _ in 0..20 {
            gov.record_run(agent, RunOutcome::Success, 50, 100);
        }

        let change = gov.evaluate(agent);
        assert_eq!(change, AutonomyChange::Promote { from: 1, to: 2 });
    }

    #[test]
    fn promotion_only_applies_when_approved() {
        let mut gov = AdaptiveGovernor::new();
        let agent = Uuid::new_v4();
        gov.register(agent, 1, 3);

        // Denied promotion
        assert!(!gov.apply_promotion(agent, false));
        assert_eq!(gov.get_policy(agent).unwrap().current_autonomy, 1);

        // Approved promotion
        assert!(gov.apply_promotion(agent, true));
        assert_eq!(gov.get_policy(agent).unwrap().current_autonomy, 2);
    }

    #[test]
    fn demotion_applies_automatically() {
        let mut gov = AdaptiveGovernor::new();
        let agent = Uuid::new_v4();
        gov.register(agent, 2, 4);

        // Drive trust score below demotion threshold with violations
        for _ in 0..3 {
            gov.record_run(
                agent,
                RunOutcome::PolicyViolation {
                    violation: "bad".to_string(),
                },
                100,
                100,
            );
        }

        let record = gov.get_record(agent).unwrap();
        assert!(record.trust_score < 0.3);

        let change = gov.evaluate(agent);
        match change {
            AutonomyChange::Demote { from, to, .. } => {
                assert_eq!(from, 2);
                assert_eq!(to, 1);
            }
            _ => panic!("expected Demote, got {:?}", change),
        }

        // Apply demotion (no approval needed)
        assert!(gov.apply_demotion(agent));
        assert_eq!(gov.get_policy(agent).unwrap().current_autonomy, 1);
    }

    #[test]
    fn max_autonomy_ceiling_never_exceeded() {
        let mut gov = AdaptiveGovernor::new();
        let agent = Uuid::new_v4();
        gov.register(agent, 2, 3); // max is 3

        // Promote to 3
        assert!(gov.apply_promotion(agent, true));
        assert_eq!(gov.get_policy(agent).unwrap().current_autonomy, 3);

        // Try to promote past max — should fail
        assert!(!gov.apply_promotion(agent, true));
        assert_eq!(gov.get_policy(agent).unwrap().current_autonomy, 3);

        // evaluate should return NoChange at max
        for _ in 0..20 {
            gov.record_run(agent, RunOutcome::Success, 50, 100);
        }
        let change = gov.evaluate(agent);
        assert_eq!(change, AutonomyChange::NoChange);
    }

    #[test]
    fn violation_resets_trust_score_downward() {
        let mut gov = AdaptiveGovernor::new();
        let agent = Uuid::new_v4();
        gov.register(agent, 1, 4);

        // Build up a good score
        for _ in 0..10 {
            gov.record_run(agent, RunOutcome::Success, 50, 100);
        }
        let before = gov.get_record(agent).unwrap().trust_score;
        assert!((before - 1.0).abs() < f64::EPSILON);

        // Single violation drops the score
        gov.record_run(
            agent,
            RunOutcome::PolicyViolation {
                violation: "oops".to_string(),
            },
            50,
            100,
        );

        let after = gov.get_record(agent).unwrap().trust_score;
        assert!(after < before);
        assert!(gov.get_record(agent).unwrap().last_violation_at.is_some());
    }

    #[test]
    fn failures_reduce_trust_without_violation_penalty() {
        let mut gov = AdaptiveGovernor::new();
        let agent = Uuid::new_v4();
        gov.register(agent, 1, 3);

        // 5 successes, 5 failures (no violations)
        for _ in 0..5 {
            gov.record_run(agent, RunOutcome::Success, 50, 100);
        }
        for _ in 0..5 {
            gov.record_run(
                agent,
                RunOutcome::Failed {
                    reason: "timeout".to_string(),
                },
                50,
                100,
            );
        }

        let record = gov.get_record(agent).unwrap();
        // trust = (5/10) * (1.0 - 0*0.2) = 0.5
        assert!((record.trust_score - 0.5).abs() < f64::EPSILON);
        assert_eq!(record.policy_violations, 0);
    }

    #[test]
    fn unregistered_agent_returns_no_change() {
        let gov = AdaptiveGovernor::new();
        assert_eq!(gov.evaluate(Uuid::new_v4()), AutonomyChange::NoChange);
    }
}
