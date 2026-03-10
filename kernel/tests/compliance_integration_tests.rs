//! Integration tests for Phase 7.3 — Compliance, Erasure & Provenance
//!
//! Tests verify the full compliance pipeline: EU AI Act risk classification,
//! transparency reporting, GDPR Article 17 cryptographic erasure, data
//! provenance tracking, retention policy enforcement, and the compliance
//! monitor's continuous governance checks.

use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::compliance::data_governance::{
    AgentDataEraser, DataClass, ErasureError, RetentionPolicy,
};
use nexus_kernel::compliance::eu_ai_act::{EuAiActRiskTier, RiskClassifier};
use nexus_kernel::compliance::monitor::{
    AgentSnapshot, AlertSeverity, ComplianceMonitor, OverallStatus,
};
use nexus_kernel::compliance::provenance::{
    DataClassification, DataOrigin, ProvenanceTracker, TransformationKind,
};
use nexus_kernel::compliance::transparency::TransparencyReportGenerator;
use nexus_kernel::identity::agent_identity::IdentityManager;
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::permissions::PermissionManager;
use nexus_kernel::privacy::PrivacyManager;
use serde_json::json;
use uuid::Uuid;

// ── Helpers ─────────────────────────────────────────────────────────────────

fn base_manifest(name: &str, caps: Vec<&str>) -> AgentManifest {
    AgentManifest {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        capabilities: caps.into_iter().map(String::from).collect(),
        fuel_budget: 5000,
        autonomy_level: None,
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        llm_model: Some("claude-sonnet-4-5".to_string()),
        fuel_period_id: None,
        monthly_fuel_cap: None,
        allowed_endpoints: None,
        domain_tags: vec![],
    }
}

fn running_agent(name: &str, caps: Vec<&str>) -> AgentSnapshot {
    AgentSnapshot {
        agent_id: Uuid::new_v4(),
        manifest: base_manifest(name, caps),
        running: true,
    }
}

fn populate_trail(agent_id: Uuid, trail: &mut AuditTrail, count: usize) {
    for i in 0..count {
        trail
            .append_event(
                agent_id,
                EventType::ToolCall,
                json!({"tool": "test", "index": i}),
            )
            .expect("audit: fail-closed");
    }
}

// ── Test 1: Agent classified correct EU AI Act tier ─────────────────────────

#[test]
fn agent_classified_correct_eu_ai_act_tier() {
    let classifier = RiskClassifier::new();

    // Minimal — read-only agent
    let minimal = base_manifest("reader", vec!["audit.read", "fs.read"]);
    assert_eq!(
        classifier.classify_agent(&minimal).tier,
        EuAiActRiskTier::Minimal
    );

    // Limited — LLM-capable agent
    let limited = base_manifest("chatbot", vec!["llm.query"]);
    assert_eq!(
        classifier.classify_agent(&limited).tier,
        EuAiActRiskTier::Limited
    );

    // High — write + network capabilities
    let high = base_manifest("deployer", vec!["fs.write", "web.search", "llm.query"]);
    assert_eq!(classifier.classify_agent(&high).tier, EuAiActRiskTier::High);

    // High — autonomy L4
    let mut autonomous = base_manifest("auto-agent", vec!["llm.query"]);
    autonomous.autonomy_level = Some(4);
    assert_eq!(
        classifier.classify_agent(&autonomous).tier,
        EuAiActRiskTier::High
    );

    // High — critical-infrastructure domain tag
    let mut infra = base_manifest("infra-agent", vec!["audit.read"]);
    infra.domain_tags = vec!["critical-infrastructure".to_string()];
    assert_eq!(
        classifier.classify_agent(&infra).tier,
        EuAiActRiskTier::High
    );

    // Unacceptable — biometric domain tag
    let mut bio = base_manifest("bio-scanner", vec!["fs.read"]);
    bio.domain_tags = vec!["biometric".to_string()];
    assert_eq!(
        classifier.classify_agent(&bio).tier,
        EuAiActRiskTier::Unacceptable
    );

    // Unacceptable — social-scoring domain tag
    let mut social = base_manifest("scorer", vec!["llm.query"]);
    social.domain_tags = vec!["social-scoring".to_string()];
    assert_eq!(
        classifier.classify_agent(&social).tier,
        EuAiActRiskTier::Unacceptable
    );
}

