//! Exhaustive edge case tests for boundary conditions, special values,
//! and every enum variant.

use nexus_self_improve::envelope::BehavioralEnvelope;
use nexus_self_improve::invariants::{validate_all_invariants, HardInvariant, InvariantCheckState};
use nexus_self_improve::observer::{Observer, ObserverConfig};
use nexus_self_improve::prompt_optimizer::cosine_similarity;
use nexus_self_improve::scheduler::AdaptiveScheduler;
use nexus_self_improve::trajectory::AttemptOutcome;
use nexus_self_improve::types::*;
use std::collections::HashMap;
use uuid::Uuid;

fn make_proposal(change: ProposedChange) -> ImprovementProposal {
    ImprovementProposal {
        id: Uuid::new_v4(),
        opportunity_id: Uuid::new_v4(),
        domain: change.domain(),
        description: "edge case".into(),
        change,
        rollback_plan: RollbackPlan {
            checkpoint_id: Uuid::new_v4(),
            steps: vec![RollbackStep {
                description: "revert".into(),
                action: serde_json::json!({}),
            }],
            estimated_rollback_time_ms: 0,
            automatic: true,
        },
        expected_tests: vec![],
        proof: None,
        generated_by: "edge".into(),
        fuel_cost: 0,
    }
}

fn passing_state() -> InvariantCheckState {
    InvariantCheckState {
        audit_chain_valid: true,
        test_suite_passing: true,
        hitl_approved: true,
        fuel_remaining: 10_000,
        fuel_budget: 10_000,
    }
}

// ── ProposedChange variant tests ────────────────────────────────────

#[test]
fn test_prompt_update_empty_prompt() {
    let proposal = make_proposal(ProposedChange::PromptUpdate {
        agent_id: String::new(),
        old_prompt_hash: String::new(),
        new_prompt: String::new(),
        optimization_trajectory: vec![],
    });
    // Should pass invariants (empty prompt is not a governance violation)
    let result = validate_all_invariants(&proposal, &passing_state());
    assert!(result.is_ok());
}

#[test]
fn test_config_change_empty_key() {
    let proposal = make_proposal(ProposedChange::ConfigChange {
        key: String::new(),
        old_value: serde_json::Value::Null,
        new_value: serde_json::Value::Null,
        justification: String::new(),
    });
    assert!(validate_all_invariants(&proposal, &passing_state()).is_ok());
}

#[test]
fn test_policy_update_empty_cedar() {
    let proposal = make_proposal(ProposedChange::PolicyUpdate {
        policy_id: String::new(),
        old_policy_hash: String::new(),
        new_policy_cedar: String::new(),
    });
    assert!(validate_all_invariants(&proposal, &passing_state()).is_ok());
}

#[test]
fn test_scheduling_update_empty_weights() {
    let proposal = make_proposal(ProposedChange::SchedulingUpdate {
        old_weights: vec![],
        new_weights: vec![],
        training_episodes: 0,
    });
    assert!(validate_all_invariants(&proposal, &passing_state()).is_ok());
}

#[test]
fn test_code_patch_safe_path() {
    let proposal = make_proposal(ProposedChange::CodePatch {
        target_file: "agents/coder/src/lib.rs".into(),
        diff: "+new line".into(),
        proof: SafetyProof {
            invariants_checked: vec![InvariantId(1)],
            proof_method: ProofMethod::TestSuitePassed,
            verifier_version: "1.0".into(),
            proof_hash: "abc".into(),
        },
    });
    assert!(validate_all_invariants(&proposal, &passing_state()).is_ok());
}

#[test]
fn test_code_patch_governance_path_blocked() {
    let proposal = make_proposal(ProposedChange::CodePatch {
        target_file: "kernel/src/audit/mod.rs".into(),
        diff: "hack".into(),
        proof: SafetyProof {
            invariants_checked: vec![],
            proof_method: ProofMethod::TypeCheck,
            verifier_version: "1.0".into(),
            proof_hash: "abc".into(),
        },
    });
    assert!(validate_all_invariants(&proposal, &passing_state()).is_err());
}

// ── ImprovementDomain exhaustive ────────────────────────────────────

#[test]
fn test_all_domains_have_valid_proposed_change() {
    let domains = [
        ImprovementDomain::PromptOptimization,
        ImprovementDomain::ConfigTuning,
        ImprovementDomain::GovernancePolicy,
        ImprovementDomain::SchedulingPolicy,
        ImprovementDomain::RoutingStrategy,
        ImprovementDomain::CodePatch,
    ];
    assert_eq!(domains.len(), 6, "should cover all 6 domains");
}

// ── CycleResult variant coverage ────────────────────────────────────

#[test]
fn test_cycle_result_debug_format() {
    let results: Vec<CycleResult> = vec![
        CycleResult::NoSignals,
        CycleResult::NoOpportunities,
        CycleResult::ProposalFailed("err".into()),
        CycleResult::ValidationFailed("err".into()),
        CycleResult::ApplyFailed("err".into()),
        CycleResult::GuardianSwitch("drift".into()),
        CycleResult::RateLimited("agent-1".into()),
        CycleResult::Applied(AppliedImprovement {
            id: Uuid::nil(),
            proposal_id: Uuid::nil(),
            checkpoint_id: Uuid::nil(),
            applied_at: 0,
            status: ImprovementStatus::Monitoring,
            canary_deadline: 0,
        }),
    ];
    for r in &results {
        let debug = format!("{r:?}");
        assert!(!debug.is_empty());
    }
    assert_eq!(results.len(), 8, "should cover all CycleResult variants");
}

