//! Enterprise Trust Pack — governance artifacts packaged for compliance review.
//!
//! A Trust Pack consolidates all governance data for a project into exportable,
//! verifiable artifacts: signed manifests, SLSA provenance, quality reports,
//! audit trails, dependency manifests, and compliance reports.
//!
//! **Security invariants:**
//! - Trust pack outputs NEVER contain credentials or API keys
//! - User brief text is hashed (not included verbatim) in SLSA provenance
//! - Ed25519 signatures use `nexus-crypto::CryptoIdentity`

pub mod audit_trail;
pub mod build_manifest;
pub mod compliance_report;
pub mod dependency_report;
pub mod slsa;

use serde::{Deserialize, Serialize};
use std::path::Path;

// ─── Trust Pack Result ─────────────────────────────────────────────────────

/// Result of generating a complete trust pack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustPackResult {
    pub files_generated: Vec<TrustPackFile>,
    pub signed: bool,
    pub total_files: usize,
}

/// A single file in the trust pack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustPackFile {
    pub filename: String,
    pub description: String,
    pub size_bytes: u64,
}

// ─── Trust Pack Generator ──────────────────────────────────────────────────

/// Generate a complete trust pack for a project.
///
/// Outputs all governance artifacts to `{output_dir}/trust-pack/`.
pub fn generate_trust_pack(
    project_dir: &Path,
    output_dir: &Path,
) -> Result<TrustPackResult, String> {
    let trust_dir = output_dir.join("trust-pack");
    std::fs::create_dir_all(&trust_dir).map_err(|e| format!("create trust-pack dir: {e}"))?;

    let mut files = Vec::new();

    // Load project state
    let state = crate::project::load_project_state(project_dir)
        .unwrap_or_else(|_| crate::project::create_project("unknown", ""));

    // 1. Build Manifest (signed)
    let manifest = build_manifest::create_full_build_manifest(project_dir, &state);
    let manifest_json =
        serde_json::to_string_pretty(&manifest).map_err(|e| format!("serialize manifest: {e}"))?;
    let manifest_path = trust_dir.join("build_manifest.json");
    std::fs::write(&manifest_path, &manifest_json).map_err(|e| format!("write manifest: {e}"))?;
    files.push(TrustPackFile {
        filename: "build_manifest.json".into(),
        description: "Signed build manifest with provenance and quality data".into(),
        size_bytes: manifest_json.len() as u64,
    });

    // 1b. Detached signature file
    if let Some(ref sig) = manifest.signature {
        let sig_path = trust_dir.join("build_manifest.json.sig");
        std::fs::write(&sig_path, sig).map_err(|e| format!("write sig: {e}"))?;
        files.push(TrustPackFile {
            filename: "build_manifest.json.sig".into(),
            description: "Detached Ed25519 signature".into(),
            size_bytes: sig.len() as u64,
        });
    }

    // 2. SLSA Provenance
    let slsa_json = slsa::generate_slsa_provenance(&manifest, &state.prompt);
    let slsa_path = trust_dir.join("slsa_provenance.json");
    std::fs::write(&slsa_path, &slsa_json).map_err(|e| format!("write slsa: {e}"))?;
    files.push(TrustPackFile {
        filename: "slsa_provenance.json".into(),
        description: "SLSA v1 provenance predicate".into(),
        size_bytes: slsa_json.len() as u64,
    });

    // 3. Quality Report
    let quality_json = load_or_empty_json(project_dir, "quality_report.json");
    let quality_path = trust_dir.join("quality_report.json");
    std::fs::write(&quality_path, &quality_json).map_err(|e| format!("write quality: {e}"))?;
    files.push(TrustPackFile {
        filename: "quality_report.json".into(),
        description: "Pre-fix and post-fix quality check results".into(),
        size_bytes: quality_json.len() as u64,
    });

    // 3b. Conversion Report
    let conversion_json = load_or_empty_json(project_dir, "conversion_report.json");
    let conversion_path = trust_dir.join("conversion_report.json");
    std::fs::write(&conversion_path, &conversion_json)
        .map_err(|e| format!("write conversion: {e}"))?;
    files.push(TrustPackFile {
        filename: "conversion_report.json".into(),
        description: "Conversion effectiveness analysis (CTA, trust signals, copy clarity)".into(),
        size_bytes: conversion_json.len() as u64,
    });

    // 4. Audit Trail
    let events = audit_trail::collect_audit_trail(project_dir, &state);
    let audit_json = audit_trail::export_json(&events);
    let audit_path = trust_dir.join("audit_trail.json");
    std::fs::write(&audit_path, &audit_json).map_err(|e| format!("write audit: {e}"))?;
    files.push(TrustPackFile {
        filename: "audit_trail.json".into(),
        description: "Complete governance event timeline".into(),
        size_bytes: audit_json.len() as u64,
    });

    // 5. Dependency Manifest
    let dep_report = dependency_report::generate_dependency_report(
        state.selected_template.as_deref().unwrap_or("saas_landing"),
    );
    let dep_json =
        serde_json::to_string_pretty(&dep_report).map_err(|e| format!("serialize deps: {e}"))?;
    let dep_path = trust_dir.join("dependency_manifest.json");
    std::fs::write(&dep_path, &dep_json).map_err(|e| format!("write deps: {e}"))?;
    files.push(TrustPackFile {
        filename: "dependency_manifest.json".into(),
        description: "External resources with SRI hashes".into(),
        size_bytes: dep_json.len() as u64,
    });

    // 6. Deploy History
    let deploy_json = load_or_empty_json(project_dir, "deploy_history_v2.json");
    let deploy_path = trust_dir.join("deploy_history.json");
    std::fs::write(&deploy_path, &deploy_json).map_err(|e| format!("write deploy history: {e}"))?;
    files.push(TrustPackFile {
        filename: "deploy_history.json".into(),
        description: "All deployments with hashes and signatures".into(),
        size_bytes: deploy_json.len() as u64,
    });

    // 7. Compliance Report (HTML)
    let report_html = compliance_report::generate_compliance_html(
        &manifest,
        &events,
        state.selected_template.as_deref().unwrap_or("saas_landing"),
    );
    let report_path = trust_dir.join("compliance_report.html");
    std::fs::write(&report_path, &report_html).map_err(|e| format!("write report: {e}"))?;
    files.push(TrustPackFile {
        filename: "compliance_report.html".into(),
        description: "Human-readable compliance report (print to PDF)".into(),
        size_bytes: report_html.len() as u64,
    });

    let total_files = files.len();
    let signed = manifest.signature.is_some();

    Ok(TrustPackResult {
        files_generated: files,
        signed,
        total_files,
    })
}

