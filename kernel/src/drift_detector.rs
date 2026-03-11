//! Semantic drift detection: role-aware behavioral monitoring for agents.

use crate::behavioral_profile::{
    ActionRecord, BehavioralProfiler, DriftAlert, DriftSeverity, DriftType,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Role profiles ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRoleProfile {
    pub role_name: String,
    pub expected_actions: Vec<String>,
    pub forbidden_actions: Vec<String>,
    pub max_autonomy: u8,
    pub max_fuel_per_hour: u64,
}

pub fn get_default_roles() -> HashMap<String, AgentRoleProfile> {
    let mut roles = HashMap::new();

    roles.insert(
        "researcher".to_string(),
        AgentRoleProfile {
            role_name: "researcher".to_string(),
            expected_actions: vec![
                "llm_call".to_string(),
                "fs_read".to_string(),
                "network_request".to_string(),
            ],
            forbidden_actions: vec!["fs_write".to_string(), "terminal_command".to_string()],
            max_autonomy: 2,
            max_fuel_per_hour: 5000,
        },
    );

    roles.insert(
        "coder".to_string(),
        AgentRoleProfile {
            role_name: "coder".to_string(),
            expected_actions: vec![
                "fs_read".to_string(),
                "fs_write".to_string(),
                "terminal_command".to_string(),
                "llm_call".to_string(),
            ],
            forbidden_actions: vec![],
            max_autonomy: 3,
            max_fuel_per_hour: 10000,
        },
    );

    roles.insert(
        "reviewer".to_string(),
        AgentRoleProfile {
            role_name: "reviewer".to_string(),
            expected_actions: vec!["fs_read".to_string(), "llm_call".to_string()],
            forbidden_actions: vec![
                "fs_write".to_string(),
                "terminal_command".to_string(),
                "network_request".to_string(),
            ],
            max_autonomy: 1,
            max_fuel_per_hour: 3000,
        },
    );

    roles
}

// ── Errors ──

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum DriftError {
    #[error("unknown role '{0}'")]
    UnknownRole(String),
    #[error("agent '{0}' not registered")]
    AgentNotRegistered(String),
}

// ── Assessment types ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftStatus {
    Normal,
    Drifting,
    Suspicious,
    Compromised,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecommendedAction {
    None,
    LogAndMonitor,
    DemoteAutonomy,
    RequireApproval,
    HaltAgent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftAssessment {
    pub agent_id: String,
    pub status: DriftStatus,
    pub alerts: Vec<DriftAlert>,
    pub recommended_action: RecommendedAction,
}

// ── Detector ──

#[derive(Debug)]
pub struct DriftDetector {
    role_profiles: HashMap<String, AgentRoleProfile>,
    agent_roles: HashMap<String, String>,
    profiler: BehavioralProfiler,
}

impl DriftDetector {
    pub fn new() -> Self {
        Self {
            role_profiles: get_default_roles(),
            agent_roles: HashMap::new(),
            profiler: BehavioralProfiler::new(2.0),
        }
    }

    pub fn register_agent(&mut self, agent_id: &str, role: &str) -> Result<(), DriftError> {
        if !self.role_profiles.contains_key(role) {
            return Err(DriftError::UnknownRole(role.to_string()));
        }
        self.agent_roles
            .insert(agent_id.to_string(), role.to_string());
        Ok(())
    }

    pub fn register_custom_role(&mut self, profile: AgentRoleProfile) {
        self.role_profiles
            .insert(profile.role_name.clone(), profile);
    }

    pub fn observe_action(&mut self, agent_id: &str, action: ActionRecord) -> Vec<DriftAlert> {
        // Record in the profiler (builds baseline or feeds the window).
        self.profiler.record_action(agent_id, action.clone());

        let mut alerts = Vec::new();

        // Role-specific checks.
        let role_name = match self.agent_roles.get(agent_id) {
            Some(r) => r.clone(),
            None => return alerts,
        };
        let profile = match self.role_profiles.get(&role_name) {
            Some(p) => p.clone(),
            None => return alerts,
        };

        // Check forbidden actions.
        if profile.forbidden_actions.contains(&action.action_type) {
            alerts.push(DriftAlert {
                agent_id: agent_id.to_string(),
                drift_type: DriftType::RoleDeviation,
                severity: DriftSeverity::High,
                details: format!(
                    "forbidden action '{}' for role '{}'",
                    action.action_type, role_name
                ),
                current_value: 1.0,
                baseline_value: 0.0,
                deviation_factor: f64::INFINITY,
                timestamp: action.timestamp,
            });
        }

        // Check unexpected actions (not in expected list and not forbidden — just unusual for role).
        if !profile.expected_actions.contains(&action.action_type)
            && !profile.forbidden_actions.contains(&action.action_type)
        {
            alerts.push(DriftAlert {
                agent_id: agent_id.to_string(),
                drift_type: DriftType::RoleDeviation,
                severity: DriftSeverity::Medium,
                details: format!(
                    "unexpected action '{}' for role '{}' (not in expected set)",
                    action.action_type, role_name
                ),
                current_value: 1.0,
                baseline_value: 0.0,
                deviation_factor: 1.0,
                timestamp: action.timestamp,
            });
        }

        // Append any profiler-level drift alerts (frequency, resource, unusual-type).
        alerts.extend(self.profiler.check_drift(agent_id));

        alerts
    }

    pub fn evaluate_agent(&self, agent_id: &str) -> DriftAssessment {
        let alerts = self.profiler.check_drift(agent_id);

        let (status, recommended_action) = assess_from_alerts(&alerts);

        DriftAssessment {
            agent_id: agent_id.to_string(),
            status,
            alerts,
            recommended_action,
        }
    }

    pub fn profiler(&self) -> &BehavioralProfiler {
        &self.profiler
    }

    pub fn profiler_mut(&mut self) -> &mut BehavioralProfiler {
        &mut self.profiler
    }

    pub fn agent_role(&self, agent_id: &str) -> Option<&str> {
        self.agent_roles.get(agent_id).map(|s| s.as_str())
    }
}

impl Default for DriftDetector {
    fn default() -> Self {
        Self::new()
    }
}

fn assess_from_alerts(alerts: &[DriftAlert]) -> (DriftStatus, RecommendedAction) {
    if alerts.is_empty() {
        return (DriftStatus::Normal, RecommendedAction::None);
    }

    let max_severity = alerts
        .iter()
        .map(|a| a.severity)
        .max()
        .unwrap_or(DriftSeverity::Low);

    let has_role_deviation = alerts
        .iter()
        .any(|a| a.drift_type == DriftType::RoleDeviation);
    let critical_count = alerts
        .iter()
        .filter(|a| a.severity == DriftSeverity::Critical)
        .count();

    if critical_count >= 2 || (has_role_deviation && max_severity >= DriftSeverity::Critical) {
        return (DriftStatus::Compromised, RecommendedAction::HaltAgent);
    }

    if max_severity == DriftSeverity::Critical
        || (has_role_deviation && max_severity >= DriftSeverity::High)
    {
        return (DriftStatus::Suspicious, RecommendedAction::RequireApproval);
    }

    if max_severity >= DriftSeverity::High {
        return (DriftStatus::Suspicious, RecommendedAction::DemoteAutonomy);
    }

    if max_severity >= DriftSeverity::Medium {
        return (DriftStatus::Drifting, RecommendedAction::LogAndMonitor);
    }

    (DriftStatus::Drifting, RecommendedAction::LogAndMonitor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavioral_profile::ActionRecord;

    fn make_action(action_type: &str, timestamp: u64, fuel_cost: u64) -> ActionRecord {
        ActionRecord {
            action_type: action_type.to_string(),
            timestamp,
            fuel_cost,
            resource_usage: None,
        }
    }

    #[test]
    fn default_roles_exist() {
        let roles = get_default_roles();
        assert!(roles.contains_key("researcher"));
        assert!(roles.contains_key("coder"));
        assert!(roles.contains_key("reviewer"));
    }

    #[test]
    fn register_known_role_succeeds() {
        let mut detector = DriftDetector::new();
        assert!(detector.register_agent("agent-1", "coder").is_ok());
        assert_eq!(detector.agent_role("agent-1"), Some("coder"));
    }

    #[test]
    fn register_unknown_role_fails() {
        let mut detector = DriftDetector::new();
        let result = detector.register_agent("agent-1", "hacker");
        assert_eq!(result, Err(DriftError::UnknownRole("hacker".to_string())));
    }

    #[test]
    fn forbidden_action_generates_role_deviation() {
        let mut detector = DriftDetector::new();
        detector.register_agent("agent-1", "researcher").unwrap();

        let alerts = detector.observe_action("agent-1", make_action("fs_write", 100, 10));

        let role_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| a.drift_type == DriftType::RoleDeviation)
            .collect();
        assert!(!role_alerts.is_empty());
        assert_eq!(role_alerts[0].severity, DriftSeverity::High);
        assert!(role_alerts[0].details.contains("forbidden"));
    }

    #[test]
    fn expected_action_generates_no_role_alert() {
        let mut detector = DriftDetector::new();
        detector.register_agent("agent-1", "coder").unwrap();

        let alerts = detector.observe_action("agent-1", make_action("fs_write", 100, 10));

        let role_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| a.drift_type == DriftType::RoleDeviation)
            .collect();
        assert!(role_alerts.is_empty());
    }

    #[test]
    fn unexpected_action_generates_medium_alert() {
        let mut detector = DriftDetector::new();
        detector.register_agent("agent-1", "reviewer").unwrap();

        // "data_export" is neither expected nor forbidden for reviewer.
        let alerts = detector.observe_action("agent-1", make_action("data_export", 100, 10));

        let role_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| a.drift_type == DriftType::RoleDeviation)
            .collect();
        assert!(!role_alerts.is_empty());
        assert_eq!(role_alerts[0].severity, DriftSeverity::Medium);
        assert!(role_alerts[0].details.contains("unexpected"));
    }

    #[test]
    fn unregistered_agent_gets_no_role_alerts() {
        let mut detector = DriftDetector::new();

        let alerts = detector.observe_action("unknown", make_action("fs_write", 100, 10));

        let role_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| a.drift_type == DriftType::RoleDeviation)
            .collect();
        assert!(role_alerts.is_empty());
    }

    #[test]
    fn evaluate_normal_agent() {
        let mut detector = DriftDetector::new();
        detector.register_agent("agent-1", "coder").unwrap();

        // Feed some expected actions (not enough for baseline, so no drift).
        for i in 0..10 {
            detector.observe_action("agent-1", make_action("fs_read", i, 10));
        }

        let assessment = detector.evaluate_agent("agent-1");
        assert_eq!(assessment.status, DriftStatus::Normal);
        assert_eq!(assessment.recommended_action, RecommendedAction::None);
        assert!(assessment.alerts.is_empty());
    }

    #[test]
    fn custom_role_registration() {
        let mut detector = DriftDetector::new();
        detector.register_custom_role(AgentRoleProfile {
            role_name: "auditor".to_string(),
            expected_actions: vec!["fs_read".to_string(), "llm_call".to_string()],
            forbidden_actions: vec!["fs_write".to_string()],
            max_autonomy: 1,
            max_fuel_per_hour: 2000,
        });

        assert!(detector.register_agent("agent-1", "auditor").is_ok());

        let alerts = detector.observe_action("agent-1", make_action("fs_write", 100, 10));
        assert!(alerts
            .iter()
            .any(|a| a.drift_type == DriftType::RoleDeviation));
    }

    #[test]
    fn assess_from_alerts_empty() {
        let (status, action) = assess_from_alerts(&[]);
        assert_eq!(status, DriftStatus::Normal);
        assert_eq!(action, RecommendedAction::None);
    }

    #[test]
    fn assess_from_alerts_critical_role_deviation() {
        let alerts = vec![DriftAlert {
            agent_id: "a".to_string(),
            drift_type: DriftType::RoleDeviation,
            severity: DriftSeverity::Critical,
            details: "test".to_string(),
            current_value: 1.0,
            baseline_value: 0.0,
            deviation_factor: 10.0,
            timestamp: 0,
        }];
        let (status, action) = assess_from_alerts(&alerts);
        assert_eq!(status, DriftStatus::Compromised);
        assert_eq!(action, RecommendedAction::HaltAgent);
    }

    #[test]
    fn assess_from_alerts_high_severity() {
        let alerts = vec![DriftAlert {
            agent_id: "a".to_string(),
            drift_type: DriftType::FrequencySpike,
            severity: DriftSeverity::High,
            details: "test".to_string(),
            current_value: 1.0,
            baseline_value: 0.0,
            deviation_factor: 5.0,
            timestamp: 0,
        }];
        let (status, action) = assess_from_alerts(&alerts);
        assert_eq!(status, DriftStatus::Suspicious);
        assert_eq!(action, RecommendedAction::DemoteAutonomy);
    }

    #[test]
    fn assess_from_alerts_medium_severity() {
        let alerts = vec![DriftAlert {
            agent_id: "a".to_string(),
            drift_type: DriftType::UnusualActionType,
            severity: DriftSeverity::Medium,
            details: "test".to_string(),
            current_value: 1.0,
            baseline_value: 0.0,
            deviation_factor: 2.0,
            timestamp: 0,
        }];
        let (status, action) = assess_from_alerts(&alerts);
        assert_eq!(status, DriftStatus::Drifting);
        assert_eq!(action, RecommendedAction::LogAndMonitor);
    }

    #[test]
    fn researcher_terminal_command_is_forbidden() {
        let mut detector = DriftDetector::new();
        detector.register_agent("agent-1", "researcher").unwrap();

        let alerts = detector.observe_action("agent-1", make_action("terminal_command", 100, 10));

        let forbidden: Vec<_> = alerts
            .iter()
            .filter(|a| a.drift_type == DriftType::RoleDeviation && a.details.contains("forbidden"))
            .collect();
        assert!(!forbidden.is_empty());
    }

    #[test]
    fn coder_has_no_forbidden_actions() {
        let roles = get_default_roles();
        let coder = roles.get("coder").unwrap();
        assert!(coder.forbidden_actions.is_empty());
    }

    #[test]
    fn researcher_normal_actions_no_alerts() {
        let mut detector = DriftDetector::new();
        detector.register_agent("agent-1", "researcher").unwrap();

        // fs_read and llm_call are expected actions for researcher.
        let alerts1 = detector.observe_action("agent-1", make_action("fs_read", 100, 10));
        let alerts2 = detector.observe_action("agent-1", make_action("llm_call", 101, 10));

        let role_alerts: Vec<_> = alerts1
            .iter()
            .chain(alerts2.iter())
            .filter(|a| a.drift_type == DriftType::RoleDeviation)
            .collect();
        assert!(role_alerts.is_empty());
    }

    #[test]
    fn researcher_forbidden_fs_write() {
        let mut detector = DriftDetector::new();
        detector.register_agent("agent-1", "researcher").unwrap();

        let alerts = detector.observe_action("agent-1", make_action("fs_write", 100, 10));

        let role_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| a.drift_type == DriftType::RoleDeviation)
            .collect();
        assert!(!role_alerts.is_empty());
        assert_eq!(role_alerts[0].severity, DriftSeverity::High);
        assert!(role_alerts[0].details.contains("forbidden"));
    }

    #[test]
    fn coder_allows_terminal_command() {
        let mut detector = DriftDetector::new();
        detector.register_agent("agent-1", "coder").unwrap();

        let alerts = detector.observe_action("agent-1", make_action("terminal_command", 100, 10));

        let role_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| a.drift_type == DriftType::RoleDeviation)
            .collect();
        assert!(role_alerts.is_empty());
    }

    #[test]
    fn evaluate_agent_suspicious_on_high_alerts() {
        // Construct a DriftAlert with High severity + RoleDeviation to trigger
        // Suspicious status and RequireApproval recommendation.
        let alerts = vec![DriftAlert {
            agent_id: "agent-1".to_string(),
            drift_type: DriftType::RoleDeviation,
            severity: DriftSeverity::High,
            details: "forbidden action observed".to_string(),
            current_value: 1.0,
            baseline_value: 0.0,
            deviation_factor: 5.0,
            timestamp: 100,
        }];

        let (status, action) = assess_from_alerts(&alerts);
        assert_eq!(status, DriftStatus::Suspicious);
        assert_eq!(action, RecommendedAction::RequireApproval);
    }

    #[test]
    fn unknown_role_error() {
        let mut detector = DriftDetector::new();
        let result = detector.register_agent("agent-1", "nonexistent");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            DriftError::UnknownRole("nonexistent".to_string())
        );
    }
}
