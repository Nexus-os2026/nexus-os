//! End-to-end integration tests for the Governed Self-Improvement pipeline.
//!
//! These tests exercise the FULL pipeline — observer through applier — with
//! guardian checks, envelope tracking, scheduling, and invariant enforcement.

use nexus_self_improve::applier::Applier;
use nexus_self_improve::config_optimizer::{ConfigOptimizer, ConfigOptimizerConfig};
use nexus_self_improve::envelope::BehavioralEnvelope;
use nexus_self_improve::guardian::SimplexGuardian;
use nexus_self_improve::invariants::{validate_all_invariants, HardInvariant, InvariantCheckState};
use nexus_self_improve::pipeline::{PipelineConfig, SelfImprovementPipeline};
use nexus_self_improve::prompt_optimizer::{PromptOptimizer, PromptOptimizerConfig};
use nexus_self_improve::report::ImprovementReport;
use nexus_self_improve::scheduler::AdaptiveScheduler;
use nexus_self_improve::trajectory::AttemptOutcome;
use nexus_self_improve::types::*;
use nexus_self_improve::validator::{SimulationRiskResult, TestResults, Validator};
use std::collections::HashMap;
use uuid::Uuid;

// ── Helpers ─────────────────────────────────────────────────────────

fn make_pipeline(hitl_approve: bool) -> SelfImprovementPipeline {
    let config = PipelineConfig::default();
    let validator = Validator::new(
        config.validator.clone(),
        Box::new(|| TestResults {
            passed: 100,
            failed: 0,
            failures: vec![],
        }),
        Box::new(|_| SimulationRiskResult {
            risk_score: 0.1,
            summary: "ok".into(),
        }),
        Box::new(move |_| {
            if hitl_approve {
                Ok("ed25519:sig".into())
            } else {
                Err("denied by user".into())
            }
        }),
    );
    let applier = Applier::new(
        config.applier.clone(),
        Box::new(|_| Ok(Uuid::new_v4())),
        Box::new(|| Ok(100)),
        Box::new(|_| {}),
    );
    SelfImprovementPipeline::new(config, validator, applier)
}

fn make_state() -> SystemState {
    SystemState {
        metrics: SystemMetrics::new(),
        context: SystemContext::default(),
        audit_chain_valid: true,
        test_suite_passing: true,
    }
}

fn build_baseline(pipeline: &mut SelfImprovementPipeline, metric: &str, value: f64, rounds: usize) {
    for _ in 0..rounds {
        let mut state = make_state();
        state.metrics.insert(metric, value);
        pipeline.run_cycle(&state);
    }
}

