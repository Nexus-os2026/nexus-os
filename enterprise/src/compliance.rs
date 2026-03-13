//! SOC2 compliance report generation from Nexus OS audit primitives.

use nexus_kernel::audit::AuditTrail;
use nexus_kernel::compliance::monitor::{AgentSnapshot, ComplianceMonitor, OverallStatus};
use nexus_kernel::identity::agent_identity::IdentityManager;
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

/// Generate an EU AI Act compliance section using the kernel compliance monitor.
pub fn generate_eu_ai_act_section(
    agents: &[AgentSnapshot],
    audit_trail: &AuditTrail,
    identity_manager: &IdentityManager,
) -> ComplianceSection {
    let monitor = ComplianceMonitor::new();
    let status = monitor.check_compliance(agents, audit_trail, identity_manager);

    // Article 5 — Prohibited practices
    let art5 = {
        let violations: Vec<_> = status
            .alerts
            .iter()
            .filter(|a| a.check_id == "EU_AI_ACT_PROHIBITED")
            .collect();
        ControlEvidence {
            control_id: "EU-AI-5".to_string(),
            description: "No prohibited AI practices (Article 5: biometric, social-scoring, \
                          law-enforcement, subliminal manipulation)"
                .to_string(),
            evidence_type: "risk_classification".to_string(),
            evidence_count: agents.len() as u64,
            status: if violations.is_empty() {
                ControlStatus::Satisfied
            } else {
                ControlStatus::NotMet {
                    reason: format!("{} agent(s) classified as Unacceptable", violations.len()),
                }
            },
        }
    };

    // Article 14 — Human oversight for high-risk
    let art14 = {
        let violations: Vec<_> = status
            .alerts
            .iter()
            .filter(|a| a.check_id == "HIGH_RISK_AUTONOMY")
            .collect();
        ControlEvidence {
            control_id: "EU-AI-14".to_string(),
            description: "High-risk AI systems have human oversight (autonomy ≤ L2)".to_string(),
            evidence_type: "autonomy_level_check".to_string(),
            evidence_count: agents.iter().filter(|a| a.running).count() as u64,
            status: if violations.is_empty() {
                ControlStatus::Satisfied
            } else {
                ControlStatus::NotMet {
                    reason: format!("{} high-risk agent(s) exceed L2 autonomy", violations.len()),
                }
            },
        }
    };

    // Article 13 — Transparency (agent identity / DID)
    let art13 = {
        let missing: Vec<_> = status
            .alerts
            .iter()
            .filter(|a| a.check_id == "MISSING_AGENT_IDENTITY")
            .collect();
        ControlEvidence {
            control_id: "EU-AI-13".to_string(),
            description: "Transparency: all agents have verifiable DID identity".to_string(),
            evidence_type: "identity_verification".to_string(),
            evidence_count: agents.iter().filter(|a| a.running).count() as u64,
            status: if missing.is_empty() {
                ControlStatus::Satisfied
            } else {
                ControlStatus::PartiallyMet {
                    gaps: missing.iter().map(|a| a.message.clone()).collect(),
                }
            },
        }
    };

    // Audit integrity
    let audit = {
        let broken = status
            .alerts
            .iter()
            .any(|a| a.check_id == "AUDIT_CHAIN_BROKEN");
        ControlEvidence {
            control_id: "EU-AI-AUDIT".to_string(),
            description: "Tamper-evident audit trail with hash-chain integrity".to_string(),
            evidence_type: "audit_chain_verification".to_string(),
            evidence_count: audit_trail.events().len() as u64,
            status: if broken {
                ControlStatus::NotMet {
                    reason: "Audit trail integrity verification failed".to_string(),
                }
            } else {
                ControlStatus::Satisfied
            },
        }
    };

    // Overall status control
    let overall = ControlEvidence {
        control_id: "EU-AI-OVERALL".to_string(),
        description: "Overall EU AI Act compliance status".to_string(),
        evidence_type: "compliance_monitor".to_string(),
        evidence_count: status.checks_passed as u64 + status.checks_failed as u64,
        status: match status.status {
            OverallStatus::Compliant => ControlStatus::Satisfied,
            OverallStatus::Warning => ControlStatus::PartiallyMet {
                gaps: status.alerts.iter().map(|a| a.message.clone()).collect(),
            },
            OverallStatus::Violation => ControlStatus::NotMet {
                reason: format!(
                    "{} violation(s) detected",
                    status
                        .alerts
                        .iter()
                        .filter(|a| a.severity
                            == nexus_kernel::compliance::monitor::AlertSeverity::Violation)
                        .count()
                ),
            },
        },
    };

    ComplianceSection {
        framework: "EU AI Act".to_string(),
        controls: vec![art5, art14, art13, audit, overall],
    }
}

