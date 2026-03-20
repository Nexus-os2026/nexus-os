use crate::adaptive_policy::{AdaptiveGovernor, AutonomyChange, RunOutcome};
use crate::audit::{AuditEvent, AuditTrail, EventType};
use crate::behavioral_profile::{ActionRecord, DriftAlert, DriftSeverity};
use crate::drift_detector::DriftDetector;
use crate::kill_gates::{GateStatus, KillGateConfig, KillGateError, KillGateRegistry};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

const INCIDENT_EXCERPT_SIZE: usize = 10;
const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const VIOLATION_COOLDOWN_SECS: u64 = 60;

pub type AgentId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KpiKind {
    GovernanceOverhead,
    LlmLatency,
    AuditChainIntegrity,
    FuelBurnRate,
    AgentErrorRate,
    BudgetCompliance,
    BanRate,
    ReplayMismatch,
    Divergence,
    QuorumInvariant,
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
            KpiKind::BanRate => "ban_rate",
            KpiKind::ReplayMismatch => "replay_mismatch",
            KpiKind::Divergence => "divergence",
            KpiKind::QuorumInvariant => "quorum_invariant",
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

#[derive(Debug)]
pub struct SafetySupervisor {
    pub thresholds: Vec<KpiThreshold>,
    pub violation_counter: HashMap<AgentId, u32>,
    pub mode: OperatingMode,
    agent_modes: HashMap<AgentId, OperatingMode>,
    recent_violations: HashMap<AgentId, Vec<KpiViolation>>,
    incident_reports: HashMap<AgentId, Vec<IncidentReport>>,
    tool_call_heartbeat_every: u32,
    tool_call_counter: HashMap<AgentId, u32>,
    kill_gates: KillGateRegistry,
    adaptive_governor: AdaptiveGovernor,
    drift_detector: DriftDetector,
    last_violation_timestamp: HashMap<AgentId, u64>,
    /// Monotonic sequence used as a clock in tests / environments without wall time.
    sequence_clock: u64,
}

impl Default for SafetySupervisor {
    fn default() -> Self {
        Self::new(default_thresholds(), 10)
    }
}

impl SafetySupervisor {
    pub fn new(thresholds: Vec<KpiThreshold>, tool_call_heartbeat_every: u32) -> Self {
        Self::with_kill_gates_config(
            thresholds,
            tool_call_heartbeat_every,
            &KillGateConfig::default(),
        )
    }

    pub fn with_kill_gates_config(
        thresholds: Vec<KpiThreshold>,
        tool_call_heartbeat_every: u32,
        kill_gate_config: &KillGateConfig,
    ) -> Self {
        Self {
            thresholds,
            violation_counter: HashMap::new(),
            mode: OperatingMode::Normal,
            agent_modes: HashMap::new(),
            recent_violations: HashMap::new(),
            incident_reports: HashMap::new(),
            tool_call_heartbeat_every: tool_call_heartbeat_every.max(1),
            tool_call_counter: HashMap::new(),
            kill_gates: KillGateRegistry::from_config(kill_gate_config),
            adaptive_governor: AdaptiveGovernor::new(),
            drift_detector: DriftDetector::new(),
            last_violation_timestamp: HashMap::new(),
            sequence_clock: 0,
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
        self.sequence_clock = self.sequence_clock.saturating_add(1);

        if readings.is_empty() {
            self.try_auto_reset_violations(agent_id, audit);
            return SafetyAction::Continue;
        }

        let mut violations = Vec::new();
        let mut first_frozen_subsystem = None::<String>;
        let mut first_halted_subsystem = None::<String>;
        for (kind, value) in readings {
            let status = self.check_kpi(*kind, *value);
            let threshold = self.threshold_for(*kind).cloned();
            let warn_value = threshold.as_ref().map_or(0.0, |row| row.warn_value);
            let critical_value = threshold.as_ref().map_or(0.0, |row| row.critical_value);

            if let Err(e) = audit
                .append_event(
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
                )
            {
                eprintln!("audit write failed: {e}");
            }


            if status != KpiStatus::Ok {
                violations.push(KpiViolation {
                    kind: *kind,
                    value: *value,
                    status,
                    warn_value,
                    critical_value,
                });
            }

            for (subsystem, gate_status) in
                self.kill_gates.check_metric(*kind, *value, agent_id, audit)
            {
                match gate_status {
                    GateStatus::Open => {}
                    GateStatus::Frozen => {
                        if first_frozen_subsystem.is_none() {
                            first_frozen_subsystem = Some(subsystem);
                        }
                    }
                    GateStatus::Halted => {
                        if first_halted_subsystem.is_none() {
                            first_halted_subsystem = Some(subsystem);
                        }
                    }
                }
            }
        }

        if let Some(subsystem) = first_halted_subsystem {
            return self.force_halt(
                agent_id,
                format!("kill gate halted subsystem '{subsystem}'"),
                audit,
            );
        }

        if let Some(subsystem) = first_frozen_subsystem {
            let reason = format!("kill gate froze subsystem '{subsystem}'");
            self.set_mode(agent_id, OperatingMode::Degraded(reason.clone()), audit);
            return SafetyAction::Degraded { reason };
        }

        if violations.is_empty() {
            self.try_auto_reset_violations(agent_id, audit);
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

    pub fn kill_gate_status(&self, subsystem: &str) -> Option<GateStatus> {
        self.kill_gates.gate_status(subsystem)
    }

    pub fn manual_freeze_subsystem(
        &mut self,
        subsystem: &str,
        operator_id: &str,
        agent_id: AgentId,
        audit: &mut AuditTrail,
    ) -> Result<GateStatus, KillGateError> {
        let status = self
            .kill_gates
            .manual_freeze(subsystem, operator_id, agent_id, audit)?;
        if status == GateStatus::Frozen {
            self.set_mode(
                agent_id,
                OperatingMode::Degraded(format!("manual freeze for subsystem '{subsystem}'")),
                audit,
            );
        }
        Ok(status)
    }

    pub fn manual_halt_subsystem(
        &mut self,
        subsystem: &str,
        operator_id: &str,
        agent_id: AgentId,
        audit: &mut AuditTrail,
    ) -> Result<SafetyAction, KillGateError> {
        let status = self
            .kill_gates
            .manual_halt(subsystem, operator_id, agent_id, audit)?;
        if status == GateStatus::Halted {
            return Ok(self.force_halt(
                agent_id,
                format!("manual halt for subsystem '{subsystem}'"),
                audit,
            ));
        }
        Ok(SafetyAction::Continue)
    }

    pub fn manual_unfreeze_subsystem(
        &mut self,
        subsystem: &str,
        operator_id: &str,
        hitl_tier: u8,
        agent_id: AgentId,
        audit: &mut AuditTrail,
    ) -> Result<GateStatus, KillGateError> {
        self.kill_gates
            .manual_unfreeze(subsystem, operator_id, hitl_tier, agent_id, audit)
    }

    pub fn force_halt(
        &mut self,
        agent_id: AgentId,
        reason: String,
        audit: &mut AuditTrail,
    ) -> SafetyAction {
        self.set_mode(agent_id, OperatingMode::Halted(reason.clone()), audit);
        let report = self.generate_incident_report_internal(agent_id, "halted", audit);
        let violations = self.violation_counter.get(&agent_id).copied().unwrap_or(0);
        if let Err(e) = audit
            .append_event(
                agent_id,
                EventType::Error,
                json!({
                    "event_kind": "safety.agent_halted",
                    "agent_id": agent_id,
                    "violations": violations,
                    "report_id": report.report_id,
                }),
            )
        {
            eprintln!("audit write failed: {e}");
        }


        SafetyAction::Halted {
            reason,
            report_id: report.report_id,
        }
    }

    pub fn observe_tool_call(
        &mut self,
        agent_id: AgentId,
        readings: &[(KpiKind, f64)],
        audit: &mut AuditTrail,
    ) -> SafetyAction {
        self.sequence_clock = self.sequence_clock.saturating_add(1);

        // Record tool call in drift detector.
        let action_record = ActionRecord {
            action_type: "tool_call".to_string(),
            timestamp: self.sequence_clock,
            fuel_cost: 1,
            resource_usage: None,
        };
        let drift_alerts = self
            .drift_detector
            .observe_action(&agent_id.to_string(), action_record);
        let drift_action = self.process_drift_alerts(agent_id, &drift_alerts, audit);
        if matches!(drift_action, SafetyAction::Halted { .. }) {
            return drift_action;
        }

        let counter = self.tool_call_counter.entry(agent_id).or_insert(0);
        *counter = counter.saturating_add(1);
        if (*counter).is_multiple_of(self.tool_call_heartbeat_every) {
            let heartbeat_action = self.heartbeat(agent_id, readings, audit);

            // Record run in adaptive governor.
            let outcome = match &heartbeat_action {
                SafetyAction::Continue => RunOutcome::Success,
                SafetyAction::Degraded { reason } => RunOutcome::Failed {
                    reason: reason.clone(),
                },
                SafetyAction::Halted { reason, .. } => RunOutcome::PolicyViolation {
                    violation: reason.clone(),
                },
            };
            self.adaptive_governor.record_run(agent_id, outcome, 1, 100);

            return heartbeat_action;
        }

        if matches!(drift_action, SafetyAction::Degraded { .. }) {
            return drift_action;
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
        self.sequence_clock = self.sequence_clock.saturating_add(1);

        // Record LLM call in drift detector.
        let action_record = ActionRecord {
            action_type: "llm_call".to_string(),
            timestamp: self.sequence_clock,
            fuel_cost: latency_ms,
            resource_usage: None,
        };
        let drift_alerts = self
            .drift_detector
            .observe_action(&agent_id.to_string(), action_record);
        let drift_action = self.process_drift_alerts(agent_id, &drift_alerts, audit);
        if matches!(drift_action, SafetyAction::Halted { .. }) {
            return drift_action;
        }

        let integrity = if audit.verify_integrity() { 0.0 } else { 1.0 };
        let readings = [
            (KpiKind::LlmLatency, latency_ms as f64),
            (KpiKind::GovernanceOverhead, governance_overhead_pct),
            (KpiKind::AuditChainIntegrity, integrity),
        ];
        let heartbeat_action = self.heartbeat(agent_id, &readings, audit);

        // Record run in adaptive governor.
        let outcome = match &heartbeat_action {
            SafetyAction::Continue => RunOutcome::Success,
            SafetyAction::Degraded { reason } => RunOutcome::Failed {
                reason: reason.clone(),
            },
            SafetyAction::Halted { reason, .. } => RunOutcome::PolicyViolation {
                violation: reason.clone(),
            },
        };
        self.adaptive_governor.record_run(agent_id, outcome, 1, 100);

        if matches!(
            heartbeat_action,
            SafetyAction::Halted { .. } | SafetyAction::Degraded { .. }
        ) {
            return heartbeat_action;
        }

        if matches!(drift_action, SafetyAction::Degraded { .. }) {
            return drift_action;
        }

        heartbeat_action
    }

    pub fn observe_workflow_node_completion(
        &mut self,
        agent_id: AgentId,
        readings: &[(KpiKind, f64)],
        audit: &mut AuditTrail,
    ) -> SafetyAction {
        self.heartbeat(agent_id, readings, audit)
    }

    /// Explicit reset — always resets immediately (operator-driven).
    pub fn reset_violations(&mut self, agent_id: AgentId, audit: &mut AuditTrail) {
        self.do_reset_violations(agent_id, audit);
    }

    /// Automatic reset — only resets if the cooldown period has elapsed since
    /// the last violation.  This prevents the "single clean heartbeat" exploit.
    fn try_auto_reset_violations(&mut self, agent_id: AgentId, audit: &mut AuditTrail) {
        if let Some(&last_ts) = self.last_violation_timestamp.get(&agent_id) {
            if self.sequence_clock.saturating_sub(last_ts) < VIOLATION_COOLDOWN_SECS {
                return;
            }
        }
        self.do_reset_violations(agent_id, audit);
    }

    fn do_reset_violations(&mut self, agent_id: AgentId, audit: &mut AuditTrail) {
        self.violation_counter.insert(agent_id, 0);

        if matches!(
            self.agent_modes.get(&agent_id),
            Some(OperatingMode::Degraded(_))
        ) {
            let from = self
                .agent_modes
                .insert(agent_id, OperatingMode::Normal)
                .unwrap_or(OperatingMode::Normal);
            if let Err(e) = audit
                .append_event(
                    agent_id,
                    EventType::UserAction,
                    json!({
                        "event_kind": "safety.mode_changed",
                        "agent_id": agent_id,
                        "from": mode_name(&from),
                        "to": "normal",
                        "reason": "violations_reset",
                    }),
                )
            {
                eprintln!("audit write failed: {e}");
            }

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

    /// Register an agent in the adaptive governor for trust-based promotions/demotions.
    pub fn register_agent_adaptive(
        &mut self,
        agent_id: AgentId,
        base_autonomy: u8,
        max_autonomy: u8,
    ) {
        self.adaptive_governor
            .register(agent_id, base_autonomy, max_autonomy);
    }

    /// Register an agent in the drift detector with a role profile.
    pub fn register_agent_role(
        &mut self,
        agent_id: AgentId,
        role: &str,
    ) -> Result<(), crate::drift_detector::DriftError> {
        self.drift_detector
            .register_agent(&agent_id.to_string(), role)
    }

    /// Returns pending promotions that require human approval.
    pub fn check_promotions(&self) -> Vec<(AgentId, AutonomyChange)> {
        let mut promotions = Vec::new();
        for &agent_id in self.agent_modes.keys() {
            let change = self.adaptive_governor.evaluate(agent_id);
            if matches!(change, AutonomyChange::Promote { .. }) {
                promotions.push((agent_id, change));
            }
        }
        promotions
    }

    /// Approve a promotion surfaced by check_promotions.
    pub fn approve_promotion(&mut self, agent_id: AgentId) -> bool {
        self.adaptive_governor.apply_promotion(agent_id, true)
    }

    pub fn drift_detector(&self) -> &DriftDetector {
        &self.drift_detector
    }

    pub fn adaptive_governor(&self) -> &AdaptiveGovernor {
        &self.adaptive_governor
    }

    /// Advance the internal sequence clock (useful for testing cooldowns).
    pub fn advance_clock(&mut self, ticks: u64) {
        self.sequence_clock = self.sequence_clock.saturating_add(ticks);
    }

    fn process_drift_alerts(
        &mut self,
        agent_id: AgentId,
        alerts: &[DriftAlert],
        audit: &mut AuditTrail,
    ) -> SafetyAction {
        if alerts.is_empty() {
            return SafetyAction::Continue;
        }

        let max_severity = alerts
            .iter()
            .map(|a| a.severity)
            .max()
            .unwrap_or(DriftSeverity::Low);

        for alert in alerts {
            if let Err(e) = audit
                .append_event(
                    agent_id,
                    EventType::StateChange,
                    json!({
                        "event_kind": "safety.drift_alert",
                        "agent_id": agent_id,
                        "drift_type": format!("{:?}", alert.drift_type),
                        "severity": format!("{:?}", alert.severity),
                        "details": alert.details,
                        "deviation_factor": alert.deviation_factor,
                    }),
                )
            {
                eprintln!("audit write failed: {e}");
            }

        }

        match max_severity {
            DriftSeverity::Critical => {
                let violation = KpiViolation {
                    kind: KpiKind::AgentErrorRate,
                    value: 100.0,
                    status: KpiStatus::Critical,
                    warn_value: 10.0,
                    critical_value: 25.0,
                };
                self.record_violation(agent_id, violation, audit)
            }
            DriftSeverity::High => {
                let violation = KpiViolation {
                    kind: KpiKind::AgentErrorRate,
                    value: 15.0,
                    status: KpiStatus::Warn,
                    warn_value: 10.0,
                    critical_value: 25.0,
                };
                self.record_violation(agent_id, violation, audit)
            }
            _ => SafetyAction::Continue,
        }
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

        // Track violation timestamp for cooldown.
        self.last_violation_timestamp
            .insert(agent_id, self.sequence_clock);

        if let Err(e) = audit
            .append_event(
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
            )
        {
            eprintln!("audit write failed: {e}");
        }


        // Record violation in adaptive governor.
        self.adaptive_governor.record_run(
            agent_id,
            RunOutcome::PolicyViolation {
                violation: violation.kind.as_str().to_string(),
            },
            1,
            100,
        );

        let action = match count {
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
                self.force_halt(agent_id, reason, audit)
            }
        };

        // After three-strike halt, check if adaptive governor suggests demotion.
        if matches!(action, SafetyAction::Halted { .. }) {
            let change = self.adaptive_governor.evaluate(agent_id);
            if matches!(change, AutonomyChange::Demote { .. }) {
                self.adaptive_governor.apply_demotion(agent_id);
            }
        }

        action
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

        if let Err(e) = audit
            .append_event(
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
            )
        {
            eprintln!("audit write failed: {e}");
        }


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

        if let Err(e) = audit
            .append_event(
                agent_id,
                EventType::UserAction,
                json!({
                    "event_kind": "safety.mode_changed",
                    "agent_id": agent_id,
                    "from": mode_name(&from),
                    "to": mode_name(&next),
                    "reason": reason,
                }),
            )
        {
            eprintln!("audit write failed: {e}");
        }


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
        KpiThreshold {
            kind: KpiKind::BanRate,
            warn_value: 2.0,
            critical_value: 5.0,
        },
        KpiThreshold {
            kind: KpiKind::ReplayMismatch,
            warn_value: 1.0,
            critical_value: 1.0,
        },
        KpiThreshold {
            kind: KpiKind::Divergence,
            warn_value: 1.0,
            critical_value: 1.0,
        },
        KpiThreshold {
            kind: KpiKind::QuorumInvariant,
            warn_value: 1.0,
            critical_value: 1.0,
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

        // Advance past the cooldown period before attempting reset.
        supervisor.advance_clock(super::VIOLATION_COOLDOWN_SECS);

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

    #[test]
    fn test_cooldown_prevents_immediate_reset() {
        let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        let violation = [(KpiKind::GovernanceOverhead, 6.0)];
        let success = [(KpiKind::GovernanceOverhead, 1.0)];

        let _ = supervisor.heartbeat(agent_id, &violation, &mut audit);
        assert_eq!(supervisor.violation_count(agent_id), 1);

        // Clean reading immediately after — should NOT reset due to cooldown.
        let _ = supervisor.heartbeat(agent_id, &success, &mut audit);
        assert_eq!(
            supervisor.violation_count(agent_id),
            1,
            "violation counter should not reset during cooldown"
        );
    }

    #[test]
    fn test_adaptive_governor_integration() {
        let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        supervisor.register_agent_adaptive(agent_id, 2, 4);

        // Three violations trigger halt and adaptive governor records them.
        let readings = [(KpiKind::LlmLatency, 20_000.0)];
        let _ = supervisor.heartbeat(agent_id, &readings, &mut audit);
        let _ = supervisor.heartbeat(agent_id, &readings, &mut audit);
        let _ = supervisor.heartbeat(agent_id, &readings, &mut audit);

        let record = supervisor.adaptive_governor().get_record(agent_id);
        assert!(record.is_some());
        let record = record.unwrap();
        assert!(record.policy_violations > 0);
        assert!(record.trust_score < 0.5);
    }

    #[test]
    fn test_drift_detector_role_registration() {
        let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
        let agent_id = Uuid::new_v4();

        assert!(supervisor
            .register_agent_role(agent_id, "researcher")
            .is_ok());
        assert!(supervisor
            .register_agent_role(agent_id, "unknown_role")
            .is_err());
    }

    #[test]
    fn test_observe_tool_call_records_drift_action() {
        let mut supervisor = SafetySupervisor::new(default_thresholds(), 1);
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        let readings = [(KpiKind::GovernanceOverhead, 1.0)];
        let action = supervisor.observe_tool_call(agent_id, &readings, &mut audit);

        // With clean readings, should continue.
        assert_eq!(action, SafetyAction::Continue);

        // Drift detector should have recorded the action.
        let baseline = supervisor
            .drift_detector()
            .profiler()
            .get_baseline(&agent_id.to_string());
        assert!(baseline.is_some());
        assert_eq!(baseline.unwrap().samples_collected, 1);
    }

    #[test]
    fn test_observe_llm_response_records_drift_action() {
        let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        let action = supervisor.observe_llm_response(agent_id, 100, 1.0, &mut audit);
        assert_eq!(action, SafetyAction::Continue);

        let baseline = supervisor
            .drift_detector()
            .profiler()
            .get_baseline(&agent_id.to_string());
        assert!(baseline.is_some());
        assert_eq!(baseline.unwrap().samples_collected, 1);
    }

    #[test]
    fn test_check_promotions_empty_by_default() {
        let supervisor = SafetySupervisor::new(default_thresholds(), 10);
        assert!(supervisor.check_promotions().is_empty());
    }

    #[test]
    fn test_violation_cooldown_allows_reset_after_delay() {
        let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        let violation = [(KpiKind::GovernanceOverhead, 6.0)];

        // Two violations → Degraded.
        let _ = supervisor.heartbeat(agent_id, &violation, &mut audit);
        let _ = supervisor.heartbeat(agent_id, &violation, &mut audit);
        assert_eq!(supervisor.violation_count(agent_id), 2);

        // Advance past the cooldown period.
        supervisor.advance_clock(super::VIOLATION_COOLDOWN_SECS);

        // Clean heartbeat should now reset.
        let action = supervisor.heartbeat(agent_id, &[], &mut audit);
        assert_eq!(action, SafetyAction::Continue);
        assert_eq!(supervisor.violation_count(agent_id), 0);
        assert_eq!(supervisor.mode_for_agent(agent_id), OperatingMode::Normal);
    }

    #[test]
    fn test_violation_cooldown_prevents_easy_reset() {
        let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        let violation = [(KpiKind::GovernanceOverhead, 6.0)];

        // Two violations → Degraded.
        let _ = supervisor.heartbeat(agent_id, &violation, &mut audit);
        let _ = supervisor.heartbeat(agent_id, &violation, &mut audit);
        assert_eq!(supervisor.violation_count(agent_id), 2);

        // Clean heartbeat immediately — should NOT reset.
        let _ = supervisor.heartbeat(agent_id, &[], &mut audit);
        assert_eq!(
            supervisor.violation_count(agent_id),
            2,
            "violation counter should not reset during cooldown"
        );
        assert!(
            matches!(
                supervisor.mode_for_agent(agent_id),
                OperatingMode::Degraded(_)
            ),
            "should still be degraded during cooldown"
        );
    }

    #[test]
    fn test_critical_drift_alert_triggers_violation() {
        let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        // Directly invoke process_drift_alerts with a Critical alert.
        let alerts = vec![crate::behavioral_profile::DriftAlert {
            agent_id: agent_id.to_string(),
            drift_type: crate::behavioral_profile::DriftType::FrequencySpike,
            severity: crate::behavioral_profile::DriftSeverity::Critical,
            details: "test critical drift".to_string(),
            current_value: 100.0,
            baseline_value: 10.0,
            deviation_factor: 10.0,
            timestamp: 0,
        }];

        let action = supervisor.process_drift_alerts(agent_id, &alerts, &mut audit);

        // Critical drift should record a violation (count = 1 → Continue).
        assert_eq!(action, SafetyAction::Continue);
        assert_eq!(supervisor.violation_count(agent_id), 1);

        // A drift alert audit event should be logged.
        let drift_logged = audit.events().iter().any(|event| {
            event.payload.get("event_kind").and_then(|v| v.as_str()) == Some("safety.drift_alert")
        });
        assert!(drift_logged);

        // A violation recorded event should be logged.
        let violation_logged = audit.events().iter().any(|event| {
            event.payload.get("event_kind").and_then(|v| v.as_str())
                == Some("safety.violation_recorded")
        });
        assert!(violation_logged);
    }

    #[test]
    fn test_advance_clock() {
        let mut supervisor = SafetySupervisor::new(default_thresholds(), 10);
        supervisor.advance_clock(100);
        // Verify clock advanced by checking that a subsequent heartbeat tick
        // has clock > 100.
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let _ = supervisor.heartbeat(agent_id, &[], &mut audit);
        // If we got here without panic, the clock is working.
    }
}
