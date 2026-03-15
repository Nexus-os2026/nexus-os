use nexus_kernel::fuel_hardening::{
    AgentFuelLedger, BudgetPeriodId, BurnAnomalyDetector, FuelViolation,
};
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::supervisor::Supervisor;
use nexus_kernel::{audit::AuditTrail, errors::AgentError};

#[test]
fn phase5_monthly_cap_enforced() {
    let agent_id = uuid::Uuid::new_v4();
    let mut audit = AuditTrail::new();
    // Use a detector with anomaly detection disabled so we test the cap path.
    let mut ledger = AgentFuelLedger::new(
        BudgetPeriodId::new("2026-03"),
        1_000,
        BurnAnomalyDetector::new(0, 0, u64::MAX, 4),
    );

    let result = ledger.record_llm_spend(agent_id, "mock-1", 100, 100, 1_001, &mut audit);
    assert_eq!(result, Err(FuelViolation::OverMonthlyCap));

    assert!(audit.events().iter().any(|event| {
        event
            .payload
            .get("event_kind")
            .and_then(|value| value.as_str())
            == Some("fuel.exhausted_halt")
    }));
}

#[test]
fn phase5_exhaustion_triggers_autonomy_downgrade() {
    let mut supervisor = Supervisor::new();
    let manifest = AgentManifest {
        name: "fuel-agent".to_string(),
        version: "0.1.0".to_string(),
        capabilities: vec!["llm.query".to_string()],
        fuel_budget: 1,
        autonomy_level: Some(3),
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        default_goal: None,
        llm_model: Some("mock-1".to_string()),
        fuel_period_id: Some("2026-03".to_string()),
        monthly_fuel_cap: Some(1),
        allowed_endpoints: None,
        domain_tags: vec![],
        filesystem_permissions: vec![],
    };

    let id = supervisor
        .start_agent(manifest)
        .expect("agent should start with initial fuel");

    let restart = supervisor.restart_agent(id);
    assert!(matches!(
        restart,
        Err(AgentError::FuelViolation {
            violation: FuelViolation::OverMonthlyCap,
            ..
        })
    ));

    let handle = supervisor
        .get_agent(id)
        .expect("agent should remain addressable");
    assert_eq!(handle.autonomy_level, 0);
}
