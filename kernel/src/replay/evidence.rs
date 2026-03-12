//! Evidence types for replay — state snapshots, governance checks, and verdicts.

use serde::{Deserialize, Serialize};

/// Snapshot of agent state at a point in time (pre- or post-action).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub agent_capabilities: Vec<String>,
    pub fuel_remaining: u64,
    pub filesystem_permissions: Vec<serde_json::Value>,
    pub active_model: Option<String>,
    pub timestamp: u64,
    pub custom_state: serde_json::Value,
}

impl Default for StateSnapshot {
    fn default() -> Self {
        Self {
            agent_capabilities: Vec::new(),
            fuel_remaining: 0,
            filesystem_permissions: Vec::new(),
            active_model: None,
            timestamp: 0,
            custom_state: serde_json::Value::Null,
        }
    }
}

/// A single governance check performed during an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceCheck {
    /// "capability", "fuel", "pii_redaction", "hitl_approval"
    pub check_type: String,
    pub passed: bool,
    pub details: String,
    pub timestamp: u64,
}

/// Outcome of replaying / verifying an evidence bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplayVerdict {
    /// Replay matches original execution.
    Verified,
    /// Different outcome detected.
    Diverged { reason: String },
    /// A governance check that should have passed did not.
    GovernanceViolation { check: String },
    /// Cannot replay because required state is missing.
    CannotReplay { reason: String },
}

/// A self-contained replay evidence bundle that captures everything needed
/// to prove exactly what happened during a single agent action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayBundle {
    pub id: String,
    pub created_at: u64,
    pub agent_id: String,
    pub action_type: String,
    pub pre_state: StateSnapshot,
    pub post_state: StateSnapshot,
    pub governance_checks: Vec<GovernanceCheck>,
    pub audit_events: Vec<serde_json::Value>,
    pub input_hash: String,
    pub output_hash: String,
    /// SHA-256 hash of the entire bundle for integrity verification.
    pub bundle_hash: String,
    pub replay_verdict: Option<ReplayVerdict>,
}

impl ReplayBundle {
    /// Compute the SHA-256 hash of the bundle's content fields.
    #[allow(clippy::too_many_arguments)]
    pub fn compute_hash(
        id: &str,
        agent_id: &str,
        action_type: &str,
        pre_state: &StateSnapshot,
        post_state: &StateSnapshot,
        governance_checks: &[GovernanceCheck],
        audit_events: &[serde_json::Value],
        input_hash: &str,
        output_hash: &str,
    ) -> String {
        use sha2::{Digest, Sha256};

        #[derive(Serialize)]
        struct HashInput<'a> {
            id: &'a str,
            agent_id: &'a str,
            action_type: &'a str,
            pre_state: &'a StateSnapshot,
            post_state: &'a StateSnapshot,
            governance_check_count: usize,
            audit_event_count: usize,
            input_hash: &'a str,
            output_hash: &'a str,
        }

        let input = HashInput {
            id,
            agent_id,
            action_type,
            pre_state,
            post_state,
            governance_check_count: governance_checks.len(),
            audit_event_count: audit_events.len(),
            input_hash,
            output_hash,
        };

        let canonical =
            serde_json::to_vec(&input).expect("replay bundle hash serialization must not fail");

        let mut hasher = Sha256::new();
        hasher.update(&canonical);

        // Include governance check details in hash
        for check in governance_checks {
            let check_bytes = serde_json::to_vec(check).unwrap_or_default();
            hasher.update(&check_bytes);
        }

        // Include audit events in hash
        for event in audit_events {
            let event_bytes = serde_json::to_vec(event).unwrap_or_default();
            hasher.update(&event_bytes);
        }

        format!("{:x}", hasher.finalize())
    }

    /// Recompute the bundle hash and check it matches the stored value.
    pub fn verify_integrity(&self) -> bool {
        let recomputed = Self::compute_hash(
            &self.id,
            &self.agent_id,
            &self.action_type,
            &self.pre_state,
            &self.post_state,
            &self.governance_checks,
            &self.audit_events,
            &self.input_hash,
            &self.output_hash,
        );
        recomputed == self.bundle_hash
    }
}