// ── Test 2: Unacceptable agent rejected at spawn ────────────────────────────

#[test]
fn unacceptable_agent_rejected_at_spawn() {
    let classifier = RiskClassifier::new();

    // Biometric agent must be rejected
    let mut manifest = base_manifest("face-scanner", vec!["fs.read", "llm.query"]);
    manifest.domain_tags = vec!["biometric".to_string()];
    let result = classifier.may_deploy(&manifest);
    assert!(result.is_err(), "Unacceptable agent must be rejected");
    let profile = result.unwrap_err();
    assert_eq!(profile.tier, EuAiActRiskTier::Unacceptable);
    assert!(profile.justification.contains("Article 5"));

    // Non-prohibited agent is accepted
    let safe = base_manifest("safe-agent", vec!["audit.read"]);
    let result = classifier.may_deploy(&safe);
    assert!(result.is_ok(), "Safe agent must be accepted");
    assert_eq!(result.unwrap().tier, EuAiActRiskTier::Minimal);
}

// ── Test 3: Transparency report has all Article 13 fields ───────────────────

#[test]
fn transparency_report_has_all_article_13_fields() {
    let agent_id = Uuid::new_v4();
    let mut manifest = base_manifest(
        "compliance-agent",
        vec!["llm.query", "fs.read", "web.search"],
    );
    manifest.autonomy_level = Some(2);

    let mut trail = AuditTrail::new();
    // LLM calls
    for i in 0..3 {
        trail
            .append_event(
                agent_id,
                EventType::LlmCall,
                json!({"prompt": format!("query {}", i), "model": "claude-sonnet-4-5"}),
            )
            .expect("audit: fail-closed");
    }
    // Tool call
    trail
        .append_event(
            agent_id,
            EventType::ToolCall,
            json!({"tool": "web.search", "query": "compliance"}),
        )
        .expect("audit: fail-closed");
    // Approval event
    trail
        .append_event(
            agent_id,
            EventType::UserAction,
            json!({"verdict": "approved", "operation": "fs.read"}),
        )
        .expect("audit: fail-closed");

    let did = "did:key:z6MkTestCompliance";
    let gen = TransparencyReportGenerator::new();
    let report = gen.generate(&manifest, Some(did), &trail, agent_id);

    // Identity fields
    assert_eq!(report.agent_name, "compliance-agent");
    assert_eq!(report.agent_did, Some(did.to_string()));
    assert_eq!(report.report_version, "1.0.0");

    // Risk classification
    assert!(!report.risk_tier.is_empty());
    assert!(!report.risk_justification.is_empty());
    assert!(!report.applicable_articles.is_empty());
    assert!(!report.required_controls.is_empty());

    // Capabilities with risk levels
    assert_eq!(report.capabilities.len(), 3);
    for cap in &report.capabilities {
        assert!(!cap.capability.is_empty());
        assert!(!cap.risk_level.is_empty());
    }

    // Autonomy
    assert_eq!(report.autonomy_level, "L2");
    assert!(report.autonomy_description.contains("approval"));

    // Data processing summary
    assert_eq!(report.data_processing.total_events, 5);
    assert_eq!(report.data_processing.llm_calls, 3);
    assert_eq!(report.data_processing.tool_calls, 1);
    assert_eq!(report.data_processing.user_actions, 1);

    // Human oversight
    assert_eq!(report.human_oversight.total_approvals, 1);
    assert_eq!(report.human_oversight.approval_rate_percent, 100);

    // Model info
    assert_eq!(
        report.model_info.configured_model,
        Some("claude-sonnet-4-5".to_string())
    );
    assert_eq!(report.model_info.provider_type, "cloud (Anthropic)");

    // Resource usage
    assert_eq!(report.resource_usage.fuel_budget, 5000);
    assert_eq!(report.resource_usage.audit_events_generated, 5);

    // Timestamp
    assert!(report.generated_at_unix > 0);

    // JSON and Markdown rendering
    let json_str = report.to_json();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
    assert!(parsed.is_object());
    assert!(parsed.get("risk_tier").is_some());

    let md = report.to_markdown();
    assert!(md.contains("# Transparency Report:"));
    assert!(md.contains("## Risk Classification"));
    assert!(md.contains("## Granted Capabilities"));
    assert!(md.contains("## Autonomy Level"));
    assert!(md.contains("## Data Processing Summary"));
    assert!(md.contains("## Human Oversight"));
    assert!(md.contains("## Model Information"));
    assert!(md.contains("## Resource Usage"));
}