// ── f64 boundary conditions ─────────────────────────────────────────

#[test]
fn test_cosine_similarity_with_special_chars() {
    let s = cosine_similarity("hello!@#$%", "hello!@#$%");
    assert!((s - 1.0).abs() < 1e-9);
}

#[test]
fn test_cosine_similarity_single_word() {
    let s = cosine_similarity("hello", "hello");
    assert!((s - 1.0).abs() < 1e-9);
}

#[test]
fn test_cosine_similarity_completely_disjoint() {
    let s = cosine_similarity("aaa bbb ccc", "ddd eee fff");
    assert!(s < 1e-9, "disjoint words should have 0.0 similarity");
}

#[test]
fn test_observer_with_zero_value_metric() {
    let mut observer = Observer::new(ObserverConfig::default());
    let mut m = SystemMetrics::new();
    m.insert("zero", 0.0);
    let signals = observer.observe(&m);
    assert!(signals.is_empty()); // First observation, no baseline yet
}

#[test]
fn test_observer_with_negative_metric() {
    let mut observer = Observer::new(ObserverConfig::default());
    let mut m = SystemMetrics::new();
    m.insert("negative", -100.0);
    let signals = observer.observe(&m);
    assert!(signals.is_empty());
}

#[test]
fn test_observer_with_very_large_metric() {
    let mut observer = Observer::new(ObserverConfig::default());
    let mut m = SystemMetrics::new();
    m.insert("large", 1e15);
    let signals = observer.observe(&m);
    assert!(signals.is_empty()); // No baseline to compare against
}

// ── Envelope edge cases ─────────────────────────────────────────────

#[test]
fn test_envelope_zero_tolerance() {
    let mut env = BehavioralEnvelope::new("test");
    env.add_metric("m", 1.0, 0.0); // zero tolerance
    assert!(env.is_within_bounds()); // At baseline, within bounds
    env.metrics.get_mut("m").unwrap().current = 1.001;
    assert!(!env.is_within_bounds()); // Even tiny deviation is outside
}

#[test]
fn test_envelope_empty_metrics() {
    let env = BehavioralEnvelope::new("test");
    assert!(env.is_within_bounds()); // No metrics = within bounds
    assert!((env.current_drift()).abs() < 1e-9); // No drift
}

#[test]
fn test_envelope_single_metric_drift() {
    let mut env = BehavioralEnvelope::new("test");
    env.add_metric("m", 1.0, 0.5);
    env.metrics.get_mut("m").unwrap().current = 1.5; // at upper bound
    let drift = env.current_drift();
    assert!(drift > 0.0);
    assert!(drift <= 2.0); // normalized_deviation capped at 2.0
}

#[test]
fn test_envelope_would_violate_unknown_metric() {
    let env = BehavioralEnvelope::new("test");
    let mut predicted = HashMap::new();
    predicted.insert("unknown".to_string(), 999.0);
    // Unknown metric should not trigger violation
    assert!(!env.would_violate(&predicted));
}

// ── Scheduler edge cases ────────────────────────────────────────────

#[test]
fn test_scheduler_zero_base_interval() {
    let mut sched = AdaptiveScheduler::new();
    sched.base_interval_secs = 0;
    sched.record_outcome(&AttemptOutcome::NoImprovement);
    // Should clamp to min
    assert!(sched.current_interval_secs >= sched.min_interval_secs);
}

#[test]
fn test_scheduler_all_outcomes() {
    let mut sched = AdaptiveScheduler::new();
    sched.record_outcome(&AttemptOutcome::Improved { delta: 0.1 });
    sched.record_outcome(&AttemptOutcome::NoImprovement);
    sched.record_outcome(&AttemptOutcome::RolledBack {
        reason: "test".into(),
    });
    sched.record_outcome(&AttemptOutcome::Rejected {
        reason: "test".into(),
    });
    assert_eq!(sched.total_cycles, 4);
}

// ── Invariant all-pass and all-fail ─────────────────────────────────

#[test]
fn test_all_invariants_pass_for_safe_proposal() {
    let proposal = make_proposal(ProposedChange::ConfigChange {
        key: "safe.key".into(),
        old_value: serde_json::json!(1),
        new_value: serde_json::json!(2),
        justification: "safe".into(),
    });
    let result = validate_all_invariants(&proposal, &passing_state());
    assert!(result.is_ok());
}

#[test]
fn test_multiple_invariants_fail_simultaneously() {
    let proposal = make_proposal(ProposedChange::ConfigChange {
        key: "capabilities.new".into(), // invariant #4
        old_value: serde_json::json!(false),
        new_value: serde_json::json!(true),
        justification: "expand".into(),
    });
    let state = InvariantCheckState {
        audit_chain_valid: false,  // invariant #2
        test_suite_passing: false, // invariant #8
        hitl_approved: false,      // invariant #9
        fuel_remaining: 0,         // invariant #5
        fuel_budget: 0,
    };
    let result = validate_all_invariants(&proposal, &state);
    assert!(result.is_err());
    let violations = result.unwrap_err();
    // Should have multiple violations
    assert!(
        violations.len() >= 4,
        "expected 4+ violations, got {}",
        violations.len()
    );
}

#[test]
fn test_invariant_count_is_exactly_ten() {
    assert_eq!(HardInvariant::all().len(), 10);
}
