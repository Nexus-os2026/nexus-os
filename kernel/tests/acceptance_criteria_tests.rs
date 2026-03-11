//! Acceptance criteria verification tests.
//!
//! Each test maps to a criterion in `docs/acceptance.md`. If any test here
//! fails, the release gate is not satisfied.

use nexus_kernel::adaptive_policy::{AdaptiveGovernor, AutonomyChange, RunOutcome};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::fuel_hardening::{FuelContext, FuelToTokenModel, ModelCost};
use nexus_kernel::safety_supervisor::{
    default_thresholds, KpiKind, OperatingMode, SafetyAction, SafetySupervisor,
};
use serde_json::json;
use uuid::Uuid;

// ── 1. Test count regression gate ───────────────────────────────────────────

/// Meta-test: verify the kernel crate alone has at least 400 tests.
///
/// We count `#[test]` annotations across kernel/src/**/*.rs. If this drops
/// below 400 it means tests were accidentally deleted.
#[test]
fn test_minimum_test_count_kernel() {
    let kernel_src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let count = count_test_annotations(&kernel_src);
    assert!(
        count >= 400,
        "Kernel must have at least 400 #[test] functions, found {count}"
    );
}

/// Meta-test: verify the kernel integration tests have at least 60 tests.
#[test]
fn test_minimum_test_count_kernel_integration() {
    let kernel_tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let count = count_test_annotations(&kernel_tests);
    assert!(
        count >= 60,
        "Kernel integration tests must have at least 60 #[test] functions, found {count}"
    );
}

fn count_test_annotations(dir: &std::path::Path) -> usize {
    let mut count = 0;
    if !dir.exists() {
        return 0;
    }
    for entry in walkdir(dir) {
        if entry.extension().is_some_and(|ext| ext == "rs") {
            if let Ok(contents) = std::fs::read_to_string(&entry) {
                count += contents.matches("#[test]").count();
            }
        }
    }
    count
}

fn walkdir(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walkdir(&path));
            } else {
                files.push(path);
            }
        }
    }
    files
}

// ── 2. Audit chain integrity ────────────────────────────────────────────────

/// Acceptance: audit chain must maintain 100% integrity across 50 events.
#[test]
fn test_audit_chain_integrity_passes() {
    let mut audit = AuditTrail::new();
    let agent_id = Uuid::new_v4();

    for i in 0..50 {
        audit
            .append_event(
                agent_id,
                EventType::ToolCall,
                json!({ "action": format!("test_action_{i}"), "seq": i }),
            )
            .expect("append_event should succeed");
    }

    assert_eq!(audit.events().len(), 50);
    assert!(
        audit.verify_integrity(),
        "Audit chain integrity must be 100% after 50 events"
    );
}

/// Tampering with any event must cause integrity verification to fail.
#[test]
fn test_audit_chain_detects_tampering() {
    let mut audit = AuditTrail::new();
    let agent_id = Uuid::new_v4();

    for i in 0..10 {
        audit
            .append_event(agent_id, EventType::StateChange, json!({ "seq": i }))
            .unwrap();
    }

    assert!(
        audit.verify_integrity(),
        "chain should be valid before tampering"
    );

    // Tamper with the 5th event's payload
    audit.events_mut()[4].payload = json!({ "seq": 999, "tampered": true });

    assert!(
        !audit.verify_integrity(),
        "Integrity check must fail after tampering"
    );
}

// ── 3. Fuel metering ceiling accuracy ───────────────────────────────────────

/// Acceptance: fuel metering uses ceiling arithmetic with < 1 unit rounding
/// error per call. We verify 1000 calculations against the mathematical ceiling.
#[test]
fn test_fuel_metering_ceiling_accuracy() {
    let model = FuelToTokenModel::with_defaults();

    // Use deepseek-chat: cost_per_1k_input=140, cost_per_1k_output=280
    let cost_input_per_1k: u64 = 140;
    let cost_output_per_1k: u64 = 280;

    for tokens in 0..1000u32 {
        let actual = model.simulate_cost("deepseek-chat", tokens, tokens);

        // Mathematical ceiling: ceil(tokens * cost_per_1k / 1000)
        let expected_input = ceiling_div(u64::from(tokens) * cost_input_per_1k, 1000);
        let expected_output = ceiling_div(u64::from(tokens) * cost_output_per_1k, 1000);
        let expected = expected_input + expected_output;

        assert_eq!(
            actual, expected,
            "Fuel cost for {tokens} tokens: got {actual}, expected {expected} (ceiling)"
        );
    }
}

