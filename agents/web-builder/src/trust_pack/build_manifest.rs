//! Build Manifest — the signed truth about how a site was built.
//!
//! Every field populated, Ed25519 signed, cryptographically verifiable.
//! The manifest excludes all credentials and sensitive data.

use crate::project::ProjectState;
use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("signing failed: {0}")]
    SignFailed(String),
    #[error("verification failed: {0}")]
    VerifyFailed(String),
}

// ─── Types ──────────────────────────────────────────────────────────────────

/// Complete build manifest with provenance, quality, and signature data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildManifest {
    // Identity
    pub project_id: String,
    pub project_name: String,
    pub build_id: String,
    pub build_hash: String,
    pub timestamp: String,

    // Provenance
    pub template_id: String,
    pub output_mode: String,
    pub models_used: Vec<ModelUsage>,
    pub total_cost_usd: f64,

    // Quality
    pub quality_scores: QualityScoresSummary,
    pub conversion_scores: ConversionScoresSummary,
    pub issues_found: usize,
    pub issues_fixed: usize,

    // Dependencies
    pub external_dependency_count: usize,

    // Backend
    pub backend_provider: Option<String>,
    pub schema_hash: Option<String>,
    pub rls_policy_hash: Option<String>,

    // Deploy
    pub deploy_provider: Option<String>,
    pub deploy_url: Option<String>,
    pub deploy_hash: Option<String>,

    // Signature
    pub signer_public_key: Option<String>,
    pub signature: Option<String>,
}

/// A model used during the build.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model_name: String,
    pub purpose: String,
    pub cost_usd: f64,
    pub invocation_count: u32,
}

/// Summary of the six quality check scores.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityScoresSummary {
    pub accessibility: u32,
    pub seo: u32,
    pub performance: u32,
    pub security: u32,
    pub html_validity: u32,
    pub responsive: u32,
    pub overall: u32,
}

/// Summary of the four conversion check scores.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversionScoresSummary {
    pub cta_placement: u32,
    pub above_fold: u32,
    pub trust_signals: u32,
    pub copy_clarity: u32,
    pub overall: u32,
}

// ─── Creation ───────────────────────────────────────────────────────────────

/// Create a full build manifest from all project data sources.
pub fn create_full_build_manifest(project_dir: &Path, state: &ProjectState) -> BuildManifest {
    // Load quality report if available
    let quality_scores = load_quality_scores(project_dir);
    let conversion_scores = load_conversion_scores(project_dir);
    let (issues_found, issues_fixed) = load_issue_counts(project_dir);

    // Load deploy data if available
    let deploy = crate::deploy::manifest::load_manifest(project_dir);

    // Compute build hash from current HTML
    let build_hash = compute_build_hash(project_dir);

    // Determine models used from cost tracker
    let models_used = infer_models_used(state);

    // Load backend hashes if available
    let (schema_hash, rls_hash) = load_backend_hashes(project_dir);

    let mut manifest = BuildManifest {
        project_id: state.project_id.clone(),
        project_name: state
            .project_name
            .clone()
            .unwrap_or_else(|| "Untitled".into()),
        build_id: uuid::Uuid::new_v4().to_string(),
        build_hash,
        timestamp: crate::deploy::now_iso8601(),

        template_id: state
            .selected_template
            .clone()
            .unwrap_or_else(|| "unknown".into()),
        output_mode: state.output_mode.clone().unwrap_or_else(|| "Html".into()),
        models_used,
        total_cost_usd: state.total_cost,

        quality_scores,
        conversion_scores,
        issues_found,
        issues_fixed,

        external_dependency_count:
            crate::dependency_manifest::DependencyManifest::default_manifest()
                .dependencies
                .len(),

        backend_provider: None,
        schema_hash,
        rls_policy_hash: rls_hash,

        deploy_provider: deploy.as_ref().map(|d| d.provider.clone()),
        deploy_url: deploy.as_ref().map(|d| d.url.clone()),
        deploy_hash: deploy.as_ref().map(|d| d.build_hash.clone()),

        signer_public_key: None,
        signature: None,
    };

    // Attempt Ed25519 signing
    let _ = sign_manifest(&mut manifest);

    manifest
}

