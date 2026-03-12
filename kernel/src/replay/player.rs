//! Replay player — verifies and compares evidence bundles.

use super::evidence::{ReplayBundle, ReplayVerdict};

/// Stateless replay verification engine.
pub struct ReplayPlayer;

impl ReplayPlayer {
    /// Verify a single bundle's integrity and governance correctness.
    pub fn verify_bundle(bundle: &ReplayBundle) -> ReplayVerdict {
        // 1. Check bundle hash integrity
        if !bundle.verify_integrity() {
            return ReplayVerdict::Diverged {
                reason: "bundle hash does not match recomputed hash (data tampered)".into(),
            };
        }

        // 2. Check pre-state exists (before other checks that depend on it)
        if bundle.pre_state.timestamp == 0 && bundle.pre_state.agent_capabilities.is_empty() {
            return ReplayVerdict::CannotReplay {
                reason: "pre-state is empty — cannot verify action context".into(),
            };
        }

        // 3. Check post-state exists
        if bundle.post_state.timestamp == 0 && bundle.post_state.agent_capabilities.is_empty() {
            return ReplayVerdict::CannotReplay {
                reason: "post-state is empty — cannot verify action outcome".into(),
            };
        }

        // 4. Check all governance checks passed
        for check in &bundle.governance_checks {
            if !check.passed {
                return ReplayVerdict::GovernanceViolation {
                    check: format!("{}: {}", check.check_type, check.details),
                };
            }
        }

        // 5. Check fuel was sufficient (pre_state fuel >= cost)
        if bundle.post_state.fuel_remaining > bundle.pre_state.fuel_remaining {
            return ReplayVerdict::Diverged {
                reason: format!(
                    "fuel increased from {} to {} (impossible without top-up)",
                    bundle.pre_state.fuel_remaining, bundle.post_state.fuel_remaining
                ),
            };
        }

        ReplayVerdict::Verified
    }