// ── Test 4: Cryptographic erasure removes all agent data ────────────────────

#[test]
fn cryptographic_erasure_removes_all_agent_data() {
    let agent_id = Uuid::new_v4();
    let other_agent = Uuid::new_v4();
    let mut trail = AuditTrail::new();

    // Populate events for both agents
    populate_trail(agent_id, &mut trail, 5);
    populate_trail(other_agent, &mut trail, 3);

    let mut privacy = PrivacyManager::new();
    let key_id = format!("agent-key-{}", agent_id);
    let key = nexus_kernel::privacy::UserKey {
        id: key_id.clone(),
        bytes: [42u8; 32],
    };
    let encrypted = privacy.encrypt_field(b"sensitive-data", &key).unwrap();

    let mut identity_mgr = IdentityManager::in_memory();
    identity_mgr.get_or_create(agent_id).unwrap();
    identity_mgr.get_or_create(other_agent).unwrap();

    let mut perm_mgr = PermissionManager::new();

    let eraser = AgentDataEraser::new();
    let receipt = eraser
        .erase_agent_data(
            agent_id,
            std::slice::from_ref(&key_id),
            &mut trail,
            &mut privacy,
            &mut identity_mgr,
            &mut perm_mgr,
        )
        .unwrap();

    // All 5 agent events were redacted
    assert_eq!(receipt.events_redacted, 5);
    assert_eq!(receipt.keys_destroyed, vec![key_id.clone()]);
    assert!(receipt.identity_purged);
    assert!(receipt.permissions_purged);

    // Verify agent events are redacted
    for event in trail.events() {
        if event.agent_id == agent_id {
            assert_eq!(event.payload["redacted"], true);
            assert_eq!(event.payload["reason"], "GDPR Article 17 erasure");
        }
    }

    // Other agent's events are untouched
    for event in trail.events() {
        if event.agent_id == other_agent {
            assert!(
                event.payload.get("tool").is_some(),
                "other agent's events must be intact"
            );
        }
    }

    // Agent identity is gone
    assert!(identity_mgr.get(&agent_id).is_none());
    // Other agent still exists
    assert!(identity_mgr.get(&other_agent).is_some());

    // Decryption should fail (key destroyed)
    assert!(privacy.decrypt_field(&encrypted, &key).is_err());
}

// ── Test 5: Erasure proof event logged ──────────────────────────────────────

#[test]
fn erasure_proof_event_logged() {
    let agent_id = Uuid::new_v4();
    let mut trail = AuditTrail::new();
    populate_trail(agent_id, &mut trail, 3);

    let mut privacy = PrivacyManager::new();
    let mut identity_mgr = IdentityManager::in_memory();
    let mut perm_mgr = PermissionManager::new();

    let eraser = AgentDataEraser::new();
    let receipt = eraser
        .erase_agent_data(
            agent_id,
            &[],
            &mut trail,
            &mut privacy,
            &mut identity_mgr,
            &mut perm_mgr,
        )
        .unwrap();

    // Find the proof event
    let proof_event = trail
        .events()
        .iter()
        .find(|e| e.event_id == receipt.proof_event_id)
        .expect("proof event must exist");

    // Proof event is logged under system UUID (not the erased agent)
    assert_eq!(proof_event.agent_id, Uuid::nil());
    assert_eq!(
        proof_event.payload["event"],
        "gdpr.article17.erasure_completed"
    );
    assert_eq!(proof_event.payload["erased_agent_id"], agent_id.to_string());
    assert_eq!(proof_event.payload["events_redacted"], 3);
    assert_eq!(proof_event.payload["identity_purged"], false); // no identity was created
    assert_eq!(proof_event.payload["permissions_purged"], true);

    // Proof event's timestamp matches receipt
    assert_eq!(receipt.erased_at, proof_event.payload["erased_at"]);
}