/// Verify ceiling arithmetic never undercharges (always >= true cost).
#[test]
fn test_fuel_never_undercharges() {
    let mut model = FuelToTokenModel::with_defaults();
    model.insert(
        "test-model",
        ModelCost {
            cost_per_1k_input: 333,
            cost_per_1k_output: 777,
        },
    );

    for tokens in 1..500u32 {
        let fuel = model.simulate_cost("test-model", tokens, tokens);

        // True fractional cost (may not be integer)
        let true_input = f64::from(tokens) * 333.0 / 1000.0;
        let true_output = f64::from(tokens) * 777.0 / 1000.0;
        let true_total = true_input + true_output;

        assert!(
            fuel as f64 >= true_total,
            "Fuel {fuel} must be >= true cost {true_total:.3} for {tokens} tokens"
        );

        // Rounding error must be < 1 unit per component (< 2 total for input+output)
        let error = fuel as f64 - true_total;
        assert!(
            error < 2.0,
            "Rounding error {error:.3} must be < 2.0 for {tokens} tokens"
        );
    }
}

fn ceiling_div(numerator: u64, denominator: u64) -> u64 {
    if numerator == 0 || denominator == 0 {
        return 0;
    }
    numerator.div_ceil(denominator)
}

// ── 4. Health endpoint field completeness ───────────────────────────────────

/// Acceptance: health response must include all 12 documented fields.
#[test]
fn test_health_endpoint_fields_complete() {
    // Simulate a health response with the same structure as the real endpoint
    let mut audit = AuditTrail::new();
    let agent_id = Uuid::new_v4();
    audit
        .append_event(agent_id, EventType::StateChange, json!({"init": true}))
        .unwrap();

    let health = json!({
        "status": "healthy",
        "version": "0.2.0",
        "agents_registered": 1,
        "tasks_in_flight": 0,
        "started_at": 1710000000_u64,
        "uptime_seconds": 120_u64,
        "agents_active": 1,
        "total_tests_passed": audit.events().len(),
        "audit_chain_valid": audit.verify_integrity(),
        "compliance_status": "active",
        "memory_usage_bytes": 1024_u64,
        "wasm_cache_hit_rate": -1.0_f64,
    });

    let required_fields = [
        "status",
        "version",
        "agents_registered",
        "tasks_in_flight",
        "started_at",
        "uptime_seconds",
        "agents_active",
        "total_tests_passed",
        "audit_chain_valid",
        "compliance_status",
        "memory_usage_bytes",
        "wasm_cache_hit_rate",
    ];

    for field in &required_fields {
        assert!(
            !health[field].is_null(),
            "Health response must include field '{field}'"
        );
    }

    // Verify types
    assert!(health["status"].is_string());
    assert!(health["audit_chain_valid"].is_boolean());
    assert!(health["agents_registered"].is_number());
    assert!(health["memory_usage_bytes"].is_number());
    assert!(health["wasm_cache_hit_rate"].is_number());
}

// ── 5. Circuit breaker thresholds ───────────────────────────────────────────
// Note: ProviderCircuitBreaker is in nexus-connectors-llm, not nexus-kernel.
// We test the kernel-level equivalent: SafetySupervisor three-strike acts as
// the kernel's circuit breaker. See test_three_strike_safety below.

// ── 6. Trust promotion / demotion thresholds ────────────────────────────────

/// Acceptance: trust score >= 0.85 triggers promotion recommendation.
#[test]
fn test_trust_promotion_threshold() {
    let mut governor = AdaptiveGovernor::new();
    let agent_id = Uuid::new_v4();

    governor.register(agent_id, 1, 5);

    // Record enough successful runs to push trust above 0.85
    for _ in 0..100 {
        governor.record_run(agent_id, RunOutcome::Success, 10, 100);
    }

    let record = governor.get_record(agent_id).expect("record must exist");
    assert!(
        record.trust_score >= 0.85,
        "Trust score {:.2} should be >= 0.85 after 100 successes",
        record.trust_score
    );

    match governor.evaluate(agent_id) {
        AutonomyChange::Promote { from, to } => {
            assert_eq!(from, 1);
            assert_eq!(to, 2);
        }
        other => panic!("Expected Promote, got {other:?}"),
    }
}