    /// Compare two bundles (original vs replayed) for divergence.
    pub fn compare_bundles(original: &ReplayBundle, replayed: &ReplayBundle) -> ReplayVerdict {
        // Both must have valid integrity
        if !original.verify_integrity() {
            return ReplayVerdict::Diverged {
                reason: "original bundle integrity check failed".into(),
            };
        }
        if !replayed.verify_integrity() {
            return ReplayVerdict::Diverged {
                reason: "replayed bundle integrity check failed".into(),
            };
        }

        // Pre-state capabilities should match
        if original.pre_state.agent_capabilities != replayed.pre_state.agent_capabilities {
            return ReplayVerdict::Diverged {
                reason: "pre-state capabilities differ between original and replay".into(),
            };
        }

        // Pre-state fuel should match
        if original.pre_state.fuel_remaining != replayed.pre_state.fuel_remaining {
            return ReplayVerdict::Diverged {
                reason: format!(
                    "pre-state fuel differs: original={}, replayed={}",
                    original.pre_state.fuel_remaining, replayed.pre_state.fuel_remaining
                ),
            };
        }

        // Governance checks count and pass/fail should match
        if original.governance_checks.len() != replayed.governance_checks.len() {
            return ReplayVerdict::Diverged {
                reason: format!(
                    "governance check count differs: original={}, replayed={}",
                    original.governance_checks.len(),
                    replayed.governance_checks.len()
                ),
            };
        }

        for (i, (o, r)) in original
            .governance_checks
            .iter()
            .zip(replayed.governance_checks.iter())
            .enumerate()
        {
            if o.check_type != r.check_type || o.passed != r.passed {
                return ReplayVerdict::Diverged {
                    reason: format!(
                        "governance check[{i}] differs: original=({}, {}), replayed=({}, {})",
                        o.check_type, o.passed, r.check_type, r.passed
                    ),
                };
            }
        }

        // Output hash should match
        if original.output_hash != replayed.output_hash {
            return ReplayVerdict::Diverged {
                reason: "output hash differs between original and replay".into(),
            };
        }

        ReplayVerdict::Verified
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::evidence::{GovernanceCheck, ReplayBundle, StateSnapshot};

    fn make_snapshot(fuel: u64, ts: u64) -> StateSnapshot {
        StateSnapshot {
            agent_capabilities: vec!["fs.read".into()],
            fuel_remaining: fuel,
            filesystem_permissions: vec![],
            active_model: Some("mock".into()),
            timestamp: ts,
            custom_state: serde_json::json!({}),
        }
    }

    fn make_valid_bundle(id: &str, output_hash: &str) -> ReplayBundle {
        let pre = make_snapshot(100, 1000);
        let post = make_snapshot(85, 1001);
        let checks = vec![
            GovernanceCheck {
                check_type: "capability".into(),
                passed: true,
                details: "fs.read allowed".into(),
                timestamp: 1000,
            },
            GovernanceCheck {
                check_type: "fuel".into(),
                passed: true,
                details: "100 >= 15".into(),
                timestamp: 1000,
            },
        ];
        let events = vec![serde_json::json!({"seq": 0})];

        let bundle_hash = ReplayBundle::compute_hash(
            id,
            "agent-1",
            "tool_call",
            &pre,
            &post,
            &checks,
            &events,
            "input_hash",
            output_hash,
        );

        ReplayBundle {
            id: id.into(),
            created_at: 1000,
            agent_id: "agent-1".into(),
            action_type: "tool_call".into(),
            pre_state: pre,
            post_state: post,
            governance_checks: checks,
            audit_events: events,
            input_hash: "input_hash".into(),
            output_hash: output_hash.into(),
            bundle_hash,
            replay_verdict: None,
        }
    }

    #[test]
    fn test_verify_bundle_passes() {
        let bundle = make_valid_bundle("b1", "out1");
        assert_eq!(
            ReplayPlayer::verify_bundle(&bundle),
            ReplayVerdict::Verified
        );
    }

    #[test]
    fn test_verify_bundle_governance_failure() {
        let pre = make_snapshot(100, 1000);
        let post = make_snapshot(100, 1001);
        let checks = vec![GovernanceCheck {
            check_type: "capability".into(),
            passed: false,
            details: "web.scrape not allowed".into(),
            timestamp: 1000,
        }];

        let bundle_hash =
            ReplayBundle::compute_hash("b1", "a1", "test", &pre, &post, &checks, &[], "in", "out");

        let bundle = ReplayBundle {
            id: "b1".into(),
            created_at: 1000,
            agent_id: "a1".into(),
            action_type: "test".into(),
            pre_state: pre,
            post_state: post,
            governance_checks: checks,
            audit_events: vec![],
            input_hash: "in".into(),
            output_hash: "out".into(),
            bundle_hash,
            replay_verdict: None,
        };

        match ReplayPlayer::verify_bundle(&bundle) {
            ReplayVerdict::GovernanceViolation { check } => {
                assert!(check.contains("capability"));
                assert!(check.contains("web.scrape"));
            }
            other => panic!("expected GovernanceViolation, got {other:?}"),
        }
    }

    #[test]
    fn test_compare_identical_bundles() {
        let b1 = make_valid_bundle("b1", "out1");
        let b2 = make_valid_bundle("b2", "out1");
        assert_eq!(
            ReplayPlayer::compare_bundles(&b1, &b2),
            ReplayVerdict::Verified
        );
    }

    #[test]
    fn test_compare_diverged_bundles() {
        let b1 = make_valid_bundle("b1", "out1");
        let b2 = make_valid_bundle("b2", "out_different");

        match ReplayPlayer::compare_bundles(&b1, &b2) {
            ReplayVerdict::Diverged { reason } => {
                assert!(reason.contains("output hash differs"));
            }
            other => panic!("expected Diverged, got {other:?}"),
        }
    }

    #[test]
    fn test_cannot_replay_without_pre_state() {
        let pre = StateSnapshot::default(); // empty
        let post = make_snapshot(85, 1001);
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

        match ReplayPlayer::verify_bundle(&bundle) {
            ReplayVerdict::CannotReplay { reason } => {
                assert!(reason.contains("pre-state"));
            }
            other => panic!("expected CannotReplay, got {other:?}"),
        }
    }

    #[test]
    fn test_cannot_replay_without_post_state() {
        let pre = make_snapshot(100, 1000);
        let post = StateSnapshot::default(); // empty
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

        match ReplayPlayer::verify_bundle(&bundle) {
            ReplayVerdict::CannotReplay { reason } => {
                assert!(reason.contains("post-state"));
            }
            other => panic!("expected CannotReplay, got {other:?}"),
        }
    }

    #[test]
    fn test_verify_tampered_bundle() {
        let mut bundle = make_valid_bundle("b1", "out1");
        bundle.agent_id = "tampered-agent".into(); // tamper

        match ReplayPlayer::verify_bundle(&bundle) {
            ReplayVerdict::Diverged { reason } => {
                assert!(reason.contains("tampered"));
            }
            other => panic!("expected Diverged, got {other:?}"),
        }
    }
}
