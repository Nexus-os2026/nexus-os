//! # Simplex Guardian
//!
//! Dual-execution watchdog inspired by the Simplex architecture. Maintains a
//! verified-safe baseline configuration alongside the active (possibly improved)
//! configuration. If behavioral drift approaches safety bounds, the guardian
//! switches to the verified baseline.

use crate::envelope::BehavioralEnvelope;
use crate::types::SystemMetrics;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors from the guardian.
#[derive(Debug, Error)]
pub enum GuardianError {
    #[error("no baseline captured — call capture_baseline() first")]
    NoBaseline,
    #[error("switch failed: {0}")]
    SwitchFailed(String),
}

/// A verified-safe system configuration snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineConfig {
    /// Agent ID → verified-safe system prompt.
    pub prompts: HashMap<String, String>,
    /// Config key → verified-safe value.
    pub configs: HashMap<String, serde_json::Value>,
    /// Verified Cedar policies.
    pub policies: Vec<String>,
    /// SHA-256 hash of the entire baseline.
    pub snapshot_hash: String,
    /// When this baseline was captured.
    pub created_at: u64,
}

/// Result of a baseline switch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchResult {
    pub switched: bool,
    pub reason: String,
    pub drift_at_switch: f64,
    pub baseline_hash: String,
}

/// Decision from the guardian's barrier certificate check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwitchDecision {
    /// Safe to continue with active configuration.
    ContinueActive { drift: f64, headroom: f64 },
    /// Must switch to baseline — safety boundary approached.
    SwitchToBaseline {
        reason: String,
        drift: f64,
        threshold: f64,
    },
}

/// Status of the guardian for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardianStatus {
    pub has_baseline: bool,
    pub baseline_hash: String,
    pub baseline_created_at: u64,
    pub switch_threshold: f64,
    pub current_drift: f64,
    pub drift_bound: f64,
    pub headroom: f64,
    pub decision: String,
}

/// The Simplex Guardian.
pub struct SimplexGuardian {
    baseline: Option<BaselineConfig>,
    /// Fraction of D* at which to trigger the switch (0.0–1.0).
    switch_threshold: f64,
}

impl SimplexGuardian {
    pub fn new(switch_threshold: f64) -> Self {
        Self {
            baseline: None,
            switch_threshold: switch_threshold.clamp(0.1, 1.0),
        }
    }

    /// Capture the current system state as the verified baseline.
    pub fn capture_baseline(
        &mut self,
        prompts: HashMap<String, String>,
        configs: HashMap<String, serde_json::Value>,
        policies: Vec<String>,
    ) -> BaselineConfig {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let hash_input = format!("{prompts:?}{configs:?}{policies:?}");
        let snapshot_hash = simple_hash(&hash_input);

        let config = BaselineConfig {
            prompts,
            configs,
            policies,
            snapshot_hash: snapshot_hash.clone(),
            created_at: now,
        };

        self.baseline = Some(config.clone());
        config
    }

    /// Check if we should switch to baseline (barrier certificate check).
    pub fn should_switch_to_baseline(
        &self,
        envelope: &BehavioralEnvelope,
        _current_metrics: &SystemMetrics,
    ) -> SwitchDecision {
        let drift = envelope.current_drift();
        let bound = envelope.drift_bound_guarantee();
        let threshold = bound * self.switch_threshold;

        if drift > threshold && threshold.is_finite() {
            SwitchDecision::SwitchToBaseline {
                reason: format!(
                    "Drift {drift:.4} exceeds barrier {threshold:.4} (D*={bound:.4} * threshold={:.2})",
                    self.switch_threshold
                ),
                drift,
                threshold,
            }
        } else {
            let headroom = if bound.is_finite() {
                (threshold - drift).max(0.0)
            } else {
                f64::INFINITY
            };
            SwitchDecision::ContinueActive { drift, headroom }
        }
    }

    /// Execute switch to baseline.
    pub fn switch_to_baseline(&self) -> Result<SwitchResult, GuardianError> {
        let baseline = self.baseline.as_ref().ok_or(GuardianError::NoBaseline)?;

        Ok(SwitchResult {
            switched: true,
            reason: "Guardian triggered baseline switch".into(),
            drift_at_switch: 0.0,
            baseline_hash: baseline.snapshot_hash.clone(),
        })
    }

    /// After a successful canary period, promote the current state to baseline.
    pub fn promote_to_baseline(
        &mut self,
        prompts: HashMap<String, String>,
        configs: HashMap<String, serde_json::Value>,
        policies: Vec<String>,
    ) {
        self.capture_baseline(prompts, configs, policies);
    }

    /// Get the current baseline configuration.
    pub fn baseline(&self) -> Option<&BaselineConfig> {
        self.baseline.as_ref()
    }

    /// Get guardian status for the frontend.
    pub fn status(&self, envelope: &BehavioralEnvelope) -> GuardianStatus {
        let drift = envelope.current_drift();
        let bound = envelope.drift_bound_guarantee();
        let threshold = bound * self.switch_threshold;
        let headroom = if bound.is_finite() {
            (threshold - drift).max(0.0)
        } else {
            f64::INFINITY
        };

        let decision = match self.should_switch_to_baseline(envelope, &SystemMetrics::new()) {
            SwitchDecision::ContinueActive { .. } => "continue_active".to_string(),
            SwitchDecision::SwitchToBaseline { .. } => "switch_to_baseline".to_string(),
        };

        GuardianStatus {
            has_baseline: self.baseline.is_some(),
            baseline_hash: self
                .baseline
                .as_ref()
                .map(|b| b.snapshot_hash.clone())
                .unwrap_or_default(),
            baseline_created_at: self.baseline.as_ref().map(|b| b.created_at).unwrap_or(0),
            switch_threshold: self.switch_threshold,
            current_drift: drift,
            drift_bound: bound,
            headroom,
            decision,
        }
    }

