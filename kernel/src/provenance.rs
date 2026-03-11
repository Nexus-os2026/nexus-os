//! SLSA v1.0 provenance attestation generation.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A material (input artifact) used during the build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceMaterial {
    pub uri: String,
    pub digest_sha256: String,
    pub name: String,
}

/// Build environment details captured for provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildEnvironment {
    pub os: String,
    pub arch: String,
    pub rust_version: String,
    pub node_version: Option<String>,
    pub ci_pipeline_id: Option<String>,
    pub ci_job_id: Option<String>,
    pub git_commit: String,
    pub git_tag: Option<String>,
    pub git_branch: Option<String>,
    pub cargo_features: Vec<String>,
}

/// Metadata about the provenance attestation itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceMetadata {
    pub reproducible: bool,
    pub slsa_level: u8,
    pub sbom_reference: Option<String>,
}

/// A SLSA v1.0 provenance attestation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlsaProvenance {
    pub build_type: String,
    pub builder_id: String,
    pub invocation_id: String,
    pub build_started_on: String,
    pub build_finished_on: Option<String>,
    pub materials: Vec<ProvenanceMaterial>,
    pub environment: BuildEnvironment,
    pub metadata: ProvenanceMetadata,
}

impl SlsaProvenance {
    /// Create a new provenance attestation with defaults.
    pub fn new(builder_id: &str, git_commit: &str) -> Self {
        Self {
            build_type: "https://nexus-os.dev/build/v1".to_string(),
            builder_id: builder_id.to_string(),
            invocation_id: Uuid::new_v4().to_string(),
            build_started_on: String::new(),
            build_finished_on: None,
            materials: Vec::new(),
            environment: BuildEnvironment {
                os: String::new(),
                arch: String::new(),
                rust_version: String::new(),
                node_version: None,
                ci_pipeline_id: None,
                ci_job_id: None,
                git_commit: git_commit.to_string(),
                git_tag: None,
                git_branch: None,
                cargo_features: Vec::new(),
            },
            metadata: ProvenanceMetadata {
                reproducible: false,
                slsa_level: 2,
                sbom_reference: None,
            },
        }
    }

    /// Set the build environment details.
    pub fn set_environment(&mut self, env: BuildEnvironment) {
        self.environment = env;
    }

    /// Add a build material (input artifact).
    pub fn add_material(&mut self, name: &str, uri: &str, sha256: &str) {
        self.materials.push(ProvenanceMaterial {
            uri: uri.to_string(),
            digest_sha256: sha256.to_string(),
            name: name.to_string(),
        });
    }

    /// Mark the build as finished by setting build_finished_on.
    pub fn finalize(&mut self) {
        self.build_finished_on = Some(String::new());
    }

    /// Set the SBOM reference path.
    pub fn set_sbom_reference(&mut self, sbom_path: &str) {
        self.metadata.sbom_reference = Some(sbom_path.to_string());
    }

