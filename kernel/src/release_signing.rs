//! Sigstore-compatible signing and verification for release artifacts.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

/// A single signed release artifact with its hash and optional certificate chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningArtifact {
    pub artifact_path: String,
    pub artifact_hash: String,
    pub signature: Vec<u8>,
    pub certificate: Option<String>,
    pub timestamp: u64,
    pub signer_identity: String,
}

/// Manifest describing all signed artifacts in a release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningManifest {
    pub version: String,
    pub artifacts: Vec<SigningArtifact>,
    pub build_id: String,
    pub git_commit: String,
    pub git_tag: String,
    pub pipeline_id: String,
    pub created_at: u64,
}

impl SigningManifest {
    pub fn new(git_commit: String, git_tag: String, pipeline_id: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            version: "1.0.0".to_string(),
            artifacts: Vec::new(),
            build_id: uuid::Uuid::new_v4().to_string(),
            git_commit,
            git_tag,
            pipeline_id,
            created_at: now,
        }
    }

    pub fn add_artifact(&mut self, path: &str, hash: &str, signature: Vec<u8>) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.artifacts.push(SigningArtifact {
            artifact_path: path.to_string(),
            artifact_hash: hash.to_string(),
            signature,
            certificate: None,
            timestamp: now,
            signer_identity: String::new(),
        });
    }

    pub fn compute_hash(file_bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(file_bytes);
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    pub fn verify_artifact_hash(artifact: &SigningArtifact, file_bytes: &[u8]) -> bool {
        let computed = Self::compute_hash(file_bytes);
        computed == artifact.artifact_hash
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Result of verifying all artifacts in a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationResult {
    AllValid,
    PartialFailure { failed: Vec<String> },
    AllFailed,
}

/// Errors that can occur during release verification.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerificationError {
    #[error("manifest parse error: {0}")]
    ManifestParseError(String),
    #[error("hash mismatch for {artifact}: expected {expected}, got {actual}")]
    HashMismatch {
        artifact: String,
        expected: String,
        actual: String,
    },
    #[error("missing signature for artifact: {0}")]
    MissingSignature(String),
}

/// Verifies release manifests and artifact integrity.
#[derive(Debug)]
pub struct ReleaseVerifier;

impl ReleaseVerifier {
    pub fn verify_manifest(
        manifest: &SigningManifest,
    ) -> Result<VerificationResult, VerificationError> {
        if manifest.artifacts.is_empty() {
            return Err(VerificationError::ManifestParseError(
                "manifest contains no artifacts".to_string(),
            ));
        }

        let mut failed = Vec::new();
        for artifact in &manifest.artifacts {
            if artifact.artifact_hash.is_empty() {
                failed.push(artifact.artifact_path.clone());
                continue;
            }
            if artifact.signature.is_empty() {
                return Err(VerificationError::MissingSignature(
                    artifact.artifact_path.clone(),
                ));
            }
            // Validate hash is valid hex of correct length (SHA-256 = 64 hex chars)
            if artifact.artifact_hash.len() != 64
                || !artifact
                    .artifact_hash
                    .chars()
                    .all(|c| c.is_ascii_hexdigit())
            {
                failed.push(artifact.artifact_path.clone());
            }
        }

        if failed.len() == manifest.artifacts.len() {
            Ok(VerificationResult::AllFailed)
        } else if failed.is_empty() {
            Ok(VerificationResult::AllValid)
        } else {
            Ok(VerificationResult::PartialFailure { failed })
        }
    }

    pub fn verify_artifact_integrity(artifact: &SigningArtifact, file_bytes: &[u8]) -> bool {
        SigningManifest::verify_artifact_hash(artifact, file_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_deterministic() {
        let data = b"nexus-os release binary";
        let h1 = SigningManifest::compute_hash(data);
        let h2 = SigningManifest::compute_hash(data);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn test_compute_hash_different_inputs() {
        let h1 = SigningManifest::compute_hash(b"binary-v1");
        let h2 = SigningManifest::compute_hash(b"binary-v2");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_add_artifact_and_verify_hash() {
        let data = b"release-artifact-bytes";
        let hash = SigningManifest::compute_hash(data);

        let mut manifest =
            SigningManifest::new("abc123".into(), "v7.0.0".into(), "pipeline-42".into());
        manifest.add_artifact("target/release/nexus-server", &hash, vec![1, 2, 3]);

        assert_eq!(manifest.artifacts.len(), 1);
        assert!(SigningManifest::verify_artifact_hash(
            &manifest.artifacts[0],
            data
        ));
    }

    #[test]
    fn test_verify_artifact_hash_mismatch() {
        let data = b"original";
        let hash = SigningManifest::compute_hash(data);

        let artifact = SigningArtifact {
            artifact_path: "bin/test".to_string(),
            artifact_hash: hash,
            signature: vec![1],
            certificate: None,
            timestamp: 0,
            signer_identity: "ci".to_string(),
        };

        assert!(!SigningManifest::verify_artifact_hash(
            &artifact,
            b"tampered"
        ));
    }

    #[test]
    fn test_manifest_json_roundtrip() {
        let mut manifest =
            SigningManifest::new("def456".into(), "v7.1.0".into(), "pipeline-99".into());
        let hash = SigningManifest::compute_hash(b"test-binary");
        manifest.add_artifact("target/release/nexus-server", &hash, vec![10, 20]);

        let json = manifest.to_json().unwrap();
        let restored = SigningManifest::from_json(&json).unwrap();

        assert_eq!(restored.git_commit, "def456");
        assert_eq!(restored.git_tag, "v7.1.0");
        assert_eq!(restored.artifacts.len(), 1);
        assert_eq!(
            restored.artifacts[0].artifact_path,
            "target/release/nexus-server"
        );
    }

    #[test]
    fn test_verify_manifest_all_valid() {
        let hash = SigningManifest::compute_hash(b"data");
        let mut manifest = SigningManifest::new("abc".into(), "v1.0.0".into(), "pipe-1".into());
        manifest.add_artifact("bin/a", &hash, vec![1, 2, 3]);

        // Set signer identity so it's complete
        manifest.artifacts[0].signer_identity = "ci-bot".to_string();

        let result = ReleaseVerifier::verify_manifest(&manifest).unwrap();
        assert_eq!(result, VerificationResult::AllValid);
    }

    #[test]
    fn test_verify_manifest_missing_signature() {
        let hash = SigningManifest::compute_hash(b"data");
        let manifest = SigningManifest {
            version: "1.0.0".to_string(),
            artifacts: vec![SigningArtifact {
                artifact_path: "bin/test".to_string(),
                artifact_hash: hash,
                signature: vec![],
                certificate: None,
                timestamp: 0,
                signer_identity: "ci".to_string(),
            }],
            build_id: "build-1".to_string(),
            git_commit: "abc".to_string(),
            git_tag: "v1".to_string(),
            pipeline_id: "p1".to_string(),
            created_at: 0,
        };

        let result = ReleaseVerifier::verify_manifest(&manifest);
        assert!(matches!(
            result,
            Err(VerificationError::MissingSignature(_))
        ));
    }

    #[test]
    fn test_verify_manifest_empty_artifacts() {
        let manifest = SigningManifest::new("abc".into(), "v1".into(), "p1".into());
        let result = ReleaseVerifier::verify_manifest(&manifest);
        assert!(matches!(
            result,
            Err(VerificationError::ManifestParseError(_))
        ));
    }

    #[test]
    fn test_verify_manifest_partial_failure() {
        let valid_hash = SigningManifest::compute_hash(b"good");
        let manifest = SigningManifest {
            version: "1.0.0".to_string(),
            artifacts: vec![
                SigningArtifact {
                    artifact_path: "bin/good".to_string(),
                    artifact_hash: valid_hash,
                    signature: vec![1],
                    certificate: None,
                    timestamp: 0,
                    signer_identity: "ci".to_string(),
                },
                SigningArtifact {
                    artifact_path: "bin/bad".to_string(),
                    artifact_hash: "not-a-valid-hash".to_string(),
                    signature: vec![1],
                    certificate: None,
                    timestamp: 0,
                    signer_identity: "ci".to_string(),
                },
            ],
            build_id: "b1".to_string(),
            git_commit: "abc".to_string(),
            git_tag: "v1".to_string(),
            pipeline_id: "p1".to_string(),
            created_at: 0,
        };

        let result = ReleaseVerifier::verify_manifest(&manifest).unwrap();
        assert!(matches!(result, VerificationResult::PartialFailure { .. }));
        if let VerificationResult::PartialFailure { failed } = result {
            assert_eq!(failed, vec!["bin/bad"]);
        }
    }

    #[test]
    fn test_signing_manifest_add_multiple_artifacts() {
        let mut manifest = SigningManifest::new("abc".into(), "v2.0.0".into(), "pipe-10".into());
        let h1 = SigningManifest::compute_hash(b"binary-server");
        let h2 = SigningManifest::compute_hash(b"binary-agent");
        let h3 = SigningManifest::compute_hash(b"binary-poster");
        manifest.add_artifact("target/release/nexus-server", &h1, vec![1]);
        manifest.add_artifact("target/release/coding-agent", &h2, vec![2]);
        manifest.add_artifact("target/release/social-poster-agent", &h3, vec![3]);

        assert_eq!(manifest.artifacts.len(), 3);
        assert_eq!(
            manifest.artifacts[0].artifact_path,
            "target/release/nexus-server"
        );
        assert_eq!(
            manifest.artifacts[1].artifact_path,
            "target/release/coding-agent"
        );
        assert_eq!(
            manifest.artifacts[2].artifact_path,
            "target/release/social-poster-agent"
        );
        assert_eq!(manifest.artifacts[0].artifact_hash, h1);
        assert_eq!(manifest.artifacts[1].artifact_hash, h2);
        assert_eq!(manifest.artifacts[2].artifact_hash, h3);
    }

    #[test]
    fn test_verification_error_display() {
        let err = VerificationError::ManifestParseError("bad json".to_string());
        assert_eq!(err.to_string(), "manifest parse error: bad json");

        let err = VerificationError::HashMismatch {
            artifact: "bin/server".to_string(),
            expected: "aaa".to_string(),
            actual: "bbb".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "hash mismatch for bin/server: expected aaa, got bbb"
        );

        let err = VerificationError::MissingSignature("bin/agent".to_string());
        assert_eq!(err.to_string(), "missing signature for artifact: bin/agent");
    }

    #[test]
    fn test_release_verifier_integrity_check() {
        let data = b"my-binary";
        let hash = SigningManifest::compute_hash(data);
        let artifact = SigningArtifact {
            artifact_path: "bin/test".to_string(),
            artifact_hash: hash,
            signature: vec![1],
            certificate: None,
            timestamp: 0,
            signer_identity: "ci".to_string(),
        };

        assert!(ReleaseVerifier::verify_artifact_integrity(&artifact, data));
        assert!(!ReleaseVerifier::verify_artifact_integrity(
            &artifact, b"wrong"
        ));
    }
}
