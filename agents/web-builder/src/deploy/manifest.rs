//! Deploy manifest — governance record for every deployment.
//!
//! Every deploy produces a signed manifest that records the build hash,
//! timestamp, file count, cost, and model attribution. The manifest is
//! saved in the project directory and included in governance exports.
//!
//! **Security invariants:**
//! - Manifests NEVER contain credential tokens
//! - The Ed25519 signature slot is populated when nexus-crypto is available

use super::{now_iso8601, DeployResult};
use crate::build_orchestrator::BuildCost;
use serde::{Deserialize, Serialize};

// ─── Deploy Manifest ───────────────────────────────────────────────────────

/// Governance manifest for a deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployManifest {
    pub deploy_id: String,
    pub provider: String,
    pub url: String,
    pub build_hash: String,
    pub timestamp: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub model_attribution: Vec<String>,
    pub cost_usd: f64,
    /// Ed25519 signature of the canonical manifest JSON.
    /// None until signing infrastructure is connected.
    pub signature: Option<String>,
}

/// Create a deploy manifest from a deploy result and build metadata.
pub fn create_deploy_manifest(
    deploy_result: &DeployResult,
    build_cost: &BuildCost,
    model_attribution: &[String],
    file_count: usize,
    total_bytes: u64,
) -> DeployManifest {
    DeployManifest {
        deploy_id: deploy_result.deploy_id.clone(),
        provider: deploy_result.provider.clone(),
        url: deploy_result.url.clone(),
        build_hash: deploy_result.build_hash.clone(),
        timestamp: deploy_result.timestamp.clone(),
        file_count,
        total_bytes,
        model_attribution: model_attribution.to_vec(),
        cost_usd: build_cost.total,
        signature: None,
    }
}

/// Produce the canonical JSON bytes for signing (excludes the signature field).
fn canonical_bytes(manifest: &DeployManifest) -> Vec<u8> {
    // Create a copy without the signature for canonical form
    let canonical = serde_json::json!({
        "deploy_id": manifest.deploy_id,
        "provider": manifest.provider,
        "url": manifest.url,
        "build_hash": manifest.build_hash,
        "timestamp": manifest.timestamp,
        "file_count": manifest.file_count,
        "total_bytes": manifest.total_bytes,
        "model_attribution": manifest.model_attribution,
        "cost_usd": manifest.cost_usd,
    });
    serde_json::to_vec(&canonical).unwrap_or_default()
}

/// Sign the manifest using Ed25519 (placeholder — connects to nexus-crypto).
/// Returns the hex-encoded signature string.
///
/// In the current implementation, this returns None because the signing key
/// infrastructure requires runtime key management. The signature slot is
/// present so governance reports show the field exists.
pub fn sign_manifest(manifest: &mut DeployManifest) -> Option<String> {
    let _bytes = canonical_bytes(manifest);

    // TODO: Connect to nexus-crypto CryptoIdentity for Ed25519 signing.
    // For now, record that signing was attempted but no key was available.
    // This matches the existing pattern in builder_export_project where
    // NEXUS_BUILDER_SIGNATURE says "signature infrastructure pending".
    None
}

/// Save the deploy manifest to the project directory.
pub fn save_manifest(
    project_dir: &std::path::Path,
    manifest: &DeployManifest,
) -> Result<(), String> {
    let path = project_dir.join("deploy_manifest.json");
    let json =
        serde_json::to_string_pretty(manifest).map_err(|e| format!("serialize manifest: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write manifest: {e}"))
}