/// Compute SHA-256 hash of a JSON value for input/output hashing.
pub fn hash_json(value: &serde_json::Value) -> String {
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

fn _now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot(fuel: u64) -> StateSnapshot {
        StateSnapshot {
            agent_capabilities: vec!["fs.read".into(), "process.exec".into()],
            fuel_remaining: fuel,
            filesystem_permissions: vec![],
            active_model: Some("mock".into()),
            timestamp: 1000,
            custom_state: serde_json::json!({}),
        }
    }

    #[test]
    fn test_hash_deterministic() {
        let pre = make_snapshot(100);
        let post = make_snapshot(85);
        let checks = vec![GovernanceCheck {
            check_type: "capability".into(),
            passed: true,
            details: "ok".into(),
            timestamp: 1000,
        }];
        let events = vec![serde_json::json!({"seq": 0})];

        let h1 = ReplayBundle::compute_hash(
            "b1", "a1", "test", &pre, &post, &checks, &events, "in1", "out1",
        );
        let h2 = ReplayBundle::compute_hash(
            "b1", "a1", "test", &pre, &post, &checks, &events, "in1", "out1",
        );
        assert_eq!(h1, h2);
        assert!(!h1.is_empty());
    }

    #[test]
    fn test_hash_changes_with_different_input() {
        let pre = make_snapshot(100);
        let post = make_snapshot(85);

        let h1 =
            ReplayBundle::compute_hash("b1", "a1", "test", &pre, &post, &[], &[], "in1", "out1");
        let h2 =
            ReplayBundle::compute_hash("b1", "a1", "test", &pre, &post, &[], &[], "in2", "out1");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_verify_integrity_passes() {
        let pre = make_snapshot(100);
        let post = make_snapshot(85);
        let hash =
            ReplayBundle::compute_hash("b1", "a1", "test", &pre, &post, &[], &[], "in", "out");

        let bundle = ReplayBundle {
            id: "b1".into(),
            created_at: 1000,
            agent_id: "a1".into(),
            action_type: "test".into(),
            pre_state: pre,
            post_state: post,
            governance_checks: vec![],
            audit_events: vec![],
            input_hash: "in".into(),
            output_hash: "out".into(),
            bundle_hash: hash,
            replay_verdict: None,
        };

        assert!(bundle.verify_integrity());
    }

    #[test]
    fn test_verify_integrity_fails_on_tamper() {
        let pre = make_snapshot(100);
        let post = make_snapshot(85);
        let hash =
            ReplayBundle::compute_hash("b1", "a1", "test", &pre, &post, &[], &[], "in", "out");

        let mut bundle = ReplayBundle {
            id: "b1".into(),
            created_at: 1000,
            agent_id: "a1".into(),
            action_type: "test".into(),
            pre_state: pre,
            post_state: post,
            governance_checks: vec![],
            audit_events: vec![],
            input_hash: "in".into(),
            output_hash: "out".into(),
            bundle_hash: hash,
            replay_verdict: None,
        };

        // Tamper
        bundle.output_hash = "tampered".into();
        assert!(!bundle.verify_integrity());
    }

    #[test]
    fn test_replay_verdict_serde() {
        let v = ReplayVerdict::Diverged {
            reason: "output mismatch".into(),
        };
        let json = serde_json::to_string(&v).unwrap();
        let back: ReplayVerdict = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_hash_json() {
        let v1 = serde_json::json!({"key": "value"});
        let v2 = serde_json::json!({"key": "value"});
        assert_eq!(hash_json(&v1), hash_json(&v2));

        let v3 = serde_json::json!({"key": "other"});
        assert_ne!(hash_json(&v1), hash_json(&v3));
    }
}