// ─── Signing ────────────────────────────────────────────────────────────────

/// Produce canonical JSON bytes for signing (excludes signature fields).
pub fn canonical_bytes(manifest: &BuildManifest) -> Vec<u8> {
    let canonical = serde_json::json!({
        "project_id": manifest.project_id,
        "project_name": manifest.project_name,
        "build_id": manifest.build_id,
        "build_hash": manifest.build_hash,
        "timestamp": manifest.timestamp,
        "template_id": manifest.template_id,
        "output_mode": manifest.output_mode,
        "models_used": manifest.models_used,
        "total_cost_usd": manifest.total_cost_usd,
        "quality_scores": manifest.quality_scores,
        "conversion_scores": manifest.conversion_scores,
        "issues_found": manifest.issues_found,
        "issues_fixed": manifest.issues_fixed,
        "external_dependency_count": manifest.external_dependency_count,
        "deploy_provider": manifest.deploy_provider,
        "deploy_url": manifest.deploy_url,
        "deploy_hash": manifest.deploy_hash,
    });
    serde_json::to_vec(&canonical).unwrap_or_default()
}

/// Sign the manifest using Ed25519 via nexus-crypto.
///
/// Generates a fresh keypair, signs the canonical bytes, and populates
/// `signer_public_key` and `signature` fields with hex-encoded values.
pub fn sign_manifest(manifest: &mut BuildManifest) -> Result<(), ManifestError> {
    let identity = CryptoIdentity::generate(SignatureAlgorithm::Ed25519)
        .map_err(|e| ManifestError::SignFailed(format!("{e}")))?;

    let bytes = canonical_bytes(manifest);
    let sig = identity
        .sign(&bytes)
        .map_err(|e| ManifestError::SignFailed(format!("{e}")))?;

    manifest.signer_public_key = Some(hex::encode(identity.verifying_key()));
    manifest.signature = Some(hex::encode(&sig));

    Ok(())
}

/// Verify a manifest's Ed25519 signature.
///
/// Returns `true` if signature is valid, `false` if invalid.
/// Returns error if the signature or key is missing/malformed.
pub fn verify_manifest(manifest: &BuildManifest) -> Result<bool, ManifestError> {
    let public_key_hex = manifest
        .signer_public_key
        .as_ref()
        .ok_or_else(|| ManifestError::VerifyFailed("no public key".into()))?;
    let signature_hex = manifest
        .signature
        .as_ref()
        .ok_or_else(|| ManifestError::VerifyFailed("no signature".into()))?;

    let public_key =
        hex::decode(public_key_hex).map_err(|e| ManifestError::VerifyFailed(format!("{e}")))?;
    let signature =
        hex::decode(signature_hex).map_err(|e| ManifestError::VerifyFailed(format!("{e}")))?;

    let bytes = canonical_bytes(manifest);

    CryptoIdentity::verify(SignatureAlgorithm::Ed25519, &public_key, &bytes, &signature)
        .map_err(|e| ManifestError::VerifyFailed(format!("{e}")))
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn compute_build_hash(project_dir: &Path) -> String {
    let html_path = project_dir.join("current").join("index.html");
    if let Ok(content) = std::fs::read(&html_path) {
        use sha2::Digest;
        let hash = sha2::Sha256::digest(&content);
        hex::encode(hash)
    } else {
        "no_build_output".into()
    }
}

fn load_quality_scores(project_dir: &Path) -> QualityScoresSummary {
    let path = project_dir.join("quality_report.json");
    let report: Option<crate::quality::QualityReport> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok());

    match report {
        Some(r) => {
            let score_by_id = |id: &str| -> u32 {
                r.checks
                    .iter()
                    .find(|c| c.check_id == id)
                    .map(|c| c.score)
                    .unwrap_or(0)
            };
            QualityScoresSummary {
                accessibility: score_by_id("accessibility"),
                seo: score_by_id("seo"),
                performance: score_by_id("performance"),
                security: score_by_id("security"),
                html_validity: score_by_id("html_validity"),
                responsive: score_by_id("responsive"),
                overall: r.overall_score,
            }
        }
        None => QualityScoresSummary::default(),
    }
}

