use nexus_kernel::errors::AgentError;
use nexus_kernel::kill_gates::GateStatus;
use nexus_kernel::lifecycle::AgentState;
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::safety_supervisor::KpiKind;
use nexus_kernel::supervisor::Supervisor;

fn sample_manifest(fuel_budget: u64) -> AgentManifest {
    AgentManifest {
        name: "phase7-agent".to_string(),
        version: "0.1.0".to_string(),
        capabilities: vec!["llm.query".to_string()],
        fuel_budget,
        autonomy_level: None,
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        llm_model: Some("mock-1".to_string()),
        fuel_period_id: None,
        monthly_fuel_cap: None,
        allowed_endpoints: None,
        domain_tags: vec![],
        filesystem_permissions: vec![],
    }
}

#[test]
fn test_screen_poster_freeze_from_kpi() {
    let mut supervisor = Supervisor::new();
    let agent_id = supervisor
        .start_agent(sample_manifest(100))
        .expect("agent should start");

    let result = supervisor.record_subsystem_metric(agent_id, KpiKind::BanRate, 3.0);
    assert!(result.is_ok());
    assert_eq!(
        supervisor.subsystem_gate_status("screen_poster"),
        Some(GateStatus::Frozen)
    );
}

#[test]
fn test_unfreeze_requires_tier3() {
    let mut supervisor = Supervisor::new();
    let agent_id = supervisor
        .start_agent(sample_manifest(100))
        .expect("agent should start");

    supervisor
        .manual_freeze_subsystem(agent_id, "cluster", "operator-1")
        .expect("manual freeze should succeed");

    let denied = supervisor.manual_unfreeze_subsystem(agent_id, "cluster", "operator-1", 2);
    assert!(denied.is_err());

    let allowed = supervisor.manual_unfreeze_subsystem(agent_id, "cluster", "operator-1", 3);
    assert!(allowed.is_ok());
    assert_eq!(
        supervisor.subsystem_gate_status("cluster"),
        Some(GateStatus::Open)
    );
}

#[test]
fn test_manual_override_halts_agent_immediately() {
    let mut supervisor = Supervisor::new();
    let agent_id = supervisor
        .start_agent(sample_manifest(100))
        .expect("agent should start");

    let halted = supervisor.manual_halt_agent(agent_id, "operator-1", "emergency stop");
    assert!(matches!(halted, Err(AgentError::SupervisorError(_))));

    let state = supervisor
        .get_agent(agent_id)
        .expect("agent should still exist")
        .state;
    assert_eq!(state, AgentState::Stopped);
}
