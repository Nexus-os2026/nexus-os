use super::bundle::{compute_manifest_hash, BundleError, EvidenceBundle};
use super::format::EvidenceFile;
use crate::autonomy::AutonomyLevel;
use crate::manifest::AgentManifest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;

/// Outcome of the five-check verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationReport {
    pub bundle_id: String,
    pub agent_id: String,
    pub run_id: String,

    /// 1. Every event hash recomputes correctly and chains link.
    pub chain_integrity: bool,
    /// 2. manifest_hash matches capabilities declared in policy_snapshot.
    pub manifest_capabilities_match: bool,
    /// 3. fuel_consumed <= fuel_budget.
    pub fuel_within_budget: bool,
    /// 4. All outputs have approval records when autonomy < L3.
    pub approvals_present: bool,
    /// 5. Event timestamps are monotonically non-decreasing.
    pub monotonic_ordering: bool,

    pub verdict: VerificationVerdict,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationVerdict {
    Valid,
    Invalid,
}

/// Run all five verification checks against an evidence bundle.
///
/// Optionally accepts the original `AgentManifest` to cross-check the
/// manifest hash. When `None`, check 2 verifies only that the
/// `policy_snapshot.capabilities` is non-empty.
pub fn verify_bundle(
    bundle: &EvidenceBundle,
    manifest: Option<&AgentManifest>,
) -> VerificationReport {
    let mut issues: Vec<String> = Vec::new();

    // -- Check 1: hash-chain integrity --
    let chain_integrity = check_hash_chain(bundle, &mut issues);

    // -- Check 2: manifest hash matches capabilities --
    let manifest_capabilities_match = check_manifest_capabilities(bundle, manifest, &mut issues);

    // -- Check 3: fuel_consumed <= fuel_budget --
    let fuel_within_budget = check_fuel_budget(bundle, &mut issues);

    // -- Check 4: outputs have approvals when autonomy < L3 --
    let approvals_present = check_approvals(bundle, &mut issues);

    // -- Check 5: monotonic event ordering --
    let monotonic_ordering = check_monotonic_ordering(bundle, &mut issues);

    let verdict = if chain_integrity
        && manifest_capabilities_match
        && fuel_within_budget
        && approvals_present
        && monotonic_ordering
    {
        VerificationVerdict::Valid
    } else {
        VerificationVerdict::Invalid
    };

    VerificationReport {
        bundle_id: bundle.bundle_id.to_string(),
        agent_id: bundle.agent_id.to_string(),
        run_id: bundle.run_id.to_string(),
        chain_integrity,
        manifest_capabilities_match,
        fuel_within_budget,
        approvals_present,
        monotonic_ordering,
        verdict,
        issues,
    }
}

/// Convenience: load from file, verify, return report.
pub fn verify_file(
    path: &Path,
    manifest: Option<&AgentManifest>,
) -> Result<VerificationReport, BundleError> {
    let evidence = EvidenceFile::read_from_file(path)?;
    Ok(verify_bundle(&evidence.bundle, manifest))
}

// ── Check 1: hash-chain integrity ──────────────────────────────────

const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn check_hash_chain(bundle: &EvidenceBundle, issues: &mut Vec<String>) -> bool {
    let mut expected_previous = GENESIS_HASH.to_string();
    let mut valid = true;

    for (idx, event) in bundle.audit_events.iter().enumerate() {
        if event.previous_hash != expected_previous {
            issues.push(format!("chain: event[{idx}] previous_hash mismatch"));
            valid = false;
        }

        let recomputed = recompute_event_hash(event);
        if event.hash != recomputed {
            issues.push(format!(
                "chain: event[{idx}] hash mismatch (tampered payload or metadata)"
            ));
            valid = false;
        }

        expected_previous = event.hash.clone();
    }

    // Also verify chain_root_hash equals last event hash
    let actual_root = bundle
        .audit_events
        .last()
        .map(|e| e.hash.as_str())
        .unwrap_or("");
    if bundle.chain_root_hash != actual_root {
        issues.push("chain: chain_root_hash does not match last event".to_string());
        valid = false;
    }

    // Verify bundle digest
    let recomputed_digest = recompute_bundle_digest(bundle);
    if bundle.bundle_digest != recomputed_digest {
        issues.push("chain: bundle_digest mismatch".to_string());
        valid = false;
    }

    valid
}