// ── Test 6: Legal hold prevents erasure ─────────────────────────────────────

#[test]
fn legal_hold_prevents_erasure() {
    let agent_id = Uuid::new_v4();
    let mut trail = AuditTrail::new();
    populate_trail(agent_id, &mut trail, 3);

    let mut privacy = PrivacyManager::new();
    let mut identity_mgr = IdentityManager::in_memory();
    identity_mgr.get_or_create(agent_id).unwrap();
    let mut perm_mgr = PermissionManager::new();

    let mut eraser = AgentDataEraser::new();
    eraser.set_legal_hold(agent_id);

    // Erasure must fail
    let result = eraser.erase_agent_data(
        agent_id,
        &[],
        &mut trail,
        &mut privacy,
        &mut identity_mgr,
        &mut perm_mgr,
    );
    assert!(result.is_err());
    assert!(
        matches!(result.unwrap_err(), ErasureError::LegalHold(id) if id == agent_id),
        "error must be LegalHold with the correct agent ID"
    );

    // Data must be intact
    assert!(identity_mgr.get(&agent_id).is_some());
    for event in trail.events() {
        if event.agent_id == agent_id {
            assert!(
                event.payload.get("tool").is_some(),
                "held agent events must be intact"
            );
        }
    }

    // Release hold and retry
    eraser.release_legal_hold(&agent_id);
    let receipt = eraser
        .erase_agent_data(
            agent_id,
            &[],
            &mut trail,
            &mut privacy,
            &mut identity_mgr,
            &mut perm_mgr,
        )
        .unwrap();
    assert_eq!(receipt.events_redacted, 3);
}

// ── Test 7: Retention policy purges expired data ────────────────────────────

#[test]
fn retention_policy_purges_expired_data() {
    let agent_id = Uuid::new_v4();
    let held_agent = Uuid::new_v4();
    let mut trail = AuditTrail::new();

    // Create events for both agents
    populate_trail(agent_id, &mut trail, 4);
    populate_trail(held_agent, &mut trail, 2);

    // Age all events to make them expired
    for event in trail.events_mut() {
        event.timestamp = 0; // very old
    }

    // Add one recent event for the free agent
    trail
        .append_event(
            agent_id,
            EventType::StateChange,
            json!({"status": "recent"}),
        )
        .expect("audit: fail-closed");

    let mut policy = RetentionPolicy::new();
    policy.set_retention(DataClass::AuditEvents, 1); // 1-second retention
    policy.set_legal_hold(held_agent);

    let result = policy.check_retention(&mut trail);

    // 4 old events from free agent purged, 2 from held agent preserved
    assert_eq!(result.events_purged, 4);
    assert!(result.agents_held.contains(&held_agent));

    // Verify held agent's events are intact
    for event in trail.events() {
        if event.agent_id == held_agent {
            assert!(
                event.payload.get("tool").is_some(),
                "held agent events must be preserved"
            );
        }
    }

    // Verify recent event is intact
    let recent = trail
        .events()
        .iter()
        .find(|e| {
            e.payload
                .get("status")
                .and_then(|v| v.as_str())
                .is_some_and(|s| s == "recent")
        })
        .expect("recent event must exist");
    assert_eq!(recent.agent_id, agent_id);
}

// ── Test 8: Data provenance tracks full lineage ─────────────────────────────