/// Helper: load a JSON file from project dir, or return "[]" if missing.
fn load_or_empty_json(project_dir: &Path, filename: &str) -> String {
    let path = project_dir.join(filename);
    std::fs::read_to_string(&path).unwrap_or_else(|_| "[]".to_string())
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_trust_pack_creates_all_files() {
        let project_dir =
            std::env::temp_dir().join(format!("nexus-tp-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&project_dir).unwrap();

        // Create minimal project state
        let state = crate::project::create_project("test-proj", "AI writing tool");
        crate::project::save_project_state(&project_dir, &state).unwrap();

        let output_dir =
            std::env::temp_dir().join(format!("nexus-tp-out-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&output_dir).unwrap();

        let result = generate_trust_pack(&project_dir, &output_dir);
        assert!(result.is_ok(), "trust pack failed: {result:?}");

        let tp = result.unwrap();
        // 7 main files + 1 detached signature = 8
        assert!(
            tp.total_files >= 7,
            "expected at least 7 files, got {}",
            tp.total_files
        );

        // Verify all files exist
        let trust_dir = output_dir.join("trust-pack");
        assert!(trust_dir.join("build_manifest.json").exists());
        assert!(trust_dir.join("slsa_provenance.json").exists());
        assert!(trust_dir.join("quality_report.json").exists());
        assert!(trust_dir.join("audit_trail.json").exists());
        assert!(trust_dir.join("dependency_manifest.json").exists());
        assert!(trust_dir.join("deploy_history.json").exists());
        assert!(trust_dir.join("compliance_report.html").exists());

        let _ = std::fs::remove_dir_all(&project_dir);
        let _ = std::fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_trust_pack_no_credentials() {
        let project_dir =
            std::env::temp_dir().join(format!("nexus-tp-cred-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&project_dir).unwrap();

        let state = crate::project::create_project("test-proj", "Test");
        crate::project::save_project_state(&project_dir, &state).unwrap();

        let output_dir =
            std::env::temp_dir().join(format!("nexus-tp-credout-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&output_dir).unwrap();

        let _ = generate_trust_pack(&project_dir, &output_dir);

        // Read all generated files and check for credential patterns
        let trust_dir = output_dir.join("trust-pack");
        for entry in std::fs::read_dir(&trust_dir).unwrap() {
            let entry = entry.unwrap();
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                let lower = content.to_lowercase();
                assert!(
                    !lower.contains("api_key"),
                    "file {} contains api_key",
                    entry.file_name().to_string_lossy()
                );
                assert!(
                    !lower.contains("secret_key"),
                    "file {} contains secret_key",
                    entry.file_name().to_string_lossy()
                );
            }
        }

        let _ = std::fs::remove_dir_all(&project_dir);
        let _ = std::fs::remove_dir_all(&output_dir);
    }
}