    /// Serialize to pretty-printed JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Wrap this provenance in an in-toto Statement v1 envelope.
    pub fn to_in_toto_statement(&self) -> String {
        let subjects: Vec<serde_json::Value> = self
            .materials
            .iter()
            .map(|m| {
                serde_json::json!({
                    "name": m.name,
                    "digest": { "sha256": m.digest_sha256 }
                })
            })
            .collect();

        let statement = serde_json::json!({
            "_type": "https://in-toto.io/Statement/v1",
            "subject": subjects,
            "predicateType": "https://slsa.dev/provenance/v1",
            "predicate": {
                "buildDefinition": {
                    "buildType": self.build_type,
                    "externalParameters": {
                        "git_commit": self.environment.git_commit,
                        "git_tag": self.environment.git_tag,
                        "cargo_features": self.environment.cargo_features,
                    },
                    "internalParameters": {
                        "os": self.environment.os,
                        "arch": self.environment.arch,
                        "rust_version": self.environment.rust_version,
                        "node_version": self.environment.node_version,
                        "ci_pipeline_id": self.environment.ci_pipeline_id,
                        "ci_job_id": self.environment.ci_job_id,
                    },
                    "resolvedDependencies": self.materials.iter().map(|m| {
                        serde_json::json!({
                            "uri": m.uri,
                            "digest": { "sha256": m.digest_sha256 },
                            "name": m.name,
                        })
                    }).collect::<Vec<_>>(),
                },
                "runDetails": {
                    "builder": { "id": self.builder_id },
                    "metadata": {
                        "invocationId": self.invocation_id,
                        "startedOn": self.build_started_on,
                        "finishedOn": self.build_finished_on,
                    },
                },
            }
        });

        serde_json::to_string_pretty(&statement).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults() {
        let p = SlsaProvenance::new("nexus-ci", "abc123");
        assert_eq!(p.build_type, "https://nexus-os.dev/build/v1");
        assert_eq!(p.builder_id, "nexus-ci");
        assert_eq!(p.environment.git_commit, "abc123");
        assert_eq!(p.metadata.slsa_level, 2);
        assert!(!p.metadata.reproducible);
        assert!(p.build_finished_on.is_none());
        assert!(p.materials.is_empty());
    }

    #[test]
    fn test_set_environment() {
        let mut p = SlsaProvenance::new("nexus-ci", "abc123");
        p.set_environment(BuildEnvironment {
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            rust_version: "1.82.0".to_string(),
            node_version: Some("22.0.0".to_string()),
            ci_pipeline_id: Some("12345".to_string()),
            ci_job_id: Some("67890".to_string()),
            git_commit: "def456".to_string(),
            git_tag: Some("v7.0.0".to_string()),
            git_branch: Some("main".to_string()),
            cargo_features: vec!["default".to_string()],
        });
        assert_eq!(p.environment.os, "linux");
        assert_eq!(p.environment.rust_version, "1.82.0");
        assert_eq!(p.environment.git_commit, "def456");
        assert_eq!(p.environment.git_tag.as_deref(), Some("v7.0.0"));
    }

    #[test]
    fn test_add_material() {
        let mut p = SlsaProvenance::new("nexus-ci", "abc123");
        p.add_material("Cargo.lock", "file:///Cargo.lock", "deadbeef");
        p.add_material(
            "package-lock.json",
            "file:///app/package-lock.json",
            "cafebabe",
        );
        assert_eq!(p.materials.len(), 2);
        assert_eq!(p.materials[0].name, "Cargo.lock");
        assert_eq!(p.materials[1].digest_sha256, "cafebabe");
    }

    #[test]
    fn test_finalize() {
        let mut p = SlsaProvenance::new("nexus-ci", "abc123");
        assert!(p.build_finished_on.is_none());
        p.finalize();
        assert!(p.build_finished_on.is_some());
    }

    #[test]
    fn test_sbom_reference() {
        let mut p = SlsaProvenance::new("nexus-ci", "abc123");
        assert!(p.metadata.sbom_reference.is_none());
        p.set_sbom_reference("sbom/nexus-os-7.0.0.cdx.json");
        assert_eq!(
            p.metadata.sbom_reference.as_deref(),
            Some("sbom/nexus-os-7.0.0.cdx.json")
        );
    }

    #[test]
    fn test_json_roundtrip() {
        let mut p = SlsaProvenance::new("nexus-ci", "abc123");
        p.add_material("Cargo.lock", "file:///Cargo.lock", "deadbeef");
        p.set_sbom_reference("sbom.json");

        let json = p.to_json().expect("serialization should succeed");
        let restored = SlsaProvenance::from_json(&json).expect("deserialization should succeed");
        assert_eq!(restored.builder_id, p.builder_id);
        assert_eq!(restored.materials.len(), 1);
        assert_eq!(
            restored.metadata.sbom_reference.as_deref(),
            Some("sbom.json")
        );
    }

    #[test]
    fn test_in_toto_statement() {
        let mut p = SlsaProvenance::new("nexus-ci", "abc123");
        p.add_material(
            "nexus-server",
            "file:///target/release/nexus-server",
            "aabbcc",
        );

        let stmt = p.to_in_toto_statement();
        let parsed: serde_json::Value =
            serde_json::from_str(&stmt).expect("statement should be valid JSON");

        assert_eq!(parsed["_type"], "https://in-toto.io/Statement/v1");
        assert_eq!(parsed["predicateType"], "https://slsa.dev/provenance/v1");
        assert_eq!(
            parsed["predicate"]["buildDefinition"]["buildType"],
            "https://nexus-os.dev/build/v1"
        );
        assert_eq!(
            parsed["predicate"]["runDetails"]["builder"]["id"],
            "nexus-ci"
        );
        assert_eq!(parsed["subject"][0]["name"], "nexus-server");
        assert_eq!(parsed["subject"][0]["digest"]["sha256"], "aabbcc");
    }
}