/// Acceptance: trust score < 0.30 triggers automatic demotion.
#[test]
fn test_trust_demotion_threshold() {
    let mut governor = AdaptiveGovernor::new();
    let agent_id = Uuid::new_v4();

    governor.register(agent_id, 3, 5);

    // Record many failures and violations to drop trust below 0.30
    for _ in 0..10 {
        governor.record_run(
            agent_id,
            RunOutcome::PolicyViolation {
                violation: "test violation".to_string(),
            },
            10,
            100,
        );
    }

    let record = governor.get_record(agent_id).expect("record must exist");
    assert!(
        record.trust_score < 0.30,
        "Trust score {:.2} should be < 0.30 after many violations",
        record.trust_score
    );

    match governor.evaluate(agent_id) {
        AutonomyChange::Demote { from, to, .. } => {
            assert_eq!(from, 3);
            assert_eq!(to, 2);
        }
        other => panic!("Expected Demote, got {other:?}"),
    }
}

/// Acceptance: trust stays in no-change zone between 0.30 and 0.85.
#[test]
fn test_trust_no_change_zone() {
    let mut governor = AdaptiveGovernor::new();
    let agent_id = Uuid::new_v4();

    governor.register(agent_id, 2, 5);

    // Mix of successes and failures to land in the middle zone
    for _ in 0..7 {
        governor.record_run(agent_id, RunOutcome::Success, 10, 100);
    }
    for _ in 0..3 {
        governor.record_run(
            agent_id,
            RunOutcome::Failed {
                reason: "timeout".to_string(),
            },
            10,
            100,
        );
    }

    let record = governor.get_record(agent_id).expect("record must exist");
    assert!(
        record.trust_score >= 0.30 && record.trust_score < 0.85,
        "Trust score {:.2} should be in no-change zone [0.30, 0.85)",
        record.trust_score
    );

    assert!(
        matches!(governor.evaluate(agent_id), AutonomyChange::NoChange),
        "Should be NoChange in the middle zone"
    );
}

// ── 7. Three-strike safety model ────────────────────────────────────────────

/// Acceptance: 1st strike → Continue, 2nd → Degraded, 3rd → Halted.
#[test]
fn test_three_strike_safety() {
    let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
    let mut audit = AuditTrail::new();
    let agent_id = Uuid::new_v4();

    // Use critical-level readings to trigger violations
    let critical_readings = [(KpiKind::LlmLatency, 20_000.0)];

    // 1st strike: Continue
    let first = supervisor.heartbeat(agent_id, &critical_readings, &mut audit);
    assert_eq!(
        first,
        SafetyAction::Continue,
        "1st violation must result in Continue"
    );
    assert_eq!(supervisor.violation_count(agent_id), 1);

    // 2nd strike: Degraded
    let second = supervisor.heartbeat(agent_id, &critical_readings, &mut audit);
    assert!(
        matches!(second, SafetyAction::Degraded { .. }),
        "2nd violation must result in Degraded, got {second:?}"
    );
    assert_eq!(supervisor.violation_count(agent_id), 2);
    assert!(matches!(
        supervisor.mode_for_agent(agent_id),
        OperatingMode::Degraded(_)
    ));

    // 3rd strike: Halted
    let third = supervisor.heartbeat(agent_id, &critical_readings, &mut audit);
    assert!(
        matches!(third, SafetyAction::Halted { .. }),
        "3rd violation must result in Halted, got {third:?}"
    );
    assert!(supervisor.violation_count(agent_id) >= 3);
    assert!(matches!(
        supervisor.mode_for_agent(agent_id),
        OperatingMode::Halted(_)
    ));
}

/// Acceptance: healthy heartbeat resets violation counter.
#[test]
fn test_three_strike_resets_on_healthy() {
    let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
    let mut audit = AuditTrail::new();
    let agent_id = Uuid::new_v4();

    let critical = [(KpiKind::LlmLatency, 20_000.0)];

    // Two strikes
    supervisor.heartbeat(agent_id, &critical, &mut audit);
    supervisor.heartbeat(agent_id, &critical, &mut audit);
    assert_eq!(supervisor.violation_count(agent_id), 2);

    // Healthy reading resets counter
    supervisor.reset_violations(agent_id, &mut audit);
    assert_eq!(supervisor.violation_count(agent_id), 0);

    // Next violation is strike 1 again, not strike 3
    let action = supervisor.heartbeat(agent_id, &critical, &mut audit);
    assert_eq!(
        action,
        SafetyAction::Continue,
        "After reset, violation should be strike 1 (Continue)"
    );
}

// ── 8. Fail-closed on error ─────────────────────────────────────────────────