/// Generate a HIPAA compliance section assessing PHI safeguards.
pub fn generate_hipaa_section(
    audit_trail: &AuditTrail,
    encryption_enabled: bool,
    access_controls_enabled: bool,
    period_start: u64,
    period_end: u64,
) -> ComplianceSection {
    let events = audit_trail.events();
    let events_in_period: Vec<_> = events
        .iter()
        .filter(|e| e.timestamp >= period_start && e.timestamp <= period_end)
        .collect();

    // § 164.312(a)(1) — Access Control
    let access = ControlEvidence {
        control_id: "HIPAA-AC".to_string(),
        description: "Access controls: unique agent identifiers and capability-gated access"
            .to_string(),
        evidence_type: "access_control_events".to_string(),
        evidence_count: events_in_period.len() as u64,
        status: if access_controls_enabled {
            ControlStatus::Satisfied
        } else {
            ControlStatus::NotMet {
                reason: "Capability-based access controls not configured".to_string(),
            }
        },
    };

    // § 164.312(a)(2)(iv) — Encryption and Decryption
    let encrypt = ControlEvidence {
        control_id: "HIPAA-ENC".to_string(),
        description: "Encryption of PHI at rest and in transit (AES-256-GCM)".to_string(),
        evidence_type: "encryption_config".to_string(),
        evidence_count: if encryption_enabled { 1 } else { 0 },
        status: if encryption_enabled {
            ControlStatus::Satisfied
        } else {
            ControlStatus::NotMet {
                reason: "Encryption not enabled for data at rest".to_string(),
            }
        },
    };

    // § 164.312(b) — Audit Controls
    let audit = {
        let chain_ok = audit_trail.verify_integrity();
        ControlEvidence {
            control_id: "HIPAA-AUDIT".to_string(),
            description: "Audit controls: tamper-evident logging of all access to PHI".to_string(),
            evidence_type: "audit_trail_events".to_string(),
            evidence_count: events_in_period.len() as u64,
            status: if chain_ok && !events_in_period.is_empty() {
                ControlStatus::Satisfied
            } else if events_in_period.is_empty() {
                ControlStatus::PartiallyMet {
                    gaps: vec!["No audit events in reporting period".to_string()],
                }
            } else {
                ControlStatus::NotMet {
                    reason: "Audit trail integrity verification failed".to_string(),
                }
            },
        }
    };

    // § 164.312(c)(1) — Integrity Controls
    let integrity = ControlEvidence {
        control_id: "HIPAA-INT".to_string(),
        description: "Integrity controls: hash-chained audit prevents unauthorized alteration"
            .to_string(),
        evidence_type: "hash_chain_verification".to_string(),
        evidence_count: events.len() as u64,
        status: if audit_trail.verify_integrity() || events.is_empty() {
            ControlStatus::Satisfied
        } else {
            ControlStatus::NotMet {
                reason: "Hash chain integrity compromised".to_string(),
            }
        },
    };

    ComplianceSection {
        framework: "HIPAA".to_string(),
        controls: vec![access, encrypt, audit, integrity],
    }
}

