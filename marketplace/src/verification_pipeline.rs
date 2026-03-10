//! Automated agent verification pipeline for marketplace acceptance.
//!
//! Runs 6 checks in order before allowing an agent into the marketplace:
//! 1. Signature check — Ed25519 signature on the bundle
//! 2. Manifest validation — required fields + known capabilities
//! 3. Sandbox test — parse manifest and verify it runs without crashing
//! 4. Security scan — static analysis via existing `SecurityScanner`
//! 5. Capability audit — flag High/Critical risk capabilities (EU AI Act)
//! 6. Governance check — autonomy level appropriate for capabilities

use crate::package::{verify_package, MarketplaceError, SignedPackageBundle};
use crate::scanner::{scan_package, SafetyScanReport};
use crate::trust::{CapabilityRisk, TrustSystem};
use nexus_kernel::autonomy::AutonomyLevel;
use nexus_kernel::manifest::parse_manifest;
use nexus_kernel::permissions::PermissionRiskLevel;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Overall verdict for the verification pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// All checks passed — agent may be listed.
    Approved,
    /// Some checks raised flags that require human review.
    ConditionalApproval,
    /// Hard failures — agent must not be listed.
    Rejected,
}

/// Result of a single check step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub passed: bool,
    pub findings: Vec<String>,
}

/// Full result of the verification pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationResult {
    pub checks: Vec<CheckResult>,
    pub verdict: Verdict,
}

impl VerificationResult {
    /// True if overall verdict allows listing (Approved or ConditionalApproval).
    pub fn may_list(&self) -> bool {
        matches!(
            self.verdict,
            Verdict::Approved | Verdict::ConditionalApproval
        )
    }
}

// ---------------------------------------------------------------------------
// Capability risk mapping (permission-dashboard based)
// ---------------------------------------------------------------------------

/// Classify a single capability using the kernel's permission risk model.
fn permission_risk(capability: &str) -> PermissionRiskLevel {
    match capability {
        "audit.read" => PermissionRiskLevel::Low,
        "fs.read" | "web.search" | "web.read" | "llm.query" => PermissionRiskLevel::Medium,
        "fs.write" | "social.post" | "social.x.post" | "social.x.read" | "messaging.send"
        | "screen.capture" | "input.keyboard" => PermissionRiskLevel::High,
        "process.exec" | "shell.exec" => PermissionRiskLevel::Critical,
        _ => PermissionRiskLevel::Medium,
    }
}