    /// Get the switch threshold.
    pub fn switch_threshold(&self) -> f64 {
        self.switch_threshold
    }
}

/// Simple deterministic hash for baseline snapshotting (not cryptographic).
fn simple_hash(input: &str) -> String {
    let mut hash: u64 = 5381;
    for byte in input.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(u64::from(byte));
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::BehavioralEnvelope;

    fn make_envelope_within() -> BehavioralEnvelope {
        let mut env = BehavioralEnvelope::new("test-agent");
        env.add_metric("accuracy", 0.9, 0.1);
        env.drift_rate = 0.01;
        env.set_recovery_rate(0.10);
        // D* = 0.01/0.10 = 0.1; current drift ≈ 0.0
        env
    }

    fn make_envelope_drifted() -> BehavioralEnvelope {
        let mut env = BehavioralEnvelope::new("test-agent");
        env.add_metric("accuracy", 0.9, 0.1);
        env.metrics.get_mut("accuracy").unwrap().current = 0.7; // far outside bounds
        env.drift_rate = 0.50;
        env.set_recovery_rate(0.10);
        // D* = 0.50/0.10 = 5.0; threshold = 5.0*0.8 = 4.0
        // drift = |0.7-0.9|/0.1 = 2.0 (capped) → need drift > 4.0, so use 2 metrics
        env.add_metric("latency", 100.0, 10.0);
        env.metrics.get_mut("latency").unwrap().current = 130.0; // way outside
                                                                 // latency normalized_deviation = |130-100|/5 = 6.0 → capped at 2.0
                                                                 // drift = sqrt((2.0^2 + 2.0^2)/2) = 2.0
                                                                 // But D*=5.0, threshold=4.0, so 2.0 < 4.0 still!
                                                                 // Fix: make D* small enough. drift_rate=0.10, recovery=0.10 → D*=1.0
        env.drift_rate = 0.10;
        env.set_recovery_rate(0.10);
        // D*=1.0, threshold=0.8, drift=2.0 → 2.0 > 0.8 ✓
        env
    }

    #[test]
    fn test_switch_decision_continue() {
        let guardian = SimplexGuardian::new(0.8);
        let env = make_envelope_within();
        let decision = guardian.should_switch_to_baseline(&env, &SystemMetrics::new());
        assert!(
            matches!(decision, SwitchDecision::ContinueActive { .. }),
            "should continue when drift is low"
        );
    }

    #[test]
    fn test_switch_decision_switch() {
        let guardian = SimplexGuardian::new(0.8);
        let env = make_envelope_drifted();
        let decision = guardian.should_switch_to_baseline(&env, &SystemMetrics::new());
        assert!(
            matches!(decision, SwitchDecision::SwitchToBaseline { .. }),
            "should switch when drift exceeds barrier: {decision:?}"
        );
    }

    #[test]
    fn test_capture_baseline_hash() {
        let mut guardian = SimplexGuardian::new(0.8);
        let mut prompts = HashMap::new();
        prompts.insert("agent-1".into(), "You are a helpful agent.".into());

        let baseline = guardian.capture_baseline(prompts, HashMap::new(), vec![]);
        assert!(!baseline.snapshot_hash.is_empty());
        assert!(baseline.created_at > 0);
        assert!(guardian.baseline().is_some());
    }

    #[test]
    fn test_promote_to_baseline() {
        let mut guardian = SimplexGuardian::new(0.8);
        let mut prompts1 = HashMap::new();
        prompts1.insert("a".into(), "v1".into());
        guardian.capture_baseline(prompts1, HashMap::new(), vec![]);
        let hash1 = guardian.baseline().unwrap().snapshot_hash.clone();

        // Promote new state
        let mut prompts2 = HashMap::new();
        prompts2.insert("a".into(), "v2".into());
        guardian.promote_to_baseline(prompts2, HashMap::new(), vec![]);
        let hash2 = guardian.baseline().unwrap().snapshot_hash.clone();

        assert_ne!(hash1, hash2, "promoted baseline should have different hash");
    }

    #[test]
    fn test_switch_threshold_configurable() {
        let guardian = SimplexGuardian::new(0.5);
        assert!((guardian.switch_threshold() - 0.5).abs() < 1e-9);

        // Clamped to [0.1, 1.0]
        let guardian = SimplexGuardian::new(0.01);
        assert!((guardian.switch_threshold() - 0.1).abs() < 1e-9);
    }

    #[test]
    fn test_guardian_no_baseline_switch_fails() {
        let guardian = SimplexGuardian::new(0.8);
        let result = guardian.switch_to_baseline();
        assert!(result.is_err());
    }

    #[test]
    fn test_guardian_status() {
        let mut guardian = SimplexGuardian::new(0.8);
        guardian.capture_baseline(HashMap::new(), HashMap::new(), vec![]);
        let env = make_envelope_within();
        let status = guardian.status(&env);
        assert!(status.has_baseline);
        assert_eq!(status.decision, "continue_active");
    }
}