/// Generate a California AB 316 compliance section for autonomous decision transparency.
pub fn generate_california_ab316_section(
    agents: &[AgentSnapshot],
    audit_trail: &AuditTrail,
    identity_manager: &IdentityManager,
) -> ComplianceSection {
    let running_agents: Vec<_> = agents.iter().filter(|a| a.running).collect();

    // AB 316 § 1(a) — Disclosure of AI system use
    let disclosure = {
        let all_have_identity = running_agents
            .iter()
            .all(|a| identity_manager.get(&a.agent_id).is_some());
        ControlEvidence {
            control_id: "AB316-DISC".to_string(),
            description: "Disclosure: all autonomous agents have verifiable identity for \
                          attribution"
                .to_string(),
            evidence_type: "agent_identity_check".to_string(),
            evidence_count: running_agents.len() as u64,
            status: if all_have_identity || running_agents.is_empty() {
                ControlStatus::Satisfied
            } else {
                ControlStatus::PartiallyMet {
                    gaps: vec!["Some agents lack verifiable identity for disclosure".to_string()],
                }
            },
        }
    };

    // AB 316 § 1(b) — Decision audit trail
    let decision_trail = {
        let has_events = !audit_trail.events().is_empty();
        let chain_ok = audit_trail.verify_integrity();
        ControlEvidence {
            control_id: "AB316-TRAIL".to_string(),
            description: "Autonomous decision audit trail with tamper-evident logging".to_string(),
            evidence_type: "audit_chain_events".to_string(),
            evidence_count: audit_trail.events().len() as u64,
            status: if has_events && chain_ok {
                ControlStatus::Satisfied
            } else if !has_events {
                ControlStatus::PartiallyMet {
                    gaps: vec!["No decision audit events recorded".to_string()],
                }
            } else {
                ControlStatus::NotMet {
                    reason: "Decision audit trail integrity compromised".to_string(),
                }
            },
        }
    };

    // AB 316 § 1(c) — Human oversight capability
    let oversight = {
        let high_autonomy_without_oversight: Vec<_> = running_agents
            .iter()
            .filter(|a| {
                let level =
                    nexus_kernel::autonomy::AutonomyLevel::from_manifest(a.manifest.autonomy_level);
                level > nexus_kernel::autonomy::AutonomyLevel::L3
            })
            .collect();
        ControlEvidence {
            control_id: "AB316-OVER".to_string(),
            description: "Human override capability for autonomous decisions (max L3 recommended)"
                .to_string(),
            evidence_type: "autonomy_level_check".to_string(),
            evidence_count: running_agents.len() as u64,
            status: if high_autonomy_without_oversight.is_empty() {
                ControlStatus::Satisfied
            } else {
                ControlStatus::PartiallyMet {
                    gaps: high_autonomy_without_oversight
                        .iter()
                        .map(|a| {
                            format!(
                                "Agent '{}' has autonomy > L3 without explicit oversight",
                                a.manifest.name
                            )
                        })
                        .collect(),
                }
            },
        }
    };

    ComplianceSection {
        framework: "California AB 316".to_string(),
        controls: vec![disclosure, decision_trail, oversight],
    }
}

/// Configuration for multi-framework compliance report generation.
#[derive(Debug, Clone)]
pub struct FullReportConfig<'a> {
    pub audit_trail: &'a AuditTrail,
    pub agents: &'a [AgentSnapshot],
    pub identity_manager: &'a IdentityManager,
    pub capabilities_configured: bool,
    pub hitl_enabled: bool,
    pub fuel_tracking_enabled: bool,
    pub encryption_enabled: bool,
    pub organization: &'a str,
    pub period_start: u64,
    pub period_end: u64,
}

