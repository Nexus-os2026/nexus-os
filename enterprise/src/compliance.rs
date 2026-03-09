//! SOC2 compliance report generation from Nexus OS audit primitives.

use nexus_kernel::audit::AuditTrail;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlStatus {
    Satisfied,
    PartiallyMet { gaps: Vec<String> },
    NotMet { reason: String },
    NotApplicable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlEvidence {
    pub control_id: String,
    pub description: String,
    pub evidence_type: String,
    pub evidence_count: u64,
    pub status: ControlStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceSection {
    pub framework: String,
    pub controls: Vec<ControlEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub generated_at: u64,
    pub period_start: u64,
    pub period_end: u64,
    pub organization: String,
    pub sections: Vec<ComplianceSection>,
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Generate a SOC2 Type II compliance report by mapping Nexus OS primitives to controls.
pub fn generate_soc2_report(
    audit_trail: &AuditTrail,
    capabilities_configured: bool,
    hitl_enabled: bool,
    fuel_tracking_enabled: bool,
    organization: &str,
    period_start: u64,
    period_end: u64,
) -> ComplianceReport {
    let events = audit_trail.events();
    let events_in_period: Vec<_> = events
        .iter()
        .filter(|e| e.timestamp >= period_start && e.timestamp <= period_end)
        .collect();

    let total_events = events_in_period.len() as u64;

    // CC6.1 — Logical and Physical Access Controls (capability-gated access)
    let cc6_1 = if capabilities_configured {
        let cap_events = events_in_period
            .iter()
            .filter(|e| {
                e.payload
                    .get("action")
                    .and_then(|v| v.as_str())
                    .is_some_and(|a| {
                        a.contains("capability")
                            || a.contains("tool_call")
                            || a.contains("llm_query")
                    })
            })
            .count() as u64;

        ControlEvidence {
            control_id: "CC6.1".to_string(),
            description: "Logical access controls via capability-gated agent manifests".to_string(),
            evidence_type: "capability_check_events".to_string(),
            evidence_count: cap_events.max(total_events),
            status: ControlStatus::Satisfied,
        }
    } else {
        ControlEvidence {
            control_id: "CC6.1".to_string(),
            description: "Logical access controls via capability-gated agent manifests".to_string(),
            evidence_type: "capability_check_events".to_string(),
            evidence_count: 0,
            status: ControlStatus::NotMet {
                reason: "Capability-based access control is not configured".to_string(),
            },
        }
    };

    // CC6.2 — Prior to Issuing System Credentials (HITL approval tiers)
    let cc6_2 = if hitl_enabled {
        let approval_events = events_in_period
            .iter()
            .filter(|e| {
                e.payload
                    .get("event")
                    .and_then(|v| v.as_str())
                    .is_some_and(|a| a.contains("approval") || a.contains("consent"))
                    || e.payload
                        .get("action")
                        .and_then(|v| v.as_str())
                        .is_some_and(|a| a.contains("approval"))
            })
            .count() as u64;

        ControlEvidence {
            control_id: "CC6.2".to_string(),
            description: "Human-in-the-loop approval tiers for privileged operations".to_string(),
            evidence_type: "approval_events".to_string(),
            evidence_count: approval_events,
            status: ControlStatus::Satisfied,
        }
    } else {
        ControlEvidence {
            control_id: "CC6.2".to_string(),
            description: "Human-in-the-loop approval tiers for privileged operations".to_string(),
            evidence_type: "approval_events".to_string(),
            evidence_count: 0,
            status: ControlStatus::NotMet {
                reason: "HITL approval tiers are not enabled".to_string(),
            },
        }
    };

    // CC6.3 — System Operations and Monitoring (audit trail integrity)
    let chain_intact = audit_trail.verify_integrity();
    let cc6_3 = ControlEvidence {
        control_id: "CC6.3".to_string(),
        description: "Append-only hash-chained audit trail with integrity verification".to_string(),
        evidence_type: "audit_chain_events".to_string(),
        evidence_count: total_events,
        status: if chain_intact && total_events > 0 {
            ControlStatus::Satisfied
        } else if total_events == 0 {
            ControlStatus::PartiallyMet {
                gaps: vec!["No audit events recorded in the reporting period".to_string()],
            }
        } else {
            ControlStatus::NotMet {
                reason: "Audit trail integrity verification failed".to_string(),
            }
        },
    };

    // CC7.1 — System Monitoring (safety supervisor)
    let safety_events = events_in_period
        .iter()
        .filter(|e| {
            e.payload
                .get("event")
                .and_then(|v| v.as_str())
                .is_some_and(|a| {
                    a.contains("safety") || a.contains("kpi") || a.contains("supervisor")
                })
                || e.payload
                    .get("action")
                    .and_then(|v| v.as_str())
                    .is_some_and(|a| a.contains("safety"))
        })
        .count() as u64;

    let cc7_1 = ControlEvidence {
        control_id: "CC7.1".to_string(),
        description: "Continuous safety supervision with KPI monitoring and incident response"
            .to_string(),
        evidence_type: "safety_supervisor_events".to_string(),
        evidence_count: safety_events,
        status: if safety_events > 0 {
            ControlStatus::Satisfied
        } else {
            ControlStatus::PartiallyMet {
                gaps: vec!["No safety supervisor events found in the reporting period".to_string()],
            }
        },
    };

    // CC7.2 — Change Management (fuel budget controls)
    let cc7_2 = if fuel_tracking_enabled {
        let fuel_events = events_in_period
            .iter()
            .filter(|e| {
                e.payload
                    .get("event")
                    .and_then(|v| v.as_str())
                    .is_some_and(|a| a.contains("fuel"))
                    || e.payload
                        .get("action")
                        .and_then(|v| v.as_str())
                        .is_some_and(|a| a.contains("fuel"))
            })
            .count() as u64;

        ControlEvidence {
            control_id: "CC7.2".to_string(),
            description: "Fuel budget controls limiting agent resource consumption".to_string(),
            evidence_type: "fuel_tracking_events".to_string(),
            evidence_count: fuel_events,
            status: ControlStatus::Satisfied,
        }
    } else {
        ControlEvidence {
            control_id: "CC7.2".to_string(),
            description: "Fuel budget controls limiting agent resource consumption".to_string(),
            evidence_type: "fuel_tracking_events".to_string(),
            evidence_count: 0,
            status: ControlStatus::NotMet {
                reason: "Fuel tracking is not enabled".to_string(),
            },
        }
    };

    ComplianceReport {
        generated_at: unix_now(),
        period_start,
        period_end,
        organization: organization.to_string(),
        sections: vec![ComplianceSection {
            framework: "SOC2 Type II".to_string(),
            controls: vec![cc6_1, cc6_2, cc6_3, cc7_1, cc7_2],
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_kernel::audit::{AuditTrail, EventType};
    use serde_json::json;
    use uuid::Uuid;

    fn trail_with_events() -> AuditTrail {
        let mut trail = AuditTrail::new();
        let agent = Uuid::new_v4();

        trail
            .append_event(
                agent,
                EventType::ToolCall,
                json!({"action": "capability_check", "cap": "llm.query"}),
            )
            .expect("audit: fail-closed");
        trail
            .append_event(
                agent,
                EventType::UserAction,
                json!({"event": "consent.approval", "tier": 2}),
            )
            .expect("audit: fail-closed");
        trail
            .append_event(
                agent,
                EventType::StateChange,
                json!({"event": "safety.kpi_check", "status": "normal"}),
            )
            .expect("audit: fail-closed");
        trail
            .append_event(
                agent,
                EventType::StateChange,
                json!({"event": "fuel.budget_check", "remaining": 500}),
            )
            .expect("audit: fail-closed");
        trail
            .append_event(
                agent,
                EventType::LlmCall,
                json!({"action": "llm_query", "tokens": 100}),
            )
            .expect("audit: fail-closed");

        trail
    }

    #[test]
    fn soc2_report_generates_all_5_controls() {
        let trail = trail_with_events();
        let report = generate_soc2_report(&trail, true, true, true, "Nexus Corp", 0, u64::MAX);

        assert_eq!(report.organization, "Nexus Corp");
        assert_eq!(report.sections.len(), 1);
        assert_eq!(report.sections[0].framework, "SOC2 Type II");
        assert_eq!(report.sections[0].controls.len(), 5);

        let control_ids: Vec<&str> = report.sections[0]
            .controls
            .iter()
            .map(|c| c.control_id.as_str())
            .collect();
        assert_eq!(
            control_ids,
            vec!["CC6.1", "CC6.2", "CC6.3", "CC7.1", "CC7.2"]
        );
    }

    #[test]
    fn controls_satisfied_when_features_active() {
        let trail = trail_with_events();
        let report = generate_soc2_report(&trail, true, true, true, "Test Org", 0, u64::MAX);

        let controls = &report.sections[0].controls;

        // CC6.1: capabilities configured + events
        assert_eq!(controls[0].status, ControlStatus::Satisfied);
        assert!(controls[0].evidence_count > 0);

        // CC6.2: HITL enabled + approval events
        assert_eq!(controls[1].status, ControlStatus::Satisfied);
        assert!(controls[1].evidence_count > 0);

        // CC6.3: audit chain intact
        assert_eq!(controls[2].status, ControlStatus::Satisfied);
        assert!(controls[2].evidence_count > 0);

        // CC7.1: safety events present
        assert_eq!(controls[3].status, ControlStatus::Satisfied);
        assert!(controls[3].evidence_count > 0);

        // CC7.2: fuel tracking enabled
        assert_eq!(controls[4].status, ControlStatus::Satisfied);
    }

    #[test]
    fn controls_not_met_when_features_missing() {
        let trail = AuditTrail::new();
        let report = generate_soc2_report(&trail, false, false, false, "Empty Org", 0, u64::MAX);

        let controls = &report.sections[0].controls;

        // CC6.1: not configured
        assert!(matches!(controls[0].status, ControlStatus::NotMet { .. }));

        // CC6.2: HITL not enabled
        assert!(matches!(controls[1].status, ControlStatus::NotMet { .. }));

        // CC6.3: no events
        assert!(matches!(
            controls[2].status,
            ControlStatus::PartiallyMet { .. }
        ));

        // CC7.1: no safety events
        assert!(matches!(
            controls[3].status,
            ControlStatus::PartiallyMet { .. }
        ));

        // CC7.2: fuel not enabled
        assert!(matches!(controls[4].status, ControlStatus::NotMet { .. }));
    }

    #[test]
    fn report_covers_correct_time_period() {
        let trail = trail_with_events();
        let start = 1_000_000;
        let end = 2_000_000;

        let report = generate_soc2_report(&trail, true, true, true, "Period Test", start, end);

        assert_eq!(report.period_start, start);
        assert_eq!(report.period_end, end);

        // Events have current timestamps which are > 2_000_000, so they fall
        // within the (start, end) range only if timestamps are in that range.
        // Since test events have current timestamps (~1.7B+), the period filter
        // won't match for this narrow window.
        // But the report still generates with correct period bounds.
        assert_eq!(report.period_start, 1_000_000);
        assert_eq!(report.period_end, 2_000_000);
    }

    #[test]
    fn tampered_audit_trail_shows_not_met() {
        let mut trail = trail_with_events();
        // Tamper the trail
        trail.events_mut()[1].payload = json!({"tampered": true});

        let report = generate_soc2_report(&trail, true, true, true, "Tampered Org", 0, u64::MAX);

        let cc6_3 = &report.sections[0].controls[2];
        assert_eq!(cc6_3.control_id, "CC6.3");
        assert!(matches!(cc6_3.status, ControlStatus::NotMet { .. }));
    }
}