/// Maximum autonomy level allowed for a given permission risk.
fn max_autonomy_for_risk(risk: PermissionRiskLevel) -> AutonomyLevel {
    match risk {
        PermissionRiskLevel::Low => AutonomyLevel::L5,
        PermissionRiskLevel::Medium => AutonomyLevel::L4,
        PermissionRiskLevel::High => AutonomyLevel::L2,
        PermissionRiskLevel::Critical => AutonomyLevel::L1,
    }
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

/// Run the full 6-step verification pipeline on a signed package bundle.
pub fn verify_bundle(bundle: &SignedPackageBundle) -> VerificationResult {
    let mut checks = Vec::with_capacity(6);
    let mut has_rejection = false;
    let mut has_flag = false;

    // 1. Signature check
    let sig_check = check_signature(bundle);
    if !sig_check.passed {
        has_rejection = true;
    }
    checks.push(sig_check);

    // 2. Manifest validation
    let manifest_check = check_manifest(bundle);
    if !manifest_check.passed {
        has_rejection = true;
    }
    checks.push(manifest_check);

    // 3. Sandbox test
    let sandbox_check = check_sandbox(bundle);
    if !sandbox_check.passed {
        has_rejection = true;
    }
    checks.push(sandbox_check);

    // 4. Security scan
    let security_check = check_security_scan(bundle);
    if !security_check.passed {
        has_flag = true;
    }
    checks.push(security_check);

    // 5. Capability audit
    let cap_check = check_capability_audit(bundle);
    if !cap_check.passed {
        has_flag = true;
    }
    checks.push(cap_check);

    // 6. Governance check
    let gov_check = check_governance(bundle);
    if !gov_check.passed {
        has_flag = true;
    }
    checks.push(gov_check);

    let verdict = if has_rejection {
        Verdict::Rejected
    } else if has_flag {
        Verdict::ConditionalApproval
    } else {
        Verdict::Approved
    };

    VerificationResult { checks, verdict }
}

// ---------------------------------------------------------------------------
// Individual checks
// ---------------------------------------------------------------------------

/// Step 1: Verify Ed25519 signature on the wasm bundle.
fn check_signature(bundle: &SignedPackageBundle) -> CheckResult {
    match verify_package(bundle) {
        Ok(()) => CheckResult {
            name: "signature_check".to_string(),
            passed: true,
            findings: vec![],
        },
        Err(e) => CheckResult {
            name: "signature_check".to_string(),
            passed: false,
            findings: vec![format!("Signature verification failed: {e}")],
        },
    }
}

/// Step 2: Validate manifest has required fields and capabilities are from registry.
fn check_manifest(bundle: &SignedPackageBundle) -> CheckResult {
    let mut findings = Vec::new();

    // Parse the TOML manifest
    match parse_manifest(&bundle.manifest_toml) {
        Ok(manifest) => {
            // Verify metadata consistency
            if manifest.name != bundle.metadata.name {
                findings.push(format!(
                    "Manifest name '{}' != metadata name '{}'",
                    manifest.name, bundle.metadata.name
                ));
            }
            if manifest.version != bundle.metadata.version {
                findings.push(format!(
                    "Manifest version '{}' != metadata version '{}'",
                    manifest.version, bundle.metadata.version
                ));
            }
        }
        Err(e) => {
            findings.push(format!("Manifest parse error: {e}"));
        }
    }

    CheckResult {
        name: "manifest_validation".to_string(),
        passed: findings.is_empty(),
        findings,
    }
}

/// Step 3: Parse manifest as a sandbox test — verifies agent code doesn't crash.
///
/// Since we can't run arbitrary Wasm in the marketplace crate without the full
/// sandbox runtime, we simulate this by parsing the manifest, verifying the code
/// is non-empty, and running the behavioral sandbox heuristics from scanner.
fn check_sandbox(bundle: &SignedPackageBundle) -> CheckResult {
    let mut findings = Vec::new();

    if bundle.agent_code.trim().is_empty() {
        findings.push("Agent code is empty".to_string());
    }

    // Parse manifest to verify it would not crash during load
    if let Err(e) = parse_manifest(&bundle.manifest_toml) {
        findings.push(format!("Manifest would crash sandbox on load: {e}"));
    }

    // Run behavioral sandbox heuristic from scanner
    let report = scan_package(bundle);
    if !report.sandbox.passed {
        for finding in &report.sandbox.findings {
            findings.push(format!("Sandbox heuristic: {finding}"));
        }
    }

    CheckResult {
        name: "sandbox_test".to_string(),
        passed: findings.is_empty(),
        findings,
    }
}

/// Step 4: Run existing SecurityScanner static analysis.
fn check_security_scan(bundle: &SignedPackageBundle) -> CheckResult {
    let report: SafetyScanReport = scan_package(bundle);
    let mut findings = Vec::new();

    for pattern in &report.static_analysis.suspicious_patterns {
        findings.push(format!("Suspicious pattern: {pattern}"));
    }

    for finding in &report.policy.findings {
        findings.push(format!("Policy: {finding}"));
    }

    CheckResult {
        name: "security_scan".to_string(),
        // Suspicious patterns flag for review, not hard-reject
        passed: findings.is_empty(),
        findings,
    }
}

/// Step 5: Flag agents requesting High or Critical risk capabilities.
fn check_capability_audit(bundle: &SignedPackageBundle) -> CheckResult {
    let mut findings = Vec::new();

    for cap in &bundle.metadata.capabilities {
        let risk = permission_risk(cap);
        match risk {
            PermissionRiskLevel::Critical => {
                findings.push(format!(
                    "Critical-risk capability '{cap}' requires manual review"
                ));
            }
            PermissionRiskLevel::High => {
                findings.push(format!("High-risk capability '{cap}' flagged for review"));
            }
            _ => {}
        }
    }

    // Also check the trust system's aggregate risk
    let trust_risk = TrustSystem::classify_capability_set(&bundle.metadata.capabilities);
    if trust_risk == CapabilityRisk::High {
        findings.push("Aggregate capability risk: High (EU AI Act review recommended)".to_string());
    }

    CheckResult {
        name: "capability_audit".to_string(),
        passed: findings.is_empty(),
        findings,
    }
}

/// Step 6: Verify the declared autonomy level is appropriate for capabilities.
fn check_governance(bundle: &SignedPackageBundle) -> CheckResult {
    let mut findings = Vec::new();

    let manifest = match parse_manifest(&bundle.manifest_toml) {
        Ok(m) => m,
        Err(_) => {
            // Manifest already failed in step 2, don't double-report
            return CheckResult {
                name: "governance_check".to_string(),
                passed: true,
                findings: vec![],
            };
        }
    };

    let autonomy = AutonomyLevel::from_manifest(manifest.autonomy_level);

    // Find highest-risk capability
    let max_risk = bundle
        .metadata
        .capabilities
        .iter()
        .map(|c| permission_risk(c))
        .fold(PermissionRiskLevel::Low, |acc, r| {
            let rank = |l: PermissionRiskLevel| match l {
                PermissionRiskLevel::Low => 0,
                PermissionRiskLevel::Medium => 1,
                PermissionRiskLevel::High => 2,
                PermissionRiskLevel::Critical => 3,
            };
            if rank(r) > rank(acc) {
                r
            } else {
                acc
            }
        });

    let max_allowed = max_autonomy_for_risk(max_risk);

    if autonomy > max_allowed {
        findings.push(format!(
            "Autonomy {} exceeds maximum {} for {}-risk capabilities",
            autonomy.as_str(),
            max_allowed.as_str(),
            max_risk.as_str(),
        ));
    }

    // L4+ always flagged for governance review regardless of capabilities
    if autonomy >= AutonomyLevel::L4 {
        findings.push(format!(
            "Autonomy {} requires governance board review",
            autonomy.as_str()
        ));
    }

    CheckResult {
        name: "governance_check".to_string(),
        passed: findings.is_empty(),
        findings,
    }
}

// ---------------------------------------------------------------------------
// Verified publish helper
// ---------------------------------------------------------------------------

/// Error from the verified publish flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifiedPublishError {
    Verification(VerificationResult),
    Marketplace(MarketplaceError),
}