/// Generate a comprehensive multi-framework compliance report.
pub fn generate_full_compliance_report(config: &FullReportConfig<'_>) -> ComplianceReport {
    let soc2 = generate_soc2_report(
        config.audit_trail,
        config.capabilities_configured,
        config.hitl_enabled,
        config.fuel_tracking_enabled,
        config.organization,
        config.period_start,
        config.period_end,
    );

    let eu_ai_act =
        generate_eu_ai_act_section(config.agents, config.audit_trail, config.identity_manager);
    let hipaa = generate_hipaa_section(
        config.audit_trail,
        config.encryption_enabled,
        config.capabilities_configured,
        config.period_start,
        config.period_end,
    );
    let ab316 = generate_california_ab316_section(
        config.agents,
        config.audit_trail,
        config.identity_manager,
    );

    let mut sections = soc2.sections;
    sections.push(eu_ai_act);
    sections.push(hipaa);
    sections.push(ab316);

    ComplianceReport {
        generated_at: unix_now(),
        period_start: config.period_start,
        period_end: config.period_end,
        organization: config.organization.to_string(),
        sections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_kernel::audit::{AuditTrail, EventType};
    use nexus_kernel::identity::agent_identity::IdentityManager;
    use nexus_kernel::manifest::AgentManifest;
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
        trail.events_mut_for_testing()[1].payload = json!({"tampered": true});

        let report = generate_soc2_report(&trail, true, true, true, "Tampered Org", 0, u64::MAX);

        let cc6_3 = &report.sections[0].controls[2];
        assert_eq!(cc6_3.control_id, "CC6.3");
        assert!(matches!(cc6_3.status, ControlStatus::NotMet { .. }));
    }

    fn test_manifest(name: &str, caps: Vec<&str>) -> AgentManifest {
        AgentManifest {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            capabilities: caps.into_iter().map(String::from).collect(),
            fuel_budget: 1000,
            autonomy_level: None,
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            llm_model: None,
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
            filesystem_permissions: vec![],
        }
    }

    fn test_agent(name: &str, caps: Vec<&str>) -> AgentSnapshot {
        AgentSnapshot {
            agent_id: Uuid::new_v4(),
            manifest: test_manifest(name, caps),
            running: true,
        }
    }

    #[test]
    fn eu_ai_act_section_compliant() {
        let agent = test_agent("safe-agent", vec!["audit.read"]);
        let trail = AuditTrail::new();
        let mut id_mgr = IdentityManager::in_memory();
        id_mgr.get_or_create(agent.agent_id).unwrap();

        let section = generate_eu_ai_act_section(&[agent], &trail, &id_mgr);

        assert_eq!(section.framework, "EU AI Act");
        assert_eq!(section.controls.len(), 5);
        // All controls should be satisfied
        for ctrl in &section.controls {
            assert_eq!(
                ctrl.status,
                ControlStatus::Satisfied,
                "control {} not satisfied",
                ctrl.control_id
            );
        }
    }

    #[test]
    fn eu_ai_act_section_unacceptable_agent() {
        let mut agent = test_agent("bio-scanner", vec!["fs.read"]);
        agent.manifest.domain_tags = vec!["biometric".to_string()];
        let trail = AuditTrail::new();
        let mut id_mgr = IdentityManager::in_memory();
        id_mgr.get_or_create(agent.agent_id).unwrap();

        let section = generate_eu_ai_act_section(&[agent], &trail, &id_mgr);

        let art5 = &section.controls[0];
        assert_eq!(art5.control_id, "EU-AI-5");
        assert!(matches!(art5.status, ControlStatus::NotMet { .. }));

        let overall = &section.controls[4];
        assert_eq!(overall.control_id, "EU-AI-OVERALL");
        assert!(matches!(overall.status, ControlStatus::NotMet { .. }));
    }

    #[test]
    fn hipaa_section_all_enabled() {
        let trail = trail_with_events();
        let section = generate_hipaa_section(&trail, true, true, 0, u64::MAX);

        assert_eq!(section.framework, "HIPAA");
        assert_eq!(section.controls.len(), 4);
        for ctrl in &section.controls {
            assert_eq!(
                ctrl.status,
                ControlStatus::Satisfied,
                "control {} not satisfied",
                ctrl.control_id
            );
        }
    }

    #[test]
    fn hipaa_section_encryption_disabled() {
        let trail = trail_with_events();
        let section = generate_hipaa_section(&trail, false, true, 0, u64::MAX);

        let enc = section
            .controls
            .iter()
            .find(|c| c.control_id == "HIPAA-ENC")
            .unwrap();
        assert!(matches!(enc.status, ControlStatus::NotMet { .. }));
    }

    #[test]
    fn california_ab316_section_compliant() {
        let agent = test_agent("compliant-agent", vec!["audit.read"]);
        let trail = trail_with_events();
        let mut id_mgr = IdentityManager::in_memory();
        id_mgr.get_or_create(agent.agent_id).unwrap();

        let section = generate_california_ab316_section(&[agent], &trail, &id_mgr);

        assert_eq!(section.framework, "California AB 316");
        assert_eq!(section.controls.len(), 3);
        for ctrl in &section.controls {
            assert_eq!(
                ctrl.status,
                ControlStatus::Satisfied,
                "control {} not satisfied",
                ctrl.control_id
            );
        }
    }

    #[test]
    fn california_ab316_high_autonomy_warning() {
        let mut agent = test_agent("auto-agent", vec!["fs.write"]);
        agent.manifest.autonomy_level = Some(4); // L4
        let trail = trail_with_events();
        let mut id_mgr = IdentityManager::in_memory();
        id_mgr.get_or_create(agent.agent_id).unwrap();

        let section = generate_california_ab316_section(&[agent], &trail, &id_mgr);

        let oversight = section
            .controls
            .iter()
            .find(|c| c.control_id == "AB316-OVER")
            .unwrap();
        assert!(matches!(
            oversight.status,
            ControlStatus::PartiallyMet { .. }
        ));
    }

    #[test]
    fn full_report_has_four_frameworks() {
        let agent = test_agent("full-test", vec!["audit.read"]);
        let trail = trail_with_events();
        let mut id_mgr = IdentityManager::in_memory();
        id_mgr.get_or_create(agent.agent_id).unwrap();

        let report = generate_full_compliance_report(&FullReportConfig {
            audit_trail: &trail,
            agents: &[agent],
            identity_manager: &id_mgr,
            capabilities_configured: true,
            hitl_enabled: true,
            fuel_tracking_enabled: true,
            encryption_enabled: true,
            organization: "Multi-Org",
            period_start: 0,
            period_end: u64::MAX,
        });

        assert_eq!(report.sections.len(), 4);
        let frameworks: Vec<&str> = report
            .sections
            .iter()
            .map(|s| s.framework.as_str())
            .collect();
        assert_eq!(
            frameworks,
            vec!["SOC2 Type II", "EU AI Act", "HIPAA", "California AB 316"]
        );
    }
}
