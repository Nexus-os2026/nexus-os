use nexus_kernel::audit::AuditTrail;
use nexus_kernel::safety_supervisor::{
    default_thresholds, KpiKind, OperatingMode, SafetyAction, SafetySupervisor,
};

#[test]
fn test_kpi_normal() {
    let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
    let mut audit = AuditTrail::new();
    let agent_id = uuid::Uuid::new_v4();

    let readings = [
        (KpiKind::GovernanceOverhead, 2.0),
        (KpiKind::LlmLatency, 200.0),
        (KpiKind::AuditChainIntegrity, 0.0),
        (KpiKind::FuelBurnRate, 30.0),
        (KpiKind::AgentErrorRate, 1.0),
        (KpiKind::BudgetCompliance, 40.0),
    ];

    let action = supervisor.heartbeat(agent_id, &readings, &mut audit);
    assert_eq!(action, SafetyAction::Continue);
    assert_eq!(supervisor.mode_for_agent(agent_id), OperatingMode::Normal);
}

#[test]
fn test_kpi_degraded() {
    let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
    let mut audit = AuditTrail::new();
    let agent_id = uuid::Uuid::new_v4();
    let warning_readings = [(KpiKind::GovernanceOverhead, 6.0)];

    let first = supervisor.heartbeat(agent_id, &warning_readings, &mut audit);
    let second = supervisor.heartbeat(agent_id, &warning_readings, &mut audit);

    assert_eq!(first, SafetyAction::Continue);
    assert!(matches!(second, SafetyAction::Degraded { .. }));
    assert!(matches!(
        supervisor.mode_for_agent(agent_id),
        OperatingMode::Degraded(_)
    ));
}

#[test]
fn test_3_strike_halt() {
    let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
    let mut audit = AuditTrail::new();
    let agent_id = uuid::Uuid::new_v4();
    let critical_readings = [(KpiKind::LlmLatency, 20_000.0)];

    let _ = supervisor.heartbeat(agent_id, &critical_readings, &mut audit);
    let _ = supervisor.heartbeat(agent_id, &critical_readings, &mut audit);
    let third = supervisor.heartbeat(agent_id, &critical_readings, &mut audit);

    assert!(matches!(third, SafetyAction::Halted { .. }));
    assert!(matches!(
        supervisor.mode_for_agent(agent_id),
        OperatingMode::Halted(_)
    ));
}

#[test]
fn test_reset_on_success() {
    let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
    let mut audit = AuditTrail::new();
    let agent_id = uuid::Uuid::new_v4();
    let warning_readings = [(KpiKind::GovernanceOverhead, 6.0)];
    let success_readings = [(KpiKind::GovernanceOverhead, 1.0)];

    let _ = supervisor.heartbeat(agent_id, &warning_readings, &mut audit);
    let _ = supervisor.heartbeat(agent_id, &warning_readings, &mut audit);
    let action = supervisor.heartbeat(agent_id, &success_readings, &mut audit);

    assert_eq!(action, SafetyAction::Continue);
    assert_eq!(supervisor.violation_count(agent_id), 0);
    assert_eq!(supervisor.mode_for_agent(agent_id), OperatingMode::Normal);
}

#[test]
fn test_incident_report_structure() {
    let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
    let mut audit = AuditTrail::new();
    let agent_id = uuid::Uuid::new_v4();
    let critical_readings = [(KpiKind::LlmLatency, 20_000.0)];

    let _ = supervisor.heartbeat(agent_id, &critical_readings, &mut audit);
    let _ = supervisor.heartbeat(agent_id, &critical_readings, &mut audit);
    let _ = supervisor.heartbeat(agent_id, &critical_readings, &mut audit);

    let report = supervisor
        .last_incident_report(agent_id)
        .expect("incident report should be generated");
    assert_eq!(report.agent_id, agent_id.to_string());
    assert!(!report.kpi_violations.is_empty());
    assert!(!report.audit_trail_excerpt.is_empty());
    assert!(!report.signature.is_empty());
}