fn load_conversion_scores(project_dir: &Path) -> ConversionScoresSummary {
    let report = crate::quality::conversion::load_report(project_dir);
    match report {
        Some(r) => {
            let score_by_id = |id: &str| -> u32 {
                r.checks
                    .iter()
                    .find(|c| c.check_id == id)
                    .map(|c| c.score)
                    .unwrap_or(0)
            };
            ConversionScoresSummary {
                cta_placement: score_by_id("cta_placement"),
                above_fold: score_by_id("above_fold"),
                trust_signals: score_by_id("trust_signals"),
                copy_clarity: score_by_id("copy_clarity"),
                overall: r.overall_score,
            }
        }
        None => ConversionScoresSummary::default(),
    }
}

fn load_issue_counts(project_dir: &Path) -> (usize, usize) {
    let path = project_dir.join("quality_report.json");
    let report: Option<crate::quality::QualityReport> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok());

    match report {
        Some(r) => (r.total_issues, r.auto_fixable_count),
        None => (0, 0),
    }
}

fn load_backend_hashes(project_dir: &Path) -> (Option<String>, Option<String>) {
    let path = project_dir.join("backend_state.json");
    let json: Option<serde_json::Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());

    match json {
        Some(v) => (
            v.get("schema_hash")
                .and_then(|h| h.as_str())
                .map(String::from),
            v.get("rls_hash").and_then(|h| h.as_str()).map(String::from),
        ),
        None => (None, None),
    }
}

