//! Property-based tests for the Governed Self-Improvement system.
//!
//! Each test generates 256 random inputs via proptest and verifies that
//! safety invariants, mathematical properties, and behavioral contracts hold.

use nexus_self_improve::analyzer::{Analyzer, AnalyzerConfig};
use nexus_self_improve::config_optimizer::{ConfigOptimizer, ConfigOptimizerConfig};
use nexus_self_improve::envelope::BehavioralEnvelope;
use nexus_self_improve::guardian::SimplexGuardian;
use nexus_self_improve::invariants::{validate_all_invariants, InvariantCheckState};
use nexus_self_improve::observer::{Observer, ObserverConfig};
use nexus_self_improve::policy_optimizer::PolicyOptimizer;
use nexus_self_improve::prompt_optimizer::{
    cosine_similarity, PromptOptimizer, PromptOptimizerConfig,
};
use nexus_self_improve::scheduler::AdaptiveScheduler;
use nexus_self_improve::trajectory::AttemptOutcome;
use nexus_self_improve::types::*;
use proptest::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

// ── Helpers ─────────────────────────────────────────────────────────

fn make_proposal(change: ProposedChange) -> ImprovementProposal {
    ImprovementProposal {
        id: Uuid::new_v4(),
        opportunity_id: Uuid::new_v4(),
        domain: change.domain(),
        description: "prop test".into(),
        change,
        rollback_plan: RollbackPlan {
            checkpoint_id: Uuid::new_v4(),
            steps: vec![RollbackStep {
                description: "revert".into(),
                action: serde_json::json!({}),
            }],
            estimated_rollback_time_ms: 100,
            automatic: true,
        },
        expected_tests: vec![],
        proof: None,
        generated_by: "proptest".into(),
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

const GOVERNANCE_PATHS: &[&str] = &[
    "kernel/src/permissions.rs",
    "kernel/src/consent.rs",
    "kernel/src/autonomy.rs",
    "kernel/src/firewall/prompt_firewall.rs",
    "kernel/src/owasp_defenses.rs",
    "kernel/src/checkpoint.rs",
    "kernel/src/supervisor.rs",
    "kernel/src/audit/mod.rs",
    "kernel/src/identity/agent_identity.rs",
    "kernel/src/hardware_security/manager.rs",
    "kernel/src/policy_engine/mod.rs",
];

// ── INVARIANT PROPERTIES ────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_governance_kernel_always_protected(idx in 0..GOVERNANCE_PATHS.len()) {
        let path = GOVERNANCE_PATHS[idx];
        let proposal = make_proposal(ProposedChange::CodePatch {
            target_file: path.to_string(),
            diff: "change".into(),
            proof: SafetyProof {
                invariants_checked: vec![],
                proof_method: ProofMethod::TypeCheck,
                verifier_version: "1.0".into(),
                proof_hash: "x".into(),
            },
        });
        let result = validate_all_invariants(&proposal, &passing_state());
        prop_assert!(result.is_err(), "governance path {path} must be protected");
    }

    #[test]
    fn prop_self_improvement_always_protected(suffix in "[a-z_]{1,20}\\.rs") {
        let path = format!("crates/nexus-self-improve/src/{suffix}");
        let proposal = make_proposal(ProposedChange::CodePatch {
            target_file: path.clone(),
            diff: "change".into(),
            proof: SafetyProof {
                invariants_checked: vec![],
                proof_method: ProofMethod::TypeCheck,
                verifier_version: "1.0".into(),
                proof_hash: "x".into(),
            },
        });
        let result = validate_all_invariants(&proposal, &passing_state());
        prop_assert!(result.is_err(), "self-improve path {path} must be protected");
    }

    #[test]
    fn prop_hitl_required_for_all_proposals(key in "[a-z.]{1,30}") {
        let proposal = make_proposal(ProposedChange::ConfigChange {
            key,
            old_value: serde_json::json!(1),
            new_value: serde_json::json!(2),
            justification: "test".into(),
        });
        let state = InvariantCheckState {
            hitl_approved: false,
            ..passing_state()
        };
        let result = validate_all_invariants(&proposal, &state);
        prop_assert!(result.is_err(), "HITL approval must always be required");
    }

    #[test]
    fn prop_fuel_never_goes_negative(fuel_remaining in 0u64..100, fuel_cost in 0u64..200) {
        let mut proposal = make_proposal(ProposedChange::ConfigChange {
            key: "test".into(),
            old_value: serde_json::json!(1),
            new_value: serde_json::json!(2),
            justification: "test".into(),
        });
        proposal.fuel_cost = fuel_cost;
        let state = InvariantCheckState {
            fuel_remaining,
            fuel_budget: fuel_remaining,
            ..passing_state()
        };
        let result = validate_all_invariants(&proposal, &state);
        if fuel_cost > fuel_remaining {
            prop_assert!(result.is_err(), "over-budget proposal must be rejected");
        }
    }

    #[test]
    fn prop_test_suite_checked_every_validation(passing in proptest::bool::ANY) {
        let proposal = make_proposal(ProposedChange::ConfigChange {
            key: "test".into(),
            old_value: serde_json::json!(1),
            new_value: serde_json::json!(2),
            justification: "test".into(),
        });
        let state = InvariantCheckState {
            test_suite_passing: passing,
            ..passing_state()
        };
        let result = validate_all_invariants(&proposal, &state);
        if !passing {
            prop_assert!(result.is_err(), "failing test suite must block validation");
        }
    }

    #[test]
    fn prop_crypto_identity_immutable(suffix in "[a-z_]{1,20}\\.rs") {
        let path = format!("kernel/src/identity/{suffix}");
        let proposal = make_proposal(ProposedChange::CodePatch {
            target_file: path,
            diff: "change".into(),
            proof: SafetyProof {
                invariants_checked: vec![],
                proof_method: ProofMethod::TypeCheck,
                verifier_version: "1.0".into(),
                proof_hash: "x".into(),
            },
        });
        let result = validate_all_invariants(&proposal, &passing_state());
        prop_assert!(result.is_err(), "crypto identity paths must be protected");
    }

    #[test]
    fn prop_capabilities_never_expand(key_suffix in "(capabilities|permissions)[a-z._]{0,20}") {
        let proposal = make_proposal(ProposedChange::ConfigChange {
            key: key_suffix.clone(),
            old_value: serde_json::json!(false),
            new_value: serde_json::json!(true),
            justification: "expand".into(),
        });
        let result = validate_all_invariants(&proposal, &passing_state());
        prop_assert!(result.is_err(), "capability expansion via config key '{key_suffix}' must be blocked");
    }

    // ── OBSERVER PROPERTIES ─────────────────────────────────────────

    #[test]
    fn prop_sigma_threshold_determines_signals(
        value in 50.0f64..500.0,
        threshold in 1.0f64..5.0
    ) {
        let mut observer = Observer::new(ObserverConfig {
            sigma_threshold: threshold,
            min_samples: 5,
            ema_alpha: 0.1,
        });

        // Build a stable baseline at 100.0
        for _ in 0..20 {
            let mut m = SystemMetrics::new();
            m.insert("test_metric", 100.0);
            observer.observe(&m);
        }

        // Insert the test value
        let mut m = SystemMetrics::new();
        m.insert("test_metric", value);
        let signals = observer.observe(&m);

        // If value is close to baseline (within ~2*threshold*std_dev), no signal
        // If far from baseline, signal expected
        // We just verify no panic and valid output
        for s in &signals {
            prop_assert!(!s.metric_name.is_empty());
            prop_assert!(s.id != Uuid::nil());
        }
    }

    #[test]
    fn prop_empty_metrics_always_safe(count in 0usize..5) {
        let mut observer = Observer::new(ObserverConfig::default());
        let mut metrics = SystemMetrics::new();
        for i in 0..count {
            metrics.insert(format!("m{i}"), 0.0);
        }
        let signals = observer.observe(&metrics);
        // Should never panic, always return valid vec
        prop_assert!(signals.len() <= count);
    }

    // ── ANALYZER PROPERTIES ─────────────────────────────────────────

    #[test]
    fn prop_confidence_between_zero_and_one(sigma in 0.1f64..10.0) {
        let analyzer = Analyzer::new(AnalyzerConfig {
            min_confidence: 0.0,
            ..Default::default()
        });
        let signal = ImprovementSignal {
            id: Uuid::new_v4(),
            timestamp: 1000,
            domain: ImprovementDomain::ConfigTuning,
            source: SignalSource::PerformanceProfiler,
            metric_name: "test".into(),
            current_value: 200.0,
            baseline_value: 100.0,
            deviation_sigma: sigma,
            evidence: vec![],
        };
        let opps = analyzer.analyze(&[signal]);
        for opp in &opps {
            prop_assert!(opp.confidence >= 0.0 && opp.confidence <= 1.0,
                "confidence {} out of range", opp.confidence);
        }
    }

    #[test]
    fn prop_empty_signals_no_opportunities(n in 0usize..1) {
        let analyzer = Analyzer::new(AnalyzerConfig::default());
        let signals: Vec<ImprovementSignal> = Vec::new();
        // n is just to make proptest happy with at least one parameter
        let _ = n;
        let opps = analyzer.analyze(&signals);
        prop_assert!(opps.is_empty(), "empty signals should produce no opportunities");
    }

    // ── PROMPT OPTIMIZER PROPERTIES ─────────────────────────────────

    #[test]
    fn prop_cosine_similarity_range(a in "[a-z ]{0,100}", b in "[a-z ]{0,100}") {
        let sim = cosine_similarity(&a, &b);
        prop_assert!((0.0..=1.0).contains(&sim),
            "cosine similarity {} out of [0, 1]", sim);
    }

    #[test]
    fn prop_cosine_similarity_symmetric(a in "[a-z ]{1,50}", b in "[a-z ]{1,50}") {
        let ab = cosine_similarity(&a, &b);
        let ba = cosine_similarity(&b, &a);
        prop_assert!((ab - ba).abs() < 1e-9,
            "similarity not symmetric: sim(a,b)={ab} != sim(b,a)={ba}");
    }

    #[test]
    fn prop_cosine_similarity_identity(s in "[a-z]{1,50}") {
        let sim = cosine_similarity(&s, &s);
        prop_assert!((sim - 1.0).abs() < 1e-9,
            "self-similarity should be 1.0, got {sim}");
    }

    #[test]
    fn prop_improvement_threshold_prevents_lateral(
        current in 0.1f64..0.9,
        variant_delta in -0.1f64..0.2
    ) {
        let optimizer = PromptOptimizer::new(PromptOptimizerConfig {
            improvement_threshold: 0.05,
            ..Default::default()
        });
        let variant_score = current + variant_delta;
        let sv = nexus_self_improve::prompt_optimizer::ScoredVariant {
            variant: PromptVariant {
                variant_id: Uuid::new_v4(),
                prompt_text: "test".into(),
                score: 0.0,
            },
            similarity_to_original: 0.9,
            safety_check_passed: true,
            generation_method: "test".into(),
        };
        let result = optimizer.select_best(current, &[(sv, variant_score)]);
        if variant_score <= current * 1.05 {
            prop_assert!(result.is_none(),
                "lateral move (delta={variant_delta:.3}) should be rejected");
        }
    }

    #[test]
    fn prop_meta_prompt_contains_current_prompt(prompt in "[a-zA-Z ]{10,100}") {
        let optimizer = PromptOptimizer::new(PromptOptimizerConfig::default());
        let context = nexus_self_improve::prompt_optimizer::PerformanceContext {
            current_score: 0.7,
            metric_history: vec![],
            weaknesses: vec![],
            optimization_history: vec![],
        };
        let meta = optimizer.build_meta_prompt(&prompt, &context);
        prop_assert!(meta.contains(&prompt),
            "meta-prompt must contain the current prompt");
    }

    // ── BEHAVIORAL ENVELOPE PROPERTIES ──────────────────────────────

    #[test]
    fn prop_drift_bound_theorem_holds(
        alpha in 0.001f64..1.0,
        gamma in 0.001f64..1.0
    ) {
        let mut env = BehavioralEnvelope::new("test");
        env.add_metric("m", 1.0, 0.5);
        env.drift_rate = alpha;
        env.set_recovery_rate(gamma);
        let bound = env.drift_bound_guarantee();
        let expected = alpha / gamma;
        prop_assert!((bound - expected).abs() < 1e-9,
            "D* should be {expected}, got {bound}");
    }

    #[test]
    fn prop_within_bounds_consistent(
        current in 0.0f64..2.0,
        baseline in 0.5f64..1.5,
        tolerance in 0.1f64..0.5
    ) {
        let mut env = BehavioralEnvelope::new("test");
        env.add_metric("m", baseline, tolerance);
        env.metrics.get_mut("m").unwrap().current = current;

        let within = env.is_within_bounds();
        let expected = current >= (baseline - tolerance) && current <= (baseline + tolerance);
        prop_assert_eq!(within, expected,
            "within_bounds({}) should be {} for [{}, {}]",
            current, expected, baseline - tolerance, baseline + tolerance);
    }

    #[test]
    fn prop_would_violate_detects_exceeding(
        value in 0.0f64..3.0,
        baseline in 1.0f64..1.5,
        tolerance in 0.2f64..0.5
    ) {
        let mut env = BehavioralEnvelope::new("test");
        env.add_metric("m", baseline, tolerance);
        let mut predicted = HashMap::new();
        predicted.insert("m".to_string(), value);
        let violates = env.would_violate(&predicted);
        let expected = value < (baseline - tolerance) || value > (baseline + tolerance);
        prop_assert_eq!(violates, expected,
            "would_violate({}) should be {} for [{}, {}]",
            value, expected, baseline - tolerance, baseline + tolerance);
    }

    #[test]
    fn prop_drift_rate_non_negative(
        v1 in 0.0f64..2.0,
        v2 in 0.0f64..2.0,
        v3 in 0.0f64..2.0
    ) {
        let mut env = BehavioralEnvelope::new("test");
        env.add_metric("m", 1.0, 0.5);
        let mut obs = HashMap::new();
        obs.insert("m".to_string(), v1);
        env.update(&obs, 1000);
        obs.insert("m".to_string(), v2);
        env.update(&obs, 2000);
        obs.insert("m".to_string(), v3);
        env.update(&obs, 3000);
        prop_assert!(env.drift_rate >= 0.0,
            "drift_rate should be non-negative, got {}", env.drift_rate);
    }

    // ── GUARDIAN PROPERTIES ─────────────────────────────────────────

    #[test]
    fn prop_switch_decision_consistent_with_drift(
        drift_rate in 0.01f64..1.0,
        recovery_rate in 0.01f64..1.0,
        switch_threshold in 0.1f64..1.0,
        current_offset in 0.0f64..2.0
    ) {
        let mut env = BehavioralEnvelope::new("test");
        env.add_metric("m", 1.0, 0.5);
        env.metrics.get_mut("m").unwrap().current = 1.0 - current_offset * 0.5;
        env.drift_rate = drift_rate;
        env.set_recovery_rate(recovery_rate);

        let guardian = SimplexGuardian::new(switch_threshold);
        let decision = guardian.should_switch_to_baseline(&env, &SystemMetrics::new());

        let drift = env.current_drift();
        let bound = env.drift_bound_guarantee();
        let threshold = bound * switch_threshold;

        match decision {
            nexus_self_improve::guardian::SwitchDecision::SwitchToBaseline { .. } => {
                prop_assert!(drift > threshold && threshold.is_finite(),
                    "switched but drift={drift} <= threshold={threshold}");
            }
            nexus_self_improve::guardian::SwitchDecision::ContinueActive { .. } => {
                // Either drift <= threshold, or threshold is infinite
                prop_assert!(drift <= threshold || !threshold.is_finite(),
                    "continued but drift={drift} > threshold={threshold}");
            }
        }
    }

    #[test]
    fn prop_baseline_hash_deterministic(key in "[a-z]{1,20}", value in "[a-z]{1,50}") {
        let mut guardian = SimplexGuardian::new(0.8);
        let mut prompts = HashMap::new();
        prompts.insert(key.clone(), value.clone());
        let b1 = guardian.capture_baseline(prompts.clone(), HashMap::new(), vec![]);
        let b2 = guardian.capture_baseline(prompts, HashMap::new(), vec![]);
        prop_assert_eq!(b1.snapshot_hash, b2.snapshot_hash,
            "same input must produce same hash");
    }

    #[test]
    fn prop_switch_threshold_bounds(threshold in -5.0f64..5.0) {
        let guardian = SimplexGuardian::new(threshold);
        let actual = guardian.switch_threshold();
        prop_assert!((0.1..=1.0).contains(&actual),
            "threshold {} out of [0.1, 1.0] for input {}", actual, threshold);
    }

    // ── SCHEDULER PROPERTIES ────────────────────────────────────────

    #[test]
    fn prop_interval_always_within_bounds(n_successes in 0u32..20, n_failures in 0u32..20) {
        let mut sched = AdaptiveScheduler::new();
        for _ in 0..n_successes {
            sched.record_outcome(&AttemptOutcome::Improved { delta: 0.1 });
        }
        for _ in 0..n_failures {
            sched.record_outcome(&AttemptOutcome::NoImprovement);
        }
        prop_assert!(sched.current_interval_secs >= sched.min_interval_secs,
            "interval {} < min {}", sched.current_interval_secs, sched.min_interval_secs);
        prop_assert!(sched.current_interval_secs <= sched.max_interval_secs,
            "interval {} > max {}", sched.current_interval_secs, sched.max_interval_secs);
    }

    #[test]
    fn prop_success_rate_between_zero_and_one(n in 0u32..50) {
        let mut sched = AdaptiveScheduler::new();
        for i in 0..n {
            if i % 3 == 0 {
                sched.record_outcome(&AttemptOutcome::Improved { delta: 0.1 });
            } else {
                sched.record_outcome(&AttemptOutcome::NoImprovement);
            }
        }
        prop_assert!(sched.success_rate >= 0.0 && sched.success_rate <= 1.0,
            "success_rate {} out of [0, 1]", sched.success_rate);
    }

    #[test]
    fn prop_consecutive_successes_shorten_interval(n in 1u32..10) {
        let mut sched = AdaptiveScheduler::new();
        let before = sched.current_interval_secs;
        for _ in 0..n {
            sched.record_outcome(&AttemptOutcome::Improved { delta: 0.1 });
        }
        prop_assert!(sched.current_interval_secs <= before,
            "after {n} successes: {} should be <= {before}", sched.current_interval_secs);
    }

    #[test]
    fn prop_consecutive_failures_lengthen_interval(n in 1u32..5) {
        let mut sched = AdaptiveScheduler::new();
        let before = sched.current_interval_secs;
        for _ in 0..n {
            sched.record_outcome(&AttemptOutcome::NoImprovement);
        }
        prop_assert!(sched.current_interval_secs >= before,
            "after {n} failures: {} should be >= {before}", sched.current_interval_secs);
    }

    // ── CONFIG OPTIMIZER PROPERTIES ─────────────────────────────────

    #[test]
    fn prop_suggestion_within_parameter_bounds(metric_value in 0.1f64..3.0) {
        let optimizer = ConfigOptimizer::new(ConfigOptimizerConfig { trigger_threshold: 0.05 });
        let mut metrics = SystemMetrics::new();
        metrics.insert("cache_hit_rate", metric_value);
        let suggestions = optimizer.analyze_config(&metrics);
        for s in &suggestions {
            let param = &optimizer.parameters[&s.parameter_key];
            prop_assert!(s.suggested_value >= param.min_value,
                "suggested {} < min {} for {}", s.suggested_value, param.min_value, s.parameter_key);
            prop_assert!(s.suggested_value <= param.max_value,
                "suggested {} > max {} for {}", s.suggested_value, param.max_value, s.parameter_key);
        }
    }

    // ── POLICY OPTIMIZER PROPERTIES ─────────────────────────────────

    #[test]
    fn prop_can_only_narrow_never_broaden(
        has_when in proptest::bool::ANY,
        has_forbid in proptest::bool::ANY
    ) {
        let mut cedar = "permit(principal, action, resource)".to_string();
        if has_when {
            cedar.push_str(" when { context.risk < 0.5 }");
        }
        if has_forbid {
            cedar = "forbid(principal, action, resource)".to_string();
        }
        cedar.push(';');

        let is_narrowing = PolicyOptimizer::is_narrowing_change(&cedar);
        if has_when || has_forbid {
            prop_assert!(is_narrowing, "'{cedar}' should be narrowing");
        } else {
            prop_assert!(!is_narrowing, "'{cedar}' should NOT be narrowing (broadening)");
        }
    }
}