impl std::fmt::Display for VerifiedPublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifiedPublishError::Verification(result) => {
                write!(f, "verification failed: {:?}", result.verdict)
            }
            VerifiedPublishError::Marketplace(e) => write!(f, "marketplace error: {e}"),
        }
    }
}

impl std::error::Error for VerifiedPublishError {}

impl From<MarketplaceError> for VerifiedPublishError {
    fn from(e: MarketplaceError) -> Self {
        VerifiedPublishError::Marketplace(e)
    }
}

/// Sign, verify through the pipeline, then publish to the registry.
/// Returns the package ID and verification result on success.
/// Rejects the bundle if the pipeline verdict is `Rejected`.
pub fn verified_publish(
    registry: &mut crate::registry::MarketplaceRegistry,
    package: crate::package::UnsignedPackageBundle,
    author_key: &ed25519_dalek::SigningKey,
) -> Result<(String, VerificationResult), VerifiedPublishError> {
    use crate::package::sign_package;

    let signed = sign_package(package, author_key)?;
    let result = verify_bundle(&signed);

    if result.verdict == Verdict::Rejected {
        return Err(VerifiedPublishError::Verification(result));
    }

    let id = signed.package_id.clone();
    registry.upsert_signed(signed);
    Ok((id, result))
}