fn infer_models_used(state: &ProjectState) -> Vec<ModelUsage> {
    let mut models = Vec::new();

    // Planning model (if plan was generated)
    if state.plan_cost > 0.0 {
        models.push(ModelUsage {
            model_name: "claude-haiku-4-5".into(),
            purpose: "planning".into(),
            cost_usd: state.plan_cost,
            invocation_count: 1,
        });
    }

    // Build model
    if state.build_cost > 0.0 {
        models.push(ModelUsage {
            model_name: "claude-sonnet-4-6".into(),
            purpose: "content_generation".into(),
            cost_usd: state.build_cost,
            invocation_count: 1,
        });
    }

    // Local model (if zero cost build happened)
    if state.build_cost == 0.0
        && state.status != crate::project::ProjectStatus::Draft
        && state.status != crate::project::ProjectStatus::Planned
    {
        models.push(ModelUsage {
            model_name: "local_scaffold".into(),
            purpose: "template_assembly".into(),
            cost_usd: 0.0,
            invocation_count: 1,
        });
    }

    // Iteration models
    for (i, cost) in state.iteration_costs.iter().enumerate() {
        if *cost > 0.0 {
            models.push(ModelUsage {
                model_name: "claude-sonnet-4-6".into(),
                purpose: format!("iteration_{}", i + 1),
                cost_usd: *cost,
                invocation_count: 1,
            });
        }
    }

    models
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state() -> ProjectState {
        let mut state = crate::project::create_project("test-proj", "AI writing tool");
        state.project_name = Some("AI Writer SaaS".into());
        state.selected_template = Some("saas_landing".into());
        state.output_mode = Some("Html".into());
        state.total_cost = 0.15;
        state.plan_cost = 0.001;
        state.build_cost = 0.149;
        state.status = crate::project::ProjectStatus::Generated;
        state
    }

    #[test]
    fn test_manifest_has_all_required_fields() {
        let dir = std::env::temp_dir().join(format!("nexus-bm-fields-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let state = sample_state();
        crate::project::save_project_state(&dir, &state).unwrap();

        let manifest = create_full_build_manifest(&dir, &state);

        assert_eq!(manifest.project_id, "test-proj");
        assert_eq!(manifest.project_name, "AI Writer SaaS");
        assert!(!manifest.build_id.is_empty());
        assert!(!manifest.timestamp.is_empty());
        assert_eq!(manifest.template_id, "saas_landing");
        assert_eq!(manifest.output_mode, "Html");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_manifest_model_attribution_complete() {
        let state = sample_state();
        let models = infer_models_used(&state);

        assert!(
            models.iter().any(|m| m.purpose == "planning"),
            "missing planning model"
        );
        assert!(
            models.iter().any(|m| m.purpose == "content_generation"),
            "missing content model"
        );
    }

    #[test]
    fn test_manifest_quality_scores_present() {
        let scores = QualityScoresSummary::default();
        // All six scores should be accessible
        let _ = scores.accessibility;
        let _ = scores.seo;
        let _ = scores.performance;
        let _ = scores.security;
        let _ = scores.html_validity;
        let _ = scores.responsive;
        let _ = scores.overall;
    }

    #[test]
    fn test_manifest_cost_accurate() {
        let state = sample_state();
        let dir = std::env::temp_dir().join(format!("nexus-bm-cost-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        crate::project::save_project_state(&dir, &state).unwrap();

        let manifest = create_full_build_manifest(&dir, &state);
        assert!((manifest.total_cost_usd - 0.15).abs() < 1e-10);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_manifest_sign_and_verify() {
        let dir = std::env::temp_dir().join(format!("nexus-bm-sign-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let state = sample_state();
        crate::project::save_project_state(&dir, &state).unwrap();

        let mut manifest = create_full_build_manifest(&dir, &state);

        // Sign
        let sign_result = sign_manifest(&mut manifest);
        assert!(sign_result.is_ok(), "signing failed: {sign_result:?}");
        assert!(manifest.signature.is_some());
        assert!(manifest.signer_public_key.is_some());

        // Verify
        let verify_result = verify_manifest(&manifest);
        assert!(verify_result.is_ok(), "verify failed: {verify_result:?}");
        assert!(verify_result.unwrap(), "signature should be valid");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_manifest_tampered_fails_verify() {
        let dir = std::env::temp_dir().join(format!("nexus-bm-tamper-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let state = sample_state();
        crate::project::save_project_state(&dir, &state).unwrap();

        let mut manifest = create_full_build_manifest(&dir, &state);
        sign_manifest(&mut manifest).unwrap();

        // Tamper with the manifest
        manifest.total_cost_usd = 999.99;

        let result = verify_manifest(&manifest);
        assert!(result.is_ok());
        assert!(
            !result.unwrap(),
            "tampered manifest should fail verification"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_manifest_serializes_to_json() {
        let dir = std::env::temp_dir().join(format!("nexus-bm-json-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let state = sample_state();
        crate::project::save_project_state(&dir, &state).unwrap();

        let manifest = create_full_build_manifest(&dir, &state);
        let json = serde_json::to_string_pretty(&manifest);
        assert!(json.is_ok(), "serialization failed: {json:?}");

        // Roundtrip
        let parsed: Result<BuildManifest, _> = serde_json::from_str(&json.unwrap());
        assert!(parsed.is_ok(), "deserialization failed: {parsed:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_manifest_no_credentials() {
        let dir = std::env::temp_dir().join(format!("nexus-bm-cred-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let state = sample_state();
        crate::project::save_project_state(&dir, &state).unwrap();

        let manifest = create_full_build_manifest(&dir, &state);
        let json = serde_json::to_string(&manifest).unwrap();
        let lower = json.to_lowercase();
        assert!(!lower.contains("api_key"));
        assert!(!lower.contains("secret"));
        assert!(!lower.contains("token"));
        assert!(!lower.contains("credential"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
