//! SLSA v1 Provenance — supply-chain security compatible predicate.
//!
//! Follows the in-toto Statement v1 and SLSA v1 provenance specification.
//! Enterprise tools (Sigstore, Cosign, in-toto) can verify this format.
//!
//! **Privacy:** The user's brief text is SHA-256 hashed, not included verbatim.

use super::build_manifest::BuildManifest;
use sha2::Digest;

/// Generate an SLSA v1 provenance predicate JSON string.
///
/// The brief text is hashed for privacy — enterprise compliance officers
/// can verify the build was deterministic without seeing sensitive business info.
pub fn generate_slsa_provenance(manifest: &BuildManifest, brief: &str) -> String {
    let brief_hash = hex::encode(sha2::Sha256::digest(brief.as_bytes()));

    let resolved_deps: Vec<serde_json::Value> = manifest
        .models_used
        .iter()
        .map(|m| {
            serde_json::json!({
                "uri": format!("model://{}", m.model_name),
                "digest": {},
                "annotations": {
                    "purpose": m.purpose,
                    "cost_usd": m.cost_usd,
                    "invocations": m.invocation_count,
                }
            })
        })
        .collect();

    let provenance = serde_json::json!({
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [{
            "name": "project_output",
            "digest": {
                "sha256": manifest.build_hash
            }
        }],
        "predicateType": "https://slsa.dev/provenance/v1",
        "predicate": {
            "buildDefinition": {
                "buildType": "https://nexus-os.dev/NexusBuilder/v1",
                "externalParameters": {
                    "brief_hash": brief_hash,
                    "template": manifest.template_id,
                    "outputMode": manifest.output_mode,
                },
                "resolvedDependencies": resolved_deps,
            },
            "runDetails": {
                "builder": {
                    "id": "https://nexus-os.dev/NexusBuilder",
                    "version": {
                        "nexus_os": env!("CARGO_PKG_VERSION"),
                    }
                },
                "metadata": {
                    "invocationId": manifest.build_id,
                    "startedOn": manifest.timestamp,
                    "finishedOn": manifest.timestamp,
                }
            }
        }
    });

    serde_json::to_string_pretty(&provenance).unwrap_or_else(|_| "{}".to_string())
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trust_pack::build_manifest::{
        BuildManifest, ConversionScoresSummary, ModelUsage, QualityScoresSummary,
    };

    fn sample_manifest() -> BuildManifest {
        BuildManifest {
            project_id: "proj-001".into(),
            project_name: "AI Writer".into(),
            build_id: "build-abc".into(),
            build_hash: "deadbeef1234567890".into(),
            timestamp: "2026-04-04T12:00:00Z".into(),
            template_id: "saas_landing".into(),
            output_mode: "Html".into(),
            models_used: vec![
                ModelUsage {
                    model_name: "claude-haiku-4-5".into(),
                    purpose: "planning".into(),
                    cost_usd: 0.001,
                    invocation_count: 1,
                },
                ModelUsage {
                    model_name: "claude-sonnet-4-6".into(),
                    purpose: "content_generation".into(),
                    cost_usd: 0.149,
                    invocation_count: 1,
                },
            ],
            total_cost_usd: 0.15,
            quality_scores: QualityScoresSummary {
                accessibility: 95,
                seo: 90,
                performance: 88,
                security: 100,
                html_validity: 92,
                responsive: 85,
                overall: 91,
            },
            conversion_scores: ConversionScoresSummary::default(),
            issues_found: 3,
            issues_fixed: 3,
            external_dependency_count: 5,
            backend_provider: None,
            schema_hash: None,
            rls_policy_hash: None,
            deploy_provider: Some("netlify".into()),
            deploy_url: Some("https://mysite.netlify.app".into()),
            deploy_hash: Some("abc123".into()),
            signer_public_key: None,
            signature: None,
        }
    }

    #[test]
    fn test_slsa_valid_json() {
        let json = generate_slsa_provenance(&sample_manifest(), "AI writing tool");
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&json);
        assert!(parsed.is_ok(), "invalid JSON: {parsed:?}");
    }

    #[test]
    fn test_slsa_has_subject_digest() {
        let json = generate_slsa_provenance(&sample_manifest(), "test brief");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        let digest = &v["subject"][0]["digest"]["sha256"];
        assert_eq!(digest, "deadbeef1234567890");
    }

    #[test]
    fn test_slsa_has_builder_id() {
        let json = generate_slsa_provenance(&sample_manifest(), "test brief");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        let builder_id = v["predicate"]["runDetails"]["builder"]["id"].as_str();
        assert_eq!(builder_id, Some("https://nexus-os.dev/NexusBuilder"));
    }

    #[test]
    fn test_slsa_has_materials() {
        let json = generate_slsa_provenance(&sample_manifest(), "test brief");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        let deps = &v["predicate"]["buildDefinition"]["resolvedDependencies"];
        assert!(deps.is_array());
        let deps_arr = deps.as_array().unwrap();
        assert_eq!(deps_arr.len(), 2);
        assert!(deps_arr[0]["uri"]
            .as_str()
            .unwrap()
            .contains("claude-haiku"));
    }

    #[test]
    fn test_slsa_hashes_brief_not_includes() {
        let brief = "Super secret business plan for AI domination";
        let json = generate_slsa_provenance(&sample_manifest(), brief);

        // Brief text should NOT appear verbatim
        assert!(
            !json.contains(brief),
            "brief text should be hashed, not included"
        );

        // But the hash should be present
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let brief_hash = v["predicate"]["buildDefinition"]["externalParameters"]["brief_hash"]
            .as_str()
            .unwrap();
        assert!(!brief_hash.is_empty());
        assert_eq!(brief_hash.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn test_slsa_has_invocation_id() {
        let json = generate_slsa_provenance(&sample_manifest(), "test");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        let inv_id = v["predicate"]["runDetails"]["metadata"]["invocationId"]
            .as_str()
            .unwrap();
        assert_eq!(inv_id, "build-abc");
    }

    #[test]
    fn test_slsa_predicate_type() {
        let json = generate_slsa_provenance(&sample_manifest(), "test");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(
            v["predicateType"].as_str().unwrap(),
            "https://slsa.dev/provenance/v1"
        );
        assert_eq!(
            v["_type"].as_str().unwrap(),
            "https://in-toto.io/Statement/v1"
        );
    }
}