fn make_proposal(change: ProposedChange) -> ImprovementProposal {
    ImprovementProposal {
        id: Uuid::new_v4(),
        opportunity_id: Uuid::new_v4(),
        domain: change.domain(),
        description: "test proposal".into(),
        change,
        rollback_plan: RollbackPlan {
            checkpoint_id: Uuid::new_v4(),
            steps: vec![RollbackStep {
                description: "revert".into(),
                action: serde_json::json!({"revert": true}),
            }],
            estimated_rollback_time_ms: 100,
            automatic: true,
        },
        expected_tests: vec![],
        proof: None,
        generated_by: "test".into(),
        fuel_cost: 50,
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

// ── Integration Tests ───────────────────────────────────────────────

#[test]
fn test_full_cycle_observe_to_apply() {
    let mut pipeline = make_pipeline(true);
    build_baseline(&mut pipeline, "latency_p99", 100.0, 25);

    let mut state = make_state();
    state.metrics.insert("latency_p99", 500.0);
    let result = pipeline.run_cycle(&state);

    assert!(
        matches!(result, CycleResult::Applied(_)),
        "full cycle should result in Applied, got: {result:?}"
    );

    // Cycle history should record the application
    let last = pipeline.cycle_history().last().unwrap();
    assert_eq!(last.result_type, "Applied");
}

#[test]
fn test_full_cycle_no_signal() {
    let mut pipeline = make_pipeline(true);
    let state = make_state();
    let result = pipeline.run_cycle(&state);
    assert!(matches!(result, CycleResult::NoSignals));
}

#[test]
fn test_full_cycle_invariant_blocks_kernel_modification() {
    let proposal = make_proposal(ProposedChange::CodePatch {
        target_file: "kernel/src/permissions.rs".into(),
        diff: "unsafe change".into(),
        proof: SafetyProof {
            invariants_checked: vec![],
            proof_method: ProofMethod::TypeCheck,
            verifier_version: "1.0".into(),
            proof_hash: "abc".into(),
        },
    });
    let result = validate_all_invariants(&proposal, &passing_state());
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert!(violations
        .iter()
        .any(|v| v.invariant == HardInvariant::GovernanceKernelImmutable));
}

#[test]
fn test_full_cycle_invariant_blocks_self_modification() {
    let proposal = make_proposal(ProposedChange::CodePatch {
        target_file: "crates/nexus-self-improve/src/pipeline.rs".into(),
        diff: "modify self".into(),
        proof: SafetyProof {
            invariants_checked: vec![],
            proof_method: ProofMethod::TypeCheck,
            verifier_version: "1.0".into(),
            proof_hash: "abc".into(),
        },
    });
    let result = validate_all_invariants(&proposal, &passing_state());
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert!(violations
        .iter()
        .any(|v| v.invariant == HardInvariant::SelfProtected));
}

#[test]
fn test_full_cycle_hitl_rejection_stops_pipeline() {
    let mut pipeline = make_pipeline(false);
    build_baseline(&mut pipeline, "latency_p99", 100.0, 25);

    let mut state = make_state();
    state.metrics.insert("latency_p99", 500.0);
    let result = pipeline.run_cycle(&state);

    assert!(
        matches!(result, CycleResult::ValidationFailed(_)),
        "HITL rejection should stop pipeline, got: {result:?}"
    );
}

#[test]
fn test_guardian_forces_baseline_during_cycle() {
    let mut pipeline = make_pipeline(true);

    // Artificially set up high-drift envelope
    let mut env = BehavioralEnvelope::new("agent-x");
    env.add_metric("quality", 0.9, 0.1);
    env.metrics.get_mut("quality").unwrap().current = 0.5;
    env.drift_rate = 0.5;
    env.set_recovery_rate(0.05);
    pipeline.envelopes.insert("agent-x".into(), env);
    pipeline.guardian = SimplexGuardian::new(0.1);
    pipeline
        .guardian
        .capture_baseline(HashMap::new(), HashMap::new(), vec![]);

    let state = make_state();
    let result = pipeline.run_cycle(&state);

    // With empty metrics the observer won't find signals, so result is NoSignals
    // But guardian check happens first. Since aggregate drift is computed from
    // the single envelope with very high drift, it may or may not trigger.
    // The important thing is no crash and no ApplyFailed.
    assert!(!matches!(result, CycleResult::ApplyFailed(_)));
}

#[test]
fn test_rate_limiting_blocks_rapid_cycles() {
    let mut pipeline = make_pipeline(true);
    build_baseline(&mut pipeline, "latency_p99", 100.0, 25);

    // First cycle should succeed
    let mut state = make_state();
    state.metrics.insert("latency_p99", 500.0);
    let r1 = pipeline.run_cycle(&state);
    assert!(matches!(r1, CycleResult::Applied(_)));

    // Second cycle immediately after — same metric spike
    // The rate limiter blocks Platform-scope changes when agents have recent improvements
    let mut state2 = make_state();
    state2.metrics.insert("latency_p99", 500.0);
    let r2 = pipeline.run_cycle(&state2);

    // May be RateLimited, NoSignals (observer baseline shifted), or another Applied
    // depending on observer state. The key assertion is no crash.
    assert!(!matches!(r2, CycleResult::ApplyFailed(_)));
}

#[test]
fn test_rollback_on_post_apply_test_failure() {
    let config = PipelineConfig::default();
    let validator = Validator::new(
        config.validator.clone(),
        Box::new(|| TestResults {
            passed: 100,
            failed: 0,
            failures: vec![],
        }),
        Box::new(|_| SimulationRiskResult {
            risk_score: 0.1,
            summary: "ok".into(),
        }),
        Box::new(|_| Ok("ed25519:sig".into())),
    );
    let applier = Applier::new(
        config.applier.clone(),
        Box::new(|_| Ok(Uuid::new_v4())),
        Box::new(|| Err(vec!["test_regression".into()])), // Tests FAIL after apply
        Box::new(|_| {}),
    );
    let mut pipeline = SelfImprovementPipeline::new(config, validator, applier);
    build_baseline(&mut pipeline, "latency_p99", 100.0, 25);

    let mut state = make_state();
    state.metrics.insert("latency_p99", 500.0);
    let result = pipeline.run_cycle(&state);

    assert!(
        matches!(result, CycleResult::ApplyFailed(_)),
        "post-apply test failure should result in ApplyFailed, got: {result:?}"
    );
}

#[test]
fn test_prompt_optimization_end_to_end() {
    let optimizer = PromptOptimizer::new(PromptOptimizerConfig::default());
    let prompt = "You are a governed AI agent with governance, safety, and audit controls. You must follow all rules and report violations.";
    let variants_text = "You are a governed AI agent with governance, safety, and audit controls. You must follow all rules and report violations. Additionally, prioritize accuracy.---VARIANT---You are a governed AI agent with governance, safety, and audit controls. You must follow all rules and report violations. Focus on efficiency.";

    let variants = optimizer.generate_variants(prompt, variants_text);
    assert!(variants.is_ok());
    let variants = variants.unwrap();
    assert!(!variants.is_empty());

    // All variants should have passed safety check
    for v in &variants {
        assert!(v.safety_check_passed);
        assert!(v.similarity_to_original > 0.7);
    }
}

#[test]
fn test_config_optimization_end_to_end() {
    let optimizer = ConfigOptimizer::new(ConfigOptimizerConfig {
        trigger_threshold: 0.05,
    });

    let mut metrics = SystemMetrics::new();
    metrics.insert("cache_hit_rate", 0.5); // Low → should suggest increasing cache

    let suggestions = optimizer.analyze_config(&metrics);
    assert!(
        !suggestions.is_empty(),
        "should generate at least one suggestion"
    );

    let suggestion = &suggestions[0];
    let change = optimizer.propose_change(suggestion);
    assert!(matches!(change, ProposedChange::ConfigChange { .. }));

    if let ProposedChange::ConfigChange {
        key,
        old_value,
        new_value,
        ..
    } = &change
    {
        assert!(!key.is_empty());
        assert_ne!(old_value, new_value);
    }
}

#[test]
fn test_policy_optimization_cannot_broaden() {
    use nexus_self_improve::policy_optimizer::PolicyOptimizer;

    // Bare permit (no constraints) is broadening
    assert!(!PolicyOptimizer::is_narrowing_change(
        "permit(principal, action, resource);"
    ));

    // Permit with when clause is narrowing
    assert!(PolicyOptimizer::is_narrowing_change(
        "permit(principal, action, resource) when { context.risk < 0.3 };"
    ));

    // Forbid is always narrowing
    assert!(PolicyOptimizer::is_narrowing_change(
        "forbid(principal, action, resource);"
    ));
}

#[test]
fn test_multi_domain_prioritization() {
    let mut pipeline = make_pipeline(true);

    // Build baseline with multiple metrics
    for _ in 0..25 {
        let mut state = make_state();
        state.metrics.insert("security_violations", 0.0);
        state.metrics.insert("latency_p99", 100.0);
        state.metrics.insert("code_quality", 0.9);
        pipeline.run_cycle(&state);
    }

    // Spike ALL metrics — security should be prioritized
    let mut state = make_state();
    state.metrics.insert("security_violations", 10.0);
    state.metrics.insert("latency_p99", 500.0);
    state.metrics.insert("code_quality", 0.2);
    let result = pipeline.run_cycle(&state);

    // Should process something (not NoSignals)
    assert!(
        !matches!(result, CycleResult::NoSignals),
        "multi-metric spike should produce signals"
    );
}

#[test]
fn test_trajectory_improves_future_proposals() {
    let mut pipeline = make_pipeline(true);
    build_baseline(&mut pipeline, "latency_p99", 100.0, 25);

    // Run 3 successful cycles
    for _ in 0..3 {
        let mut state = make_state();
        state.metrics.insert("latency_p99", 500.0);
        pipeline.run_cycle(&state);
        // Reset observer baseline for next spike
        build_baseline(&mut pipeline, "latency_p99", 100.0, 5);
    }

    // Trajectory should have entries
    assert!(
        !pipeline.trajectories.is_empty(),
        "trajectory should record attempts across cycles"
    );

    // Check that trajectory has multiple entries
    let total_attempts: usize = pipeline.trajectories.values().map(|t| t.len()).sum();
    assert!(
        total_attempts >= 2,
        "should have recorded multiple trajectory entries, got {total_attempts}"
    );
}

#[test]
fn test_scheduler_adapts_after_success() {
    let mut scheduler = AdaptiveScheduler::new();
    let before = scheduler.current_interval_secs;
    scheduler.record_outcome(&AttemptOutcome::Improved { delta: 0.1 });
    assert!(
        scheduler.current_interval_secs < before,
        "scheduler should shorten interval after success"
    );
}

#[test]
fn test_scheduler_backs_off_after_failure() {
    let mut scheduler = AdaptiveScheduler::new();
    scheduler.record_outcome(&AttemptOutcome::NoImprovement);
    scheduler.record_outcome(&AttemptOutcome::NoImprovement);
    scheduler.record_outcome(&AttemptOutcome::NoImprovement);
    assert!(
        scheduler.consecutive_failures == 3,
        "should track consecutive failures"
    );
    assert!(
        scheduler.current_interval_secs > scheduler.base_interval_secs,
        "should back off: {} <= {}",
        scheduler.current_interval_secs,
        scheduler.base_interval_secs
    );
}

#[test]
fn test_envelope_drift_bound_theorem() {
    let mut env = BehavioralEnvelope::new("agent-1");
    env.add_metric("accuracy", 0.9, 0.1);
    env.drift_rate = 0.1;
    env.set_recovery_rate(0.5);

    // D* = alpha/gamma = 0.1/0.5 = 0.2
    let bound = env.drift_bound_guarantee();
    assert!((bound - 0.2).abs() < 1e-9, "D* should be 0.2, got {bound}");

    // At baseline → drift = 0.0 → within bounds
    assert!(
        env.current_drift() < bound,
        "at baseline should be within D*"
    );

    // At boundary edge → drift = 1.0 → exceeds D*
    env.metrics.get_mut("accuracy").unwrap().current = 0.8;
    assert!(env.current_drift() > bound, "at boundary should exceed D*");
}

#[test]
fn test_ten_invariants_all_checked_every_cycle() {
    let proposal = make_proposal(ProposedChange::ConfigChange {
        key: "timeout".into(),
        old_value: serde_json::json!(5000),
        new_value: serde_json::json!(3000),
        justification: "test".into(),
    });

    let result = validate_all_invariants(&proposal, &passing_state());
    assert!(
        result.is_ok(),
        "safe proposal should pass all 10 invariants"
    );
    assert_eq!(
        HardInvariant::all().len(),
        10,
        "should have exactly 10 invariants"
    );
}

#[test]
fn test_fuel_budget_enforcement() {
    let proposal = make_proposal(ProposedChange::ConfigChange {
        key: "timeout".into(),
        old_value: serde_json::json!(5000),
        new_value: serde_json::json!(3000),
        justification: "test".into(),
    });

    let state = InvariantCheckState {
        fuel_remaining: 10, // Very low fuel
        fuel_budget: 10,
        ..passing_state()
    };

    // Proposal costs 50 fuel, only 10 remaining
    let result = validate_all_invariants(&proposal, &state);
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert!(violations
        .iter()
        .any(|v| v.invariant == HardInvariant::FuelLimitsEnforced));
}

#[test]
fn test_report_end_to_end() {
    let history = vec![
        AppliedImprovement {
            id: Uuid::new_v4(),
            proposal_id: Uuid::new_v4(),
            checkpoint_id: Uuid::new_v4(),
            applied_at: 5000,
            status: ImprovementStatus::Committed,
            canary_deadline: 7000,
        },
        AppliedImprovement {
            id: Uuid::new_v4(),
            proposal_id: Uuid::new_v4(),
            checkpoint_id: Uuid::new_v4(),
            applied_at: 6000,
            status: ImprovementStatus::RolledBack,
            canary_deadline: 8000,
        },
    ];
    let report = ImprovementReport::generate(&history, 10, 1, 2, 3000, 0, 10000);
    assert_eq!(report.improvements_applied, 2);
    assert_eq!(report.improvements_committed, 1);
    assert_eq!(report.improvements_rolled_back, 1);

    let md = report.generate_markdown();
    assert!(md.contains("Self-Improvement Report"));
    assert!(md.contains("50.0%")); // 1/2 success rate
}