#[test]
fn data_provenance_tracks_full_lineage() {
    let agent_id = Uuid::new_v4();
    let mut trail = AuditTrail::new();
    let mut tracker = ProvenanceTracker::new();

    // 1. Agent reads a file
    let data_id = tracker.record_origin(
        DataOrigin::FileRead,
        agent_id,
        DataClassification::Internal,
        "config.toml",
        &mut trail,
    );

    // 2. Agent redacts PII
    assert!(tracker.record_transformation(
        data_id,
        TransformationKind::Redacted,
        agent_id,
        "PII removed from config",
        &mut trail,
    ));

    // 3. Agent sends to LLM
    assert!(tracker.record_transformation(
        data_id,
        TransformationKind::SentToLlm,
        agent_id,
        "Sent to claude-sonnet for analysis",
        &mut trail,
    ));

    // 4. Agent writes output
    assert!(tracker.record_transformation(
        data_id,
        TransformationKind::WrittenToOutput,
        agent_id,
        "Analysis written to report.md",
        &mut trail,
    ));

    // Verify full lineage chain
    let lineage = tracker.query_lineage(&data_id).expect("lineage must exist");
    assert_eq!(lineage.origin, DataOrigin::FileRead);
    assert_eq!(lineage.label, "config.toml");
    assert_eq!(lineage.classification, DataClassification::Internal);
    assert_eq!(lineage.transformations.len(), 3);
    assert_eq!(
        lineage.transformations[0].kind,
        TransformationKind::Redacted
    );
    assert_eq!(
        lineage.transformations[1].kind,
        TransformationKind::SentToLlm
    );
    assert_eq!(
        lineage.transformations[2].kind,
        TransformationKind::WrittenToOutput
    );
    assert_eq!(lineage.current_holder, agent_id);

    // Verify audit events were emitted
    let provenance_events: Vec<_> = trail
        .events()
        .iter()
        .filter(|e| {
            e.payload
                .get("event")
                .and_then(|v| v.as_str())
                .is_some_and(|s| s.starts_with("provenance."))
        })
        .collect();
    assert_eq!(
        provenance_events.len(),
        4,
        "1 origin + 3 transformation events"
    );

    // Verify lineage report
    let report = tracker.export_lineage_report(agent_id);
    assert_eq!(report.agent_id, agent_id);
    assert!(!report.lineage_entries.is_empty());
    assert!(report.transformations_applied >= 3);

    // Verify rebuild from audit trail
    let mut rebuilt = ProvenanceTracker::new();
    rebuilt.rebuild_from_audit(&trail);
    let rebuilt_lineage = rebuilt
        .query_lineage(&data_id)
        .expect("rebuilt lineage must exist");
    assert_eq!(rebuilt_lineage.origin, DataOrigin::FileRead);
    assert_eq!(rebuilt_lineage.label, "config.toml");
    assert_eq!(rebuilt_lineage.transformations.len(), 3);
}

// ── Test 9: Provenance records delegation handoff ───────────────────────────

#[test]
fn provenance_records_delegation_handoff() {
    let agent_a = Uuid::new_v4();
    let agent_b = Uuid::new_v4();
    let mut trail = AuditTrail::new();
    let mut tracker = ProvenanceTracker::new();

    // Agent A receives user input
    let data_id = tracker.record_origin(
        DataOrigin::UserInput,
        agent_a,
        DataClassification::Confidential,
        "user query about contracts",
        &mut trail,
    );

    // Agent A summarizes data
    assert!(tracker.record_transformation(
        data_id,
        TransformationKind::Summarized,
        agent_a,
        "Extracted key terms",
        &mut trail,
    ));

    // Agent A delegates to Agent B
    assert!(tracker.record_delegation(
        data_id,
        agent_a,
        agent_b,
        "specialist legal analysis needed",
        &mut trail,
    ));

    // Agent B processes the data
    assert!(tracker.record_transformation(
        data_id,
        TransformationKind::SentToLlm,
        agent_b,
        "Legal analysis via LLM",
        &mut trail,
    ));

    // Verify handoff
    let lineage = tracker.query_lineage(&data_id).unwrap();
    assert_eq!(lineage.current_holder, agent_b);
    assert_eq!(lineage.transformations.len(), 3);

    // Delegation recorded with correct agents
    let delegation = &lineage.transformations[1];
    assert_eq!(delegation.kind, TransformationKind::DelegatedToAgent);
    assert_eq!(delegation.agent_id, agent_a);
    assert!(delegation.description.contains(&agent_b.to_string()));

    // Agent B's transformation recorded
    let llm_call = &lineage.transformations[2];
    assert_eq!(llm_call.kind, TransformationKind::SentToLlm);
    assert_eq!(llm_call.agent_id, agent_b);

    // Both agents appear in lineage reports
    let report_a = tracker.export_lineage_report(agent_a);
    let report_b = tracker.export_lineage_report(agent_b);
    assert!(!report_a.lineage_entries.is_empty());
    assert!(!report_b.lineage_entries.is_empty());
}