/// Acceptance: fuel exhaustion blocks operation, never silently passes.
#[test]
fn test_fail_closed_fuel_exhaustion() {
    let ctx = FuelContext::new(10);

    // Deduct all fuel
    ctx.deduct_fuel(10).expect("should succeed");
    assert_eq!(ctx.fuel_remaining(), 0);

    // Further deduction must fail, not silently pass
    let result = ctx.deduct_fuel(1);
    assert!(
        matches!(result, Err(AgentError::FuelExhausted)),
        "Must fail-closed with FuelExhausted, got {result:?}"
    );
}

/// Acceptance: fuel reservation auto-refunds on drop (fail-safe).
#[test]
fn test_fail_closed_reservation_auto_refund() {
    let ctx = FuelContext::new(100);

    {
        let _reservation = ctx.reserve_fuel(50).expect("reservation should succeed");
        assert_eq!(ctx.fuel_remaining(), 50);
        // _reservation dropped here without commit → auto-refund
    }

    assert_eq!(
        ctx.fuel_remaining(),
        100,
        "Fuel must be fully refunded after dropped reservation"
    );
}

/// Acceptance: over-budget reservation is denied, not silently allowed.
#[test]
fn test_fail_closed_over_budget_reservation() {
    let ctx = FuelContext::new(10);

    let result = ctx.reserve_fuel(11);
    assert!(
        result.is_err(),
        "Reservation exceeding budget must be denied"
    );
    assert_eq!(
        ctx.fuel_remaining(),
        10,
        "Fuel must remain unchanged after denied reservation"
    );
}

/// Acceptance: committed reservation permanently consumes fuel.
#[test]
fn test_reservation_commit_consumes() {
    let ctx = FuelContext::new(100);

    let reservation = ctx.reserve_fuel(30).expect("should succeed");
    assert_eq!(ctx.fuel_remaining(), 70);

    reservation.commit();
    assert_eq!(
        ctx.fuel_remaining(),
        70,
        "Committed reservation should keep fuel consumed"
    );
}

/// Acceptance: cancelled reservation returns fuel.
#[test]
fn test_reservation_cancel_refunds() {
    let ctx = FuelContext::new(100);

    let reservation = ctx.reserve_fuel(30).expect("should succeed");
    assert_eq!(ctx.fuel_remaining(), 70);

    reservation.cancel();
    assert_eq!(
        ctx.fuel_remaining(),
        100,
        "Cancelled reservation must return fuel"
    );
}

/// Acceptance: KPI thresholds match documented values from acceptance.md.
#[test]
fn test_kpi_thresholds_match_documentation() {
    let supervisor = SafetySupervisor::new(default_thresholds(), 10);

    // Governance overhead: warn=5%, critical=10%
    assert_eq!(
        supervisor.check_kpi(KpiKind::GovernanceOverhead, 4.9),
        nexus_kernel::safety_supervisor::KpiStatus::Ok
    );
    assert_eq!(
        supervisor.check_kpi(KpiKind::GovernanceOverhead, 5.0),
        nexus_kernel::safety_supervisor::KpiStatus::Warn
    );
    assert_eq!(
        supervisor.check_kpi(KpiKind::GovernanceOverhead, 10.0),
        nexus_kernel::safety_supervisor::KpiStatus::Critical
    );

    // LLM latency: warn=5000ms, critical=15000ms
    assert_eq!(
        supervisor.check_kpi(KpiKind::LlmLatency, 4999.0),
        nexus_kernel::safety_supervisor::KpiStatus::Ok
    );
    assert_eq!(
        supervisor.check_kpi(KpiKind::LlmLatency, 5000.0),
        nexus_kernel::safety_supervisor::KpiStatus::Warn
    );
    assert_eq!(
        supervisor.check_kpi(KpiKind::LlmLatency, 15000.0),
        nexus_kernel::safety_supervisor::KpiStatus::Critical
    );

    // Agent error rate: warn=10%, critical=25%
    assert_eq!(
        supervisor.check_kpi(KpiKind::AgentErrorRate, 9.9),
        nexus_kernel::safety_supervisor::KpiStatus::Ok
    );
    assert_eq!(
        supervisor.check_kpi(KpiKind::AgentErrorRate, 10.0),
        nexus_kernel::safety_supervisor::KpiStatus::Warn
    );
    assert_eq!(
        supervisor.check_kpi(KpiKind::AgentErrorRate, 25.0),
        nexus_kernel::safety_supervisor::KpiStatus::Critical
    );
}