/// Load the most recent deploy manifest from a project directory.
pub fn load_manifest(project_dir: &std::path::Path) -> Option<DeployManifest> {
    let path = project_dir.join("deploy_manifest.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
}

/// Build an audit log entry for a deploy event (credentials redacted).
pub fn deploy_audit_entry(manifest: &DeployManifest) -> serde_json::Value {
    serde_json::json!({
        "event": "deploy",
        "deploy_id": manifest.deploy_id,
        "provider": manifest.provider,
        "url": manifest.url,
        "build_hash": manifest.build_hash,
        "timestamp": manifest.timestamp,
        "file_count": manifest.file_count,
        "total_bytes": manifest.total_bytes,
        "cost_usd": manifest.cost_usd,
        "signed": manifest.signature.is_some(),
        "logged_at": now_iso8601(),
    })
}

// ─── Deploy History ────────────────────────────────────────────────────────

/// Append a deploy record to the project's deploy history.
pub fn append_deploy_history(
    project_dir: &std::path::Path,
    manifest: &DeployManifest,
) -> Result<(), String> {
    let path = project_dir.join("deploy_history.json");
    let mut history: Vec<DeployManifest> = if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    history.push(manifest.clone());

    let json =
        serde_json::to_string_pretty(&history).map_err(|e| format!("serialize history: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write history: {e}"))
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_orchestrator::BuildCost;
    use crate::deploy::DeployResult;

    fn sample_deploy_result() -> DeployResult {
        DeployResult {
            deploy_id: "dep-123".into(),
            url: "https://mysite.netlify.app".into(),
            provider: "netlify".into(),
            site_id: "site-abc".into(),
            timestamp: "2026-04-04T12:00:00Z".into(),
            build_hash: "abc123def456".into(),
            duration_ms: 5000,
        }
    }

    fn sample_cost() -> BuildCost {
        BuildCost {
            planning: 0.001,
            content: 0.15,
            build: 0.0,
            images: 0.0,
            total: 0.151,
        }
    }

    #[test]
    fn test_create_manifest_has_all_fields() {
        let result = sample_deploy_result();
        let cost = sample_cost();
        let manifest =
            create_deploy_manifest(&result, &cost, &["claude-sonnet-4-6".into()], 12, 34200);

        assert_eq!(manifest.deploy_id, "dep-123");
        assert_eq!(manifest.provider, "netlify");
        assert_eq!(manifest.url, "https://mysite.netlify.app");
        assert_eq!(manifest.build_hash, "abc123def456");
        assert_eq!(manifest.timestamp, "2026-04-04T12:00:00Z");
        assert_eq!(manifest.file_count, 12);
        assert_eq!(manifest.total_bytes, 34200);
        assert!((manifest.cost_usd - 0.151).abs() < 1e-10);
    }

    #[test]
    fn test_manifest_excludes_credentials() {
        let result = sample_deploy_result();
        let cost = sample_cost();
        let manifest = create_deploy_manifest(&result, &cost, &["sonnet".into()], 5, 1000);

        let json = serde_json::to_string(&manifest).unwrap();
        assert!(!json.contains("token"));
        assert!(!json.contains("api_key"));
        assert!(!json.contains("secret"));
        assert!(!json.contains("credential"));
    }

    #[test]
    fn test_manifest_includes_model_attribution() {
        let result = sample_deploy_result();
        let cost = sample_cost();
        let manifest = create_deploy_manifest(
            &result,
            &cost,
            &["claude-sonnet-4-6".into(), "claude-haiku-4-5".into()],
            1,
            100,
        );

        assert_eq!(manifest.model_attribution.len(), 2);
        assert!(manifest
            .model_attribution
            .contains(&"claude-sonnet-4-6".into()));
        assert!(manifest
            .model_attribution
            .contains(&"claude-haiku-4-5".into()));
    }

    #[test]
    fn test_manifest_includes_cost() {
        let result = sample_deploy_result();
        let cost = BuildCost {
            planning: 0.005,
            content: 0.20,
            build: 0.0,
            images: 0.0,
            total: 0.205,
        };
        let manifest = create_deploy_manifest(&result, &cost, &[], 1, 100);
        assert!((manifest.cost_usd - 0.205).abs() < 1e-10);
    }

    #[test]
    fn test_sign_manifest_returns_none_currently() {
        let result = sample_deploy_result();
        let cost = sample_cost();
        let mut manifest = create_deploy_manifest(&result, &cost, &[], 1, 100);

        let sig = sign_manifest(&mut manifest);
        // Currently returns None (signing infrastructure pending)
        assert!(sig.is_none());
    }

    #[test]
    fn test_deploy_audit_entry_has_no_credentials() {
        let result = sample_deploy_result();
        let cost = sample_cost();
        let manifest = create_deploy_manifest(&result, &cost, &[], 1, 100);

        let entry = deploy_audit_entry(&manifest);
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("deploy_id"));
        assert!(json.contains("build_hash"));
        assert!(!json.contains("token"));
        assert!(!json.contains("secret"));
    }

    #[test]
    fn test_save_and_load_manifest() {
        let dir =
            std::env::temp_dir().join(format!("nexus-manifest-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let result = sample_deploy_result();
        let cost = sample_cost();
        let manifest = create_deploy_manifest(&result, &cost, &["sonnet".into()], 5, 2000);

        save_manifest(&dir, &manifest).unwrap();
        let loaded = load_manifest(&dir).unwrap();

        assert_eq!(loaded.deploy_id, "dep-123");
        assert_eq!(loaded.provider, "netlify");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_deploy_history_appends() {
        let dir = std::env::temp_dir().join(format!("nexus-history-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let result = sample_deploy_result();
        let cost = sample_cost();
        let m1 = create_deploy_manifest(&result, &cost, &[], 1, 100);
        let m2 = create_deploy_manifest(&result, &cost, &[], 2, 200);

        append_deploy_history(&dir, &m1).unwrap();
        append_deploy_history(&dir, &m2).unwrap();

        let path = dir.join("deploy_history.json");
        let json = std::fs::read_to_string(&path).unwrap();
        let history: Vec<DeployManifest> = serde_json::from_str(&json).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].file_count, 1);
        assert_eq!(history[1].file_count, 2);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