// ── Test 10: Compliance monitor detects missing identity ────────────────────

#[test]
fn compliance_monitor_detects_missing_identity() {
    let agent_with_id = running_agent("identified-agent", vec!["audit.read"]);
    let agent_without_id = running_agent("no-id-agent", vec!["llm.query"]);

    let trail = AuditTrail::new();
    let mut id_mgr = IdentityManager::in_memory();
    // Only create identity for the first agent
    id_mgr.get_or_create(agent_with_id.agent_id).unwrap();

    let monitor = ComplianceMonitor::new();
    let status =
        monitor.check_compliance(&[agent_with_id, agent_without_id.clone()], &trail, &id_mgr);

    // Should produce a warning, not a violation
    assert_eq!(status.status, OverallStatus::Warning);
    assert_eq!(status.agents_checked, 2);
    assert!(status.checks_failed > 0);

    // Find the specific alert
    let identity_alert = status
        .alerts
        .iter()
        .find(|a| a.check_id == "MISSING_AGENT_IDENTITY")
        .expect("missing identity alert");
    assert_eq!(identity_alert.severity, AlertSeverity::Warning);
    assert!(identity_alert.message.contains("no-id-agent"));
    assert_eq!(identity_alert.agent_id, Some(agent_without_id.agent_id));
}

// ── Test 11: Compliance monitor detects broken audit chain ──────────────────

#[test]
fn compliance_monitor_detects_broken_audit_chain() {
    let agent = running_agent("normal-agent", vec!["audit.read"]);
    let mut trail = AuditTrail::new();
    let id = Uuid::new_v4();
    trail
        .append_event(id, EventType::StateChange, json!({"test": 1}))
        .expect("audit: fail-closed");
    trail
        .append_event(id, EventType::StateChange, json!({"test": 2}))
        .expect("audit: fail-closed");
    trail
        .append_event(id, EventType::StateChange, json!({"test": 3}))
        .expect("audit: fail-closed");

    // Tamper with the chain
    trail.events_mut()[0].payload = json!({"tampered": true});

    let mut id_mgr = IdentityManager::in_memory();
    id_mgr.get_or_create(agent.agent_id).unwrap();

    let monitor = ComplianceMonitor::new();
    let status = monitor.check_compliance(&[agent], &trail, &id_mgr);

    // Should be a violation
    assert_eq!(status.status, OverallStatus::Violation);
    assert!(status.checks_failed > 0);

    let chain_alert = status
        .alerts
        .iter()
        .find(|a| a.check_id == "AUDIT_CHAIN_BROKEN")
        .expect("chain broken alert");
    assert_eq!(chain_alert.severity, AlertSeverity::Violation);
    assert!(chain_alert.message.contains("tampering"));
}

// ── Test 12: Compliance pipeline covers all SOC2 + EU AI Act controls ───────