/// Sign, verify through the pipeline, then publish to the SQLite registry.
/// Returns the package ID and verification result on success.
/// Rejects the bundle if the pipeline verdict is `Rejected`.
pub fn verified_publish_sqlite(
    registry: &crate::sqlite_registry::SqliteRegistry,
    package: crate::package::UnsignedPackageBundle,
    author_key: &ed25519_dalek::SigningKey,
) -> Result<(String, VerificationResult), VerifiedPublishError> {
    use crate::package::sign_package;
    use crate::sqlite_registry::SqliteRegistryError;

    let signed = sign_package(package, author_key)?;
    let result = verify_bundle(&signed);

    if result.verdict == Verdict::Rejected {
        return Err(VerifiedPublishError::Verification(result));
    }

    let id = signed.package_id.clone();
    registry.upsert_signed(&signed).map_err(|e| match e {
        SqliteRegistryError::Marketplace(me) => VerifiedPublishError::Marketplace(me),
        other => VerifiedPublishError::Marketplace(MarketplaceError::SerializationError(
            other.to_string(),
        )),
    })?;
    Ok((id, result))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::{create_unsigned_bundle, sign_package, PackageMetadata};
    use ed25519_dalek::SigningKey;

    fn test_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn make_metadata(name: &str, caps: Vec<String>) -> PackageMetadata {
        PackageMetadata {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: format!("A {name} agent"),
            capabilities: caps.clone(),
            tags: vec!["test".to_string()],
            author_id: "dev-alice".to_string(),
        }
    }

    fn make_manifest_toml(name: &str, caps: &[&str], autonomy: Option<u8>) -> String {
        let caps_str = caps
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let mut s = format!(
            "name = \"{name}\"\nversion = \"1.0.0\"\ncapabilities = [{caps_str}]\nfuel_budget = 10000\n"
        );
        if let Some(level) = autonomy {
            s.push_str(&format!("autonomy_level = {level}\n"));
        }
        s
    }

    fn make_clean_bundle() -> SignedPackageBundle {
        let caps = vec!["llm.query".to_string(), "web.search".to_string()];
        let manifest = make_manifest_toml("clean-agent", &["llm.query", "web.search"], None);
        let metadata = make_metadata("clean-agent", caps);
        let unsigned = create_unsigned_bundle(
            &manifest,
            "fn run() { /* safe code */ }",
            metadata,
            "local://clean-agent",
            "nexus-test",
        )
        .unwrap();
        sign_package(unsigned, &test_key()).unwrap()
    }

    // -----------------------------------------------------------------------
    // Core tests
    // -----------------------------------------------------------------------

    #[test]
    fn clean_agent_passes_all_checks() {
        let bundle = make_clean_bundle();
        let result = verify_bundle(&bundle);

        assert_eq!(result.verdict, Verdict::Approved);
        assert!(result.may_list());
        for check in &result.checks {
            assert!(check.passed, "check '{}' should pass", check.name);
        }
        assert_eq!(result.checks.len(), 6);
    }

    #[test]
    fn unsigned_bundle_rejected() {
        let caps = vec!["llm.query".to_string()];
        let manifest = make_manifest_toml("bad-sig", &["llm.query"], None);
        let metadata = make_metadata("bad-sig", caps);
        let unsigned = create_unsigned_bundle(
            &manifest,
            "fn run() {}",
            metadata,
            "local://bad-sig",
            "nexus-test",
        )
        .unwrap();
        let mut signed = sign_package(unsigned, &test_key()).unwrap();

        // Corrupt the signature
        if signed.signature.is_empty() {
            signed.signature = vec![0u8; 64];
        } else {
            signed.signature[0] ^= 0xff;
        }

        let result = verify_bundle(&signed);
        assert_eq!(result.verdict, Verdict::Rejected);
        assert!(!result.may_list());
        assert!(!result.checks[0].passed); // signature_check
        assert!(result.checks[0].findings[0].contains("Signature"));
    }

    #[test]
    fn crashing_wasm_rejected() {
        let caps = vec!["llm.query".to_string()];
        let metadata = make_metadata("crash-agent", caps);
        // Invalid manifest TOML that won't parse — simulates a crashing agent
        let bad_manifest = "this is not valid toml [[[";
        let unsigned = create_unsigned_bundle(
            bad_manifest,
            "fn run() {}",
            metadata,
            "local://crash-agent",
            "nexus-test",
        )
        .unwrap();
        let signed = sign_package(unsigned, &test_key()).unwrap();

        let result = verify_bundle(&signed);
        assert_eq!(result.verdict, Verdict::Rejected);
        // manifest_validation should fail
        assert!(!result.checks[1].passed);
        // sandbox_test should also fail (can't parse manifest)
        assert!(!result.checks[2].passed);
    }

    #[test]
    fn overcapability_agent_flagged_for_review() {
        // Use capabilities from the kernel registry that are high/critical risk
        let caps = vec![
            "llm.query".to_string(),
            "fs.write".to_string(),
            "process.exec".to_string(),
            "social.post".to_string(),
        ];
        let manifest = make_manifest_toml(
            "overcap-agent",
            &["llm.query", "fs.write", "process.exec", "social.post"],
            None,
        );
        let metadata = make_metadata("overcap-agent", caps);
        let unsigned = create_unsigned_bundle(
            &manifest,
            "fn run() { /* lots of power */ }",
            metadata,
            "local://overcap-agent",
            "nexus-test",
        )
        .unwrap();
        let signed = sign_package(unsigned, &test_key()).unwrap();

        let result = verify_bundle(&signed);
        assert_eq!(result.verdict, Verdict::ConditionalApproval);
        assert!(result.may_list());

        // capability_audit should flag high-risk and critical-risk caps
        let cap_check = result
            .checks
            .iter()
            .find(|c| c.name == "capability_audit")
            .unwrap();
        assert!(!cap_check.passed);
        assert!(cap_check
            .findings
            .iter()
            .any(|f| f.contains("High-risk") || f.contains("Critical-risk")));
    }

    #[test]
    fn high_autonomy_with_dangerous_caps_flagged() {
        let caps = vec!["fs.write".to_string(), "process.exec".to_string()];
        // L4 autonomy with critical capabilities
        let manifest = make_manifest_toml("auto-agent", &["fs.write", "process.exec"], Some(4));
        let metadata = make_metadata("auto-agent", caps);
        let unsigned = create_unsigned_bundle(
            &manifest,
            "fn run() {}",
            metadata,
            "local://auto-agent",
            "nexus-test",
        )
        .unwrap();
        let signed = sign_package(unsigned, &test_key()).unwrap();

        let result = verify_bundle(&signed);
        assert_eq!(result.verdict, Verdict::ConditionalApproval);

        let gov_check = result
            .checks
            .iter()
            .find(|c| c.name == "governance_check")
            .unwrap();
        assert!(!gov_check.passed);
        assert!(gov_check.findings.iter().any(|f| f.contains("exceeds")));
    }

    #[test]
    fn suspicious_code_flagged() {
        let caps = vec!["llm.query".to_string()];
        let manifest = make_manifest_toml("sketchy-agent", &["llm.query"], None);
        let metadata = make_metadata("sketchy-agent", caps);
        let unsigned = create_unsigned_bundle(
            &manifest,
            "fn run() { std::process::Command::new(\"rm -rf /\"); }",
            metadata,
            "local://sketchy-agent",
            "nexus-test",
        )
        .unwrap();
        let signed = sign_package(unsigned, &test_key()).unwrap();

        let result = verify_bundle(&signed);
        assert_eq!(result.verdict, Verdict::ConditionalApproval);

        let sec_check = result
            .checks
            .iter()
            .find(|c| c.name == "security_scan")
            .unwrap();
        assert!(!sec_check.passed);
        assert!(sec_check
            .findings
            .iter()
            .any(|f| f.contains("std::process::Command")));
    }

    #[test]
    fn empty_code_rejected_by_sandbox() {
        let caps = vec!["llm.query".to_string()];
        let manifest = make_manifest_toml("empty-agent", &["llm.query"], None);
        let metadata = make_metadata("empty-agent", caps);
        let unsigned = create_unsigned_bundle(
            &manifest,
            "   ",
            metadata,
            "local://empty-agent",
            "nexus-test",
        )
        .unwrap();
        let signed = sign_package(unsigned, &test_key()).unwrap();

        let result = verify_bundle(&signed);
        assert_eq!(result.verdict, Verdict::Rejected);

        let sandbox = result
            .checks
            .iter()
            .find(|c| c.name == "sandbox_test")
            .unwrap();
        assert!(!sandbox.passed);
        assert!(sandbox.findings.iter().any(|f| f.contains("empty")));
    }

    #[test]
    fn verified_publish_rejects_bad_bundle() {
        let mut registry = crate::registry::MarketplaceRegistry::new();
        let caps = vec!["llm.query".to_string()];
        let metadata = make_metadata("bad-manifest", caps);
        let unsigned = create_unsigned_bundle(
            "not valid toml [[[",
            "fn run() {}",
            metadata,
            "local://bad",
            "nexus-test",
        )
        .unwrap();

        let result = verified_publish(&mut registry, unsigned, &test_key());
        assert!(result.is_err());
        if let Err(VerifiedPublishError::Verification(vr)) = result {
            assert_eq!(vr.verdict, Verdict::Rejected);
        } else {
            panic!("expected verification error");
        }
    }

    #[test]
    fn verified_publish_accepts_clean_bundle() {
        let mut registry = crate::registry::MarketplaceRegistry::new();
        let caps = vec!["llm.query".to_string(), "web.search".to_string()];
        let manifest = make_manifest_toml("pub-agent", &["llm.query", "web.search"], None);
        let metadata = make_metadata("pub-agent", caps);
        let unsigned = create_unsigned_bundle(
            &manifest,
            "fn run() { /* safe */ }",
            metadata,
            "local://pub-agent",
            "nexus-test",
        )
        .unwrap();

        let (pkg_id, vr) = verified_publish(&mut registry, unsigned, &test_key()).unwrap();
        assert_eq!(vr.verdict, Verdict::Approved);
        assert!(registry.get(&pkg_id).is_some());
    }

    #[test]
    fn loop_code_fails_sandbox() {
        let caps = vec!["llm.query".to_string()];
        let manifest = make_manifest_toml("loop-agent", &["llm.query"], None);
        let metadata = make_metadata("loop-agent", caps);
        let unsigned = create_unsigned_bundle(
            &manifest,
            "fn run() { loop { /* infinite */ } }",
            metadata,
            "local://loop-agent",
            "nexus-test",
        )
        .unwrap();
        let signed = sign_package(unsigned, &test_key()).unwrap();

        let result = verify_bundle(&signed);
        // sandbox heuristic detects "loop {"
        let sandbox = result
            .checks
            .iter()
            .find(|c| c.name == "sandbox_test")
            .unwrap();
        assert!(!sandbox.passed);
        assert!(sandbox
            .findings
            .iter()
            .any(|f| f.contains("non-terminating")));
    }

    #[test]
    fn all_six_check_names_present() {
        let bundle = make_clean_bundle();
        let result = verify_bundle(&bundle);
        let names: Vec<&str> = result.checks.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "signature_check",
                "manifest_validation",
                "sandbox_test",
                "security_scan",
                "capability_audit",
                "governance_check",
            ]
        );
    }
}
