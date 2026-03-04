use crate::audit::{AuditEvent, AuditTrail, EventType};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

const INCIDENT_EXCERPT_SIZE: usize = 10;
const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

pub type AgentId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KpiKind {
    GovernanceOverhead,
    LlmLatency,
    AuditChainIntegrity,
    FuelBurnRate,
    AgentErrorRate,
    BudgetCompliance,
}

impl KpiKind {
    pub fn as_str(self) -> &'static str {
        match self {
            KpiKind::GovernanceOverhead => "governance_overhead",
            KpiKind::LlmLatency => "llm_latency",
            KpiKind::AuditChainIntegrity => "audit_chain_integrity",
            KpiKind::FuelBurnRate => "fuel_burn_rate",
            KpiKind::AgentErrorRate => "agent_error_rate",
            KpiKind::BudgetCompliance => "budget_compliance",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KpiThreshold {
    pub kind: KpiKind,
    pub warn_value: f64,
    pub critical_value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KpiStatus {
    Ok,
    Warn,
    Critical,
}

impl KpiStatus {
    fn severity(self) -> u8 {
        match self {
            KpiStatus::Ok => 0,
            KpiStatus::Warn => 1,
            KpiStatus::Critical => 2,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            KpiStatus::Ok => "ok",
            KpiStatus::Warn => "warn",
            KpiStatus::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KpiViolation {
    pub kind: KpiKind,
    pub value: f64,
    pub status: KpiStatus,
    pub warn_value: f64,
    pub critical_value: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OperatingMode {
    #[default]
    Normal,
    Degraded(String),
    Halted(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IncidentReport {
    pub report_id: String,
    pub agent_id: String,
    pub timestamp_seq: u64,
    pub kpi_violations: Vec<KpiViolation>,
    pub action_taken: String,
    pub audit_trail_excerpt: Vec<AuditEvent>,
    pub recommendation: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafetyAction {
    Continue,
    Degraded { reason: String },
    Halted { reason: String, report_id: String },
}

#[derive(Debug, Clone)]
pub struct SafetySupervisor {
    pub thresholds: Vec<KpiThreshold>,
    pub violation_counter: HashMap<AgentId, u32>,
    pub mode: OperatingMode,
    agent_modes: HashMap<AgentId, OperatingMode>,
    recent_violations: HashMap<AgentId, Vec<KpiViolation>>,
    incident_reports: HashMap<AgentId, Vec<IncidentReport>>,
    tool_call_heartbeat_every: u32,
    tool_call_counter: HashMap<AgentId, u32>,
}

impl Default for SafetySupervisor {
    fn default() -> Self {
        Self::new(default_thresholds(), 10)
    }
}

impl SafetySupervisor {
    pub fn new(thresholds: Vec<KpiThreshold>, tool_call_heartbeat_every: u32) -> Self {
        Self {
            thresholds,
            violation_counter: HashMap::new(),
            mode: OperatingMode::Normal,
            agent_modes: HashMap::new(),
            recent_violations: HashMap::new(),
            incident_reports: HashMap::new(),
            tool_call_heartbeat_every: tool_call_heartbeat_every.max(1),
            tool_call_counter: HashMap::new(),
        }
    }

    pub fn mode_for_agent(&self, agent_id: AgentId) -> OperatingMode {
        self.agent_modes
            .get(&agent_id)
            .cloned()
            .unwrap_or(OperatingMode::Normal)
    }

    pub fn violation_count(&self, agent_id: AgentId) -> u32 {
        self.violation_counter.get(&agent_id).copied().unwrap_or(0)
    }

    pub fn last_incident_report(&self, agent_id: AgentId) -> Option<&IncidentReport> {
        self.incident_reports
            .get(&agent_id)
            .and_then(|reports| reports.last())
    }

    pub fn check_kpi(&self, kind: KpiKind, value: f64) -> KpiStatus {
        let Some(threshold) = self.threshold_for(kind) else {
            return KpiStatus::Ok;
        };

        if value >= threshold.critical_value {
            return KpiStatus::Critical;
        }
        if value >= threshold.warn_value {
            return KpiStatus::Warn;
        }
        KpiStatus::Ok
    }

    pub fn heartbeat(
        &mut self,
        agent_id: AgentId,
        readings: &[(KpiKind, f64)],
        audit: &mut AuditTrail,
    ) -> SafetyAction {
        if readings.is_empty() {
            self.reset_violations(agent_id, audit);
            return SafetyAction::Continue;
        }

        let mut violations = Vec::new();
        for (kind, value) in readings {
            let status = self.check_kpi(*kind, *value);
            let threshold = self.threshold_for(*kind).cloned();
            let warn_value = threshold.as_ref().map_or(0.0, |row| row.warn_value);
            let critical_value = threshold.as_ref().map_or(0.0, |row| row.critical_value);

            let _ = audit.append_event(
                agent_id,
                EventType::StateChange,
                json!({
                    "event_kind": "safety.kpi_checked",
                    "agent_id": agent_id,
                    "kpi_kind": kind.as_str(),
                    "value": value,
                    "status": status.as_str(),
                    "warn_value": warn_value,
                    "critical_value": critical_value,
                }),
            );

            if status != KpiStatus::Ok {
                violations.push(KpiViolation {
                    kind: *kind,
                    value: *value,
                    status,
                    warn_value,
                    critical_value,
                });
            }
        }

        if violations.is_empty() {
            self.reset_violations(agent_id, audit);
            return SafetyAction::Continue;
        }

        violations.sort_by_key(|violation| violation.status.severity());
        let selected = violations
            .last()
            .cloned()
            .unwrap_or_else(|| violations[0].clone());

        let violations_log = self.recent_violations.entry(agent_id).or_default();
        violations_log.push(selected.clone());
        if violations_log.len() > 32 {
            let _ = violations_log.remove(0);
        }

        self.record_violation(agent_id, selected, audit)
    }

    pub fn observe_tool_call(
        &mut self,
        agent_id: AgentId,
        readings: &[(KpiKind, f64)],
        audit: &mut AuditTrail,
    ) -> SafetyAction {
        let counter = self.tool_call_counter.entry(agent_id).or_insert(0);
        *counter = counter.saturating_add(1);
        if (*counter).is_multiple_of(self.tool_call_heartbeat_every) {
            return self.heartbeat(agent_id, readings, audit);
        }

        SafetyAction::Continue
    }

    pub fn observe_llm_response(
        &mut self,
        agent_id: AgentId,
        latency_ms: u64,
        governance_overhead_pct: f64,
        audit: &mut AuditTrail,
    ) -> SafetyAction {
        let integrity = if audit.verify_integrity() { 0.0 } else { 1.0 };
        let readings = [
            (KpiKind::LlmLatency, latency_ms as f64),
            (KpiKind::GovernanceOverhead, governance_overhead_pct),
            (KpiKind::AuditChainIntegrity, integrity),
        ];
        self.heartbeat(agent_id, &readings, audit)
    }

    pub fn observe_workflow_node_completion(
        &mut self,
        agent_id: AgentId,
        readings: &[(KpiKind, f64)],
        audit: &mut AuditTrail,
    ) -> SafetyAction {
        self.heartbeat(agent_id, readings, audit)
    }

    pub fn reset_violations(&mut self, agent_id: AgentId, audit: &mut AuditTrail) {
        self.violation_counter.insert(agent_id, 0);

        if matches!(
            self.agent_modes.get(&agent_id),
            Some(OperatingMode::Degraded(_))
        ) {
            let from = self
                .agent_modes
                .insert(agent_id, OperatingMode::Normal)
                .unwrap_or(OperatingMode::Normal);
            let _ = audit.append_event(
                agent_id,
                EventType::UserAction,
                json!({
                    "event_kind": "safety.mode_changed",
                    "agent_id": agent_id,
                    "from": mode_name(&from),
                    "to": "normal",
                    "reason": "violations_reset",
                }),
            );
        }

        self.refresh_global_mode();
    }

    pub fn generate_incident_report(
        &mut self,
        agent_id: AgentId,
        action_taken: &str,
        audit: &mut AuditTrail,
    ) -> IncidentReport {
        self.generate_incident_report_internal(agent_id, action_taken, audit)
    }

    fn record_violation(
        &mut self,
        agent_id: AgentId,
        violation: KpiViolation,
        audit: &mut AuditTrail,
    ) -> SafetyAction {
        let count = {
            let entry = self.violation_counter.entry(agent_id).or_insert(0);
            *entry = entry.saturating_add(1);
            *entry
        };

        let _ = audit.append_event(
            agent_id,
            EventType::Error,
            json!({
                "event_kind": "safety.violation_recorded",
                "agent_id": agent_id,
                "count": count,
                "kpi_kind": violation.kind.as_str(),
                "value": violation.value,
                "status": violation.status.as_str(),
                "warn_value": violation.warn_value,
                "critical_value": violation.critical_value,
            }),
        );

        match count {
            0 | 1 => {
                self.agent_modes
                    .entry(agent_id)
                    .or_insert(OperatingMode::Normal);
                self.refresh_global_mode();
                SafetyAction::Continue
            }
            2 => {
                let reason = format!(
                    "consecutive safety violations reached degraded threshold ({})",
                    violation.kind.as_str()
                );
                self.set_mode(agent_id, OperatingMode::Degraded(reason.clone()), audit);
                SafetyAction::Degraded { reason }
            }
            _ => {
                let reason = format!(
                    "three-strike safety halt triggered by {}",
                    violation.kind.as_str()
                );
                self.set_mode(agent_id, OperatingMode::Halted(reason.clone()), audit);
                let report = self.generate_incident_report_internal(agent_id, "halted", audit);
                let _ = audit.append_event(
                    agent_id,
                    EventType::Error,
                    json!({
                        "event_kind": "safety.agent_halted",
                        "agent_id": agent_id,
                        "violations": count,
                        "report_id": report.report_id,
                    }),
                );

                SafetyAction::Halted {
                    reason,
                    report_id: report.report_id,
                }
            }
        }
    }

    fn generate_incident_report_internal(
        &mut self,
        agent_id: AgentId,
        action_taken: &str,
        audit: &mut AuditTrail,
    ) -> IncidentReport {
        let sequence = u64::try_from(audit.events().len()).unwrap_or(u64::MAX) + 1;
        let report_id = deterministic_report_id(agent_id, sequence);
        let violations = self
            .recent_violations
            .get(&agent_id)
            .cloned()
            .unwrap_or_default();

        let mut excerpt = audit
            .events()
            .iter()
            .rev()
            .take(INCIDENT_EXCERPT_SIZE)
            .cloned()
            .collect::<Vec<_>>();
        excerpt.reverse();

        let recommendation = if action_taken == "halted" {
            "Require manual review before restarting agent; validate fuel/audit policies and replay last deterministic trace.".to_string()
        } else {
            "Run in degraded mode and re-evaluate KPI thresholds after deterministic replay."
                .to_string()
        };

        let mut report = IncidentReport {
            report_id: report_id.clone(),
            agent_id: agent_id.to_string(),
            timestamp_seq: sequence,
            kpi_violations: violations,
            action_taken: action_taken.to_string(),
            audit_trail_excerpt: excerpt,
            recommendation,
            signature: String::new(),
        };

        report.signature = sign_report(&report, audit);

        self.incident_reports
            .entry(agent_id)
            .or_default()
            .push(report.clone());

        let _ = audit.append_event(
            agent_id,
            EventType::UserAction,
            json!({
                "event_kind": "safety.incident_report_generated",
                "report_id": report_id,
                "agent_id": agent_id,
                "timestamp_seq": sequence,
                "action_taken": action_taken,
                "signature": report.signature,
            }),
        );

        report
    }

    fn threshold_for(&self, kind: KpiKind) -> Option<&KpiThreshold> {
        self.thresholds.iter().find(|entry| entry.kind == kind)
    }

    fn set_mode(&mut self, agent_id: AgentId, next: OperatingMode, audit: &mut AuditTrail) {
        let from = self
            .agent_modes
            .insert(agent_id, next.clone())
            .unwrap_or(OperatingMode::Normal);

        let reason = match &next {
            OperatingMode::Normal => "normal".to_string(),
            OperatingMode::Degraded(reason) | OperatingMode::Halted(reason) => reason.clone(),
        };

        let _ = audit.append_event(
            agent_id,
            EventType::UserAction,
            json!({
                "event_kind": "safety.mode_changed",
                "agent_id": agent_id,
                "from": mode_name(&from),
                "to": mode_name(&next),
                "reason": reason,
            }),
        );

        self.refresh_global_mode();
    }

    fn refresh_global_mode(&mut self) {
        let mut global = OperatingMode::Normal;
        for mode in self.agent_modes.values() {
            match mode {
                OperatingMode::Halted(reason) => {
                    global = OperatingMode::Halted(reason.clone());
                    break;
                }
                OperatingMode::Degraded(reason) => {
                    if !matches!(global, OperatingMode::Halted(_)) {
                        global = OperatingMode::Degraded(reason.clone());
                    }
                }
                OperatingMode::Normal => {}
            }
        }
        self.mode = global;
    }
}

fn sign_report(report: &IncidentReport, audit: &AuditTrail) -> String {
    let chain_tip = audit
        .events()
        .last()
        .map(|event| event.hash.as_str())
        .unwrap_or(GENESIS_HASH);

    #[derive(Serialize)]
    struct CanonicalIncident<'a> {
        report_id: &'a str,
        agent_id: &'a str,
        timestamp_seq: u64,
        action_taken: &'a str,
        recommendation: &'a str,
        chain_tip: &'a str,
    }

    let canonical = CanonicalIncident {
        report_id: report.report_id.as_str(),
        agent_id: report.agent_id.as_str(),
        timestamp_seq: report.timestamp_seq,
        action_taken: report.action_taken.as_str(),
        recommendation: report.recommendation.as_str(),
        chain_tip,
    };

    let encoded = serde_json::to_vec(&canonical).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(encoded);
    format!("{:x}", hasher.finalize())
}

fn deterministic_report_id(agent_id: AgentId, sequence: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(agent_id.to_string().as_bytes());
    hasher.update(sequence.to_le_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("ir-{}", &digest[..16])
}

fn mode_name(mode: &OperatingMode) -> &'static str {
    match mode {
        OperatingMode::Normal => "normal",
        OperatingMode::Degraded(_) => "degraded",
        OperatingMode::Halted(_) => "halted",
    }
}

pub fn default_thresholds() -> Vec<KpiThreshold> {
    vec![
        KpiThreshold {
            kind: KpiKind::GovernanceOverhead,
            warn_value: 5.0,
            critical_value: 10.0,
        },
        KpiThreshold {
            kind: KpiKind::LlmLatency,
            warn_value: 5_000.0,
            critical_value: 15_000.0,
        },
        KpiThreshold {
            kind: KpiKind::AuditChainIntegrity,
            warn_value: 1.0,
            critical_value: 1.0,
        },
        KpiThreshold {
            kind: KpiKind::FuelBurnRate,
            warn_value: 90.0,
            critical_value: 100.0,
        },
        KpiThreshold {
            kind: KpiKind::AgentErrorRate,
            warn_value: 10.0,
            critical_value: 25.0,
        },
        KpiThreshold {
            kind: KpiKind::BudgetCompliance,
            warn_value: 90.0,
            critical_value: 100.0,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{default_thresholds, KpiKind, OperatingMode, SafetyAction, SafetySupervisor};
    use crate::audit::AuditTrail;
    use uuid::Uuid;

    #[test]
    fn test_kpi_normal() {
        let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let readings = [
            (KpiKind::GovernanceOverhead, 1.0),
            (KpiKind::LlmLatency, 500.0),
            (KpiKind::AuditChainIntegrity, 0.0),
            (KpiKind::AgentErrorRate, 1.0),
            (KpiKind::BudgetCompliance, 50.0),
        ];

        let action = supervisor.heartbeat(agent_id, &readings, &mut audit);
        assert_eq!(action, SafetyAction::Continue);
        assert_eq!(supervisor.mode_for_agent(agent_id), OperatingMode::Normal);
        assert_eq!(supervisor.violation_count(agent_id), 0);
    }

    #[test]
    fn test_kpi_degraded() {
        let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let readings = [(KpiKind::GovernanceOverhead, 6.0)];

        let first = supervisor.heartbeat(agent_id, &readings, &mut audit);
        let second = supervisor.heartbeat(agent_id, &readings, &mut audit);

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
        let agent_id = Uuid::new_v4();
        let readings = [(KpiKind::LlmLatency, 20_000.0)];

        let _ = supervisor.heartbeat(agent_id, &readings, &mut audit);
        let _ = supervisor.heartbeat(agent_id, &readings, &mut audit);
        let third = supervisor.heartbeat(agent_id, &readings, &mut audit);

        assert!(matches!(third, SafetyAction::Halted { .. }));
        assert!(matches!(
            supervisor.mode_for_agent(agent_id),
            OperatingMode::Halted(_)
        ));

        let halted_logged = audit.events().iter().any(|event| {
            event
                .payload
                .get("event_kind")
                .and_then(|value| value.as_str())
                == Some("safety.agent_halted")
        });
        assert!(halted_logged);
    }

    #[test]
    fn test_reset_on_success() {
        let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        let violation = [(KpiKind::GovernanceOverhead, 6.0)];
        let success = [(KpiKind::GovernanceOverhead, 1.0)];

        let _ = supervisor.heartbeat(agent_id, &violation, &mut audit);
        let _ = supervisor.heartbeat(agent_id, &violation, &mut audit);
        let action = supervisor.heartbeat(agent_id, &success, &mut audit);

        assert_eq!(action, SafetyAction::Continue);
        assert_eq!(supervisor.violation_count(agent_id), 0);
        assert_eq!(supervisor.mode_for_agent(agent_id), OperatingMode::Normal);
    }

    #[test]
    fn test_incident_report_structure() {
        let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let readings = [(KpiKind::LlmLatency, 20_000.0)];

        let _ = supervisor.heartbeat(agent_id, &readings, &mut audit);
        let _ = supervisor.heartbeat(agent_id, &readings, &mut audit);
        let _ = supervisor.heartbeat(agent_id, &readings, &mut audit);

        let report = supervisor
            .last_incident_report(agent_id)
            .expect("incident report should exist after halt");

        assert_eq!(report.agent_id, agent_id.to_string());
        assert!(!report.kpi_violations.is_empty());
        assert!(!report.audit_trail_excerpt.is_empty());
        assert!(!report.signature.is_empty());
    }
}