#[test]
fn compliance_pipeline_covers_soc2_and_eu_ai_act_controls() {
    // This test verifies that the kernel compliance primitives (ComplianceMonitor,
    // RiskClassifier, TransparencyReportGenerator) together cover all the governance
    // checks needed by SOC2 and EU AI Act frameworks.

    let agent_id = Uuid::new_v4();
    let mut manifest = base_manifest("compliant-agent", vec!["llm.query", "fs.read"]);
    manifest.autonomy_level = Some(2);

    let agent = AgentSnapshot {
        agent_id,
        manifest: manifest.clone(),
        running: true,
    };

    let mut trail = AuditTrail::new();
    // Populate with representative audit events
    trail
        .append_event(
            agent_id,
            EventType::ToolCall,
            json!({"action": "capability_check", "cap": "llm.query"}),
        )
        .expect("audit: fail-closed");
    trail
        .append_event(
            agent_id,
            EventType::LlmCall,
            json!({"prompt": "test", "model": "claude-sonnet-4-5"}),
        )
        .expect("audit: fail-closed");
    trail
        .append_event(
            agent_id,
            EventType::UserAction,
            json!({"verdict": "approved", "operation": "fs.read"}),
        )
        .expect("audit: fail-closed");

    let mut id_mgr = IdentityManager::in_memory();
    id_mgr.get_or_create(agent_id).unwrap();

    // 1. ComplianceMonitor covers SOC2 CC6.1 (access control), CC7.2 (monitoring)
    let monitor = ComplianceMonitor::new();
    let status = monitor.check_compliance(std::slice::from_ref(&agent), &trail, &id_mgr);
    assert_eq!(status.status, OverallStatus::Compliant);
    assert!(
        status.checks_passed >= 5,
        "monitor must run at least 5 checks (unacceptable, high-risk autonomy, \
         audit integrity, agent identities, prompt firewall, retention)"
    );
    assert_eq!(status.checks_failed, 0);

    // 2. RiskClassifier covers EU AI Act Article 5 (prohibited) + Article 6 (tiers)
    let classifier = RiskClassifier::new();
    let profile = classifier.classify_agent(&manifest);
    assert!(
        !profile.justification.is_empty(),
        "risk classification must include justification"
    );
    // L2 + llm.query + fs.read = Limited tier
    assert_eq!(profile.tier, EuAiActRiskTier::Limited);
    assert!(
        !profile.applicable_articles.is_empty(),
        "EU AI Act classification must reference applicable articles"
    );
    assert!(
        !profile.required_controls.is_empty(),
        "classification must specify required controls"
    );

    // 3. Autonomy compliance check covers Article 14 (human oversight)
    assert!(
        RiskClassifier::autonomy_compliant(&manifest, EuAiActRiskTier::High),
        "L2 agent must be autonomy-compliant for High tier"
    );
    // L4 would violate Article 14
    let mut high_auto = manifest.clone();
    high_auto.autonomy_level = Some(4);
    assert!(
        !RiskClassifier::autonomy_compliant(&high_auto, EuAiActRiskTier::High),
        "L4 agent must NOT be autonomy-compliant for High tier"
    );

    // 4. TransparencyReport covers Article 13 (transparency + information)
    let gen = TransparencyReportGenerator::new();
    let did = id_mgr.get(&agent_id).unwrap().did.clone();
    let report = gen.generate(&manifest, Some(&did), &trail, agent_id);
    assert_eq!(report.agent_name, "compliant-agent");
    assert!(report.agent_did.is_some());
    assert!(!report.risk_tier.is_empty());
    assert!(!report.capabilities.is_empty());
    assert!(!report.autonomy_description.is_empty());
    assert!(report.data_processing.total_events > 0);
    assert_eq!(report.human_oversight.total_approvals, 1);
    assert!(report.resource_usage.fuel_budget > 0);

    // 5. JSON + Markdown output completes the transparency obligation
    let json_str = report.to_json();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
    assert!(parsed.get("risk_tier").is_some());
    assert!(parsed.get("applicable_articles").is_some());
    assert!(parsed.get("human_oversight").is_some());

    let md = report.to_markdown();
    assert!(md.contains("## Risk Classification"));
    assert!(md.contains("## Human Oversight"));
    assert!(md.contains("## Resource Usage"));
}