fn recompute_event_hash(event: &crate::audit::AuditEvent) -> String {
    #[derive(serde::Serialize)]
    struct CanonicalEventData<'a> {
        event_id: &'a str,
        timestamp: u64,
        agent_id: &'a str,
        event_type: &'a crate::audit::EventType,
        payload: &'a serde_json::Value,
    }

    let event_id_string = event.event_id.to_string();
    let agent_id_string = event.agent_id.to_string();
    let canonical = CanonicalEventData {
        event_id: &event_id_string,
        timestamp: event.timestamp,
        agent_id: &agent_id_string,
        event_type: &event.event_type,
        payload: &event.payload,
    };

    let serialized = match serde_json::to_vec(&canonical) {
        Ok(bytes) => bytes,
        Err(_) => {
            return String::new();
        }
    };

    let mut hasher = Sha256::new();
    hasher.update(event.previous_hash.as_bytes());
    hasher.update(serialized);
    format!("{:x}", hasher.finalize())
}

fn recompute_bundle_digest(bundle: &EvidenceBundle) -> String {
    #[derive(serde::Serialize)]
    struct DigestInput<'a> {
        bundle_id: String,
        agent_id: String,
        run_id: String,
        manifest_hash: &'a str,
        chain_root_hash: &'a str,
        event_count: usize,
        exported_at: u64,
    }

    let input = DigestInput {
        bundle_id: bundle.bundle_id.to_string(),
        agent_id: bundle.agent_id.to_string(),
        run_id: bundle.run_id.to_string(),
        manifest_hash: &bundle.manifest_hash,
        chain_root_hash: &bundle.chain_root_hash,
        event_count: bundle.audit_events.len(),
        exported_at: bundle.exported_at,
    };

    let canonical = match serde_json::to_vec(&input) {
        Ok(bytes) => bytes,
        Err(_) => {
            return String::new();
        }
    };

    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    for event in &bundle.audit_events {
        hasher.update(event.hash.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

// ── Check 2: manifest hash matches capabilities ────────────────────

fn check_manifest_capabilities(
    bundle: &EvidenceBundle,
    manifest: Option<&AgentManifest>,
    issues: &mut Vec<String>,
) -> bool {
    if let Some(m) = manifest {
        let expected_hash = compute_manifest_hash(m);
        if bundle.manifest_hash != expected_hash {
            issues.push(format!(
                "manifest: hash mismatch — bundle='{}…' expected='{}…'",
                &bundle.manifest_hash[..bundle.manifest_hash.len().min(16)],
                &expected_hash[..expected_hash.len().min(16)]
            ));
            return false;
        }

        // Verify the policy snapshot capabilities match the manifest
        let manifest_caps: HashSet<&str> = m.capabilities.iter().map(String::as_str).collect();
        let snapshot_caps: HashSet<&str> = bundle
            .policy_snapshot
            .capabilities
            .iter()
            .map(String::as_str)
            .collect();

        if manifest_caps != snapshot_caps {
            issues.push(
                "manifest: policy_snapshot capabilities do not match manifest capabilities"
                    .to_string(),
            );
            return false;
        }

        return true;
    }

    // Without a manifest, just verify capabilities are non-empty
    if bundle.policy_snapshot.capabilities.is_empty() {
        issues.push("manifest: policy_snapshot has empty capabilities".to_string());
        return false;
    }

    true
}

// ── Check 3: fuel budget ───────────────────────────────────────────

fn check_fuel_budget(bundle: &EvidenceBundle, issues: &mut Vec<String>) -> bool {
    if bundle.fuel_consumed > bundle.fuel_budget {
        issues.push(format!(
            "fuel: consumed ({}) exceeds budget ({})",
            bundle.fuel_consumed, bundle.fuel_budget
        ));
        return false;
    }
    true
}

// ── Check 4: approval records ──────────────────────────────────────

fn check_approvals(bundle: &EvidenceBundle, issues: &mut Vec<String>) -> bool {
    // Only enforce approval checks when autonomy < L3
    if bundle.autonomy_level >= AutonomyLevel::L3 {
        return true;
    }

    if bundle.outputs.is_empty() {
        return true;
    }

    // Collect all output event-ids that have approval records
    let approved_ids: HashSet<String> = bundle
        .approval_records
        .iter()
        .map(|r| r.output_event_id.to_string())
        .collect();

    let mut valid = true;
    for (idx, output) in bundle.outputs.iter().enumerate() {
        let event_id = output
            .get("event_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if event_id.is_empty() || !approved_ids.contains(event_id) {
            issues.push(format!(
                "approvals: output[{idx}] (event_id='{}'…) has no approval record \
                 (autonomy_level={}, requires approval below L3)",
                &event_id[..event_id.len().min(16)],
                bundle.autonomy_level.as_str()
            ));
            valid = false;
        }
    }

    valid
}

// ── Check 5: monotonic event ordering ──────────────────────────────

fn check_monotonic_ordering(bundle: &EvidenceBundle, issues: &mut Vec<String>) -> bool {
    let mut valid = true;
    for window in bundle.audit_events.windows(2) {
        if window[1].timestamp < window[0].timestamp {
            issues.push(format!(
                "ordering: event '{}' timestamp {} precedes previous event timestamp {}",
                window[1].event_id, window[1].timestamp, window[0].timestamp
            ));
            valid = false;
        }
    }
    valid
}

#[cfg(test)]
mod tests {
    use super::{verify_bundle, verify_file, VerificationVerdict};
    use crate::audit::{AuditTrail, EventType};
    use crate::autonomy::AutonomyLevel;
    use crate::consent::{ApprovalDecision, ApprovalVerdict, GovernedOperation};
    use crate::manifest::AgentManifest;
    use crate::replay::bundle::{ApprovalRecord, EvidenceBundle, PolicySnapshot};
    use crate::replay::format::EvidenceFile;
    use serde_json::json;
    use uuid::Uuid;

    fn test_manifest() -> AgentManifest {
        AgentManifest {
            name: "test-agent".to_string(),
            version: "1.0.0".to_string(),
            capabilities: vec![
                "web.search".to_string(),
                "llm.query".to_string(),
                "fs.read".to_string(),
            ],
            fuel_budget: 50_000,
            autonomy_level: Some(3),
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            llm_model: None,
            fuel_period_id: None,
            monthly_fuel_cap: None,
        }
    }

    fn test_policy(manifest: &AgentManifest) -> PolicySnapshot {
        PolicySnapshot {
            autonomy_level: AutonomyLevel::L3,
            consent_tiers: std::collections::BTreeMap::new(),
            capabilities: manifest.capabilities.clone(),
            fuel_budget: manifest.fuel_budget,
        }
    }

    fn make_trail(agent_id: Uuid, count: usize) -> AuditTrail {
        let mut trail = AuditTrail::new();
        for i in 0..count {
            trail.append_event(
                agent_id,
                EventType::ToolCall,
                json!({"tool": "web.search", "seq": i}),
            );
        }
        trail
    }

    fn make_bundle(
        agent_id: Uuid,
        manifest: &AgentManifest,
        autonomy: AutonomyLevel,
        fuel_consumed: u64,
        outputs: Vec<serde_json::Value>,
        approvals: Vec<ApprovalRecord>,
    ) -> EvidenceBundle {
        let trail = make_trail(agent_id, 5);
        let mut policy = test_policy(manifest);
        policy.autonomy_level = autonomy;
        EvidenceBundle::export(
            agent_id,
            Uuid::new_v4(),
            manifest,
            policy,
            &trail,
            vec![json!({"input": "query"})],
            outputs,
            fuel_consumed,
            manifest.fuel_budget,
            autonomy,
            approvals,
        )
        .unwrap()
    }

    // ── Test 1: round-trip (export → serialize → deserialize → verify) ──

    #[test]
    fn round_trip_export_serialize_verify() {
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();
        let bundle = make_bundle(
            agent_id,
            &manifest,
            AutonomyLevel::L3,
            1_000,
            vec![],
            vec![],
        );

        // Serialize to file
        let file = EvidenceFile::from_bundle(bundle.clone());
        let path =
            std::env::temp_dir().join(format!("nexus-rt-test-{}.nexus-evidence", Uuid::new_v4()));
        file.write_to_file(&path).expect("write should succeed");

        // Deserialize and verify
        let report = verify_file(&path, Some(&manifest)).expect("verify_file should succeed");

        assert_eq!(report.verdict, VerificationVerdict::Valid);
        assert!(report.chain_integrity);
        assert!(report.manifest_capabilities_match);
        assert!(report.fuel_within_budget);
        assert!(report.approvals_present);
        assert!(report.monotonic_ordering);
        assert!(report.issues.is_empty());
        assert_eq!(report.run_id, bundle.run_id.to_string());

        let _ = std::fs::remove_file(&path);
    }

    // ── Test 2: tamper detection (mutate an event, verify catches it) ──

    #[test]
    fn tamper_detection_catches_modified_event() {
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();
        let mut bundle = make_bundle(
            agent_id,
            &manifest,
            AutonomyLevel::L3,
            1_000,
            vec![],
            vec![],
        );

        // Tamper with the middle event payload
        bundle.audit_events[2].payload = json!({"tampered": true});

        let report = verify_bundle(&bundle, Some(&manifest));
        assert_eq!(report.verdict, VerificationVerdict::Invalid);
        assert!(!report.chain_integrity);
        assert!(
            report.issues.iter().any(|i| i.contains("hash mismatch")),
            "expected hash mismatch issue, got: {:?}",
            report.issues
        );
    }

    // ── Test 3: fuel overflow (fuel_consumed > fuel_budget) ──

    #[test]
    fn fuel_overflow_detected() {
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();
        // Consume more than budget
        let bundle = make_bundle(
            agent_id,
            &manifest,
            AutonomyLevel::L3,
            999_999,
            vec![],
            vec![],
        );

        let report = verify_bundle(&bundle, Some(&manifest));
        assert_eq!(report.verdict, VerificationVerdict::Invalid);
        assert!(!report.fuel_within_budget);
        assert!(
            report.issues.iter().any(|i| i.contains("exceeds budget")),
            "expected fuel issue, got: {:?}",
            report.issues
        );
    }

    // ── Test 4: missing approvals (autonomy < L3, outputs lack records) ──

    #[test]
    fn missing_approvals_detected_below_l3() {
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();
        let output_event_id = Uuid::new_v4();
        let outputs = vec![json!({
            "event_id": output_event_id.to_string(),
            "action": "social.post",
            "content": "hello world"
        })];

        // Autonomy L2, but no approval records
        let bundle = make_bundle(
            agent_id,
            &manifest,
            AutonomyLevel::L2,
            1_000,
            outputs,
            vec![], // <-- no approvals
        );

        let report = verify_bundle(&bundle, Some(&manifest));
        assert_eq!(report.verdict, VerificationVerdict::Invalid);
        assert!(!report.approvals_present);
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.contains("no approval record")),
            "expected approval issue, got: {:?}",
            report.issues
        );

        // Now provide the approval — should pass
        let output_event_id2 = Uuid::new_v4();
        let outputs2 = vec![json!({
            "event_id": output_event_id2.to_string(),
            "action": "social.post",
            "content": "hello world"
        })];
        let approvals = vec![ApprovalRecord {
            output_event_id: output_event_id2,
            operation: GovernedOperation::SocialPostPublish,
            decision: ApprovalDecision {
                id: "req-1".to_string(),
                approver_id: "human-admin".to_string(),
                decision: ApprovalVerdict::Approve,
                signature: None,
                decision_seq: 1,
            },
        }];

        let bundle_ok = make_bundle(
            agent_id,
            &manifest,
            AutonomyLevel::L2,
            1_000,
            outputs2,
            approvals,
        );

        let report_ok = verify_bundle(&bundle_ok, Some(&manifest));
        assert!(report_ok.approvals_present);
    }
}
