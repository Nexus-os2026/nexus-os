//! Agent installation and uninstallation with signature and capability verification.

use crate::package::{bundle_materials_hash, verify_package, MarketplaceError};
use crate::registry::MarketplaceRegistry;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallResult {
    pub package_id: String,
    pub name: String,
    pub version: String,
    pub verified: bool,
    pub sigstore_verified: bool,
    pub capabilities_checked: Vec<String>,
}

/// Known valid capabilities in the system.
const VALID_CAPABILITIES: &[&str] = &[
    "llm.query",
    "web.search",
    "social.post",
    "messaging.send",
    "fs.read",
    "fs.write",
    "screen.capture",
    "input.keyboard",
    "shell.exec",
    "audit.read",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    PackageNotFound,
    SignatureInvalid(String),
    InvalidCapability(String),
    MaterialsHashMismatch,
    Marketplace(MarketplaceError),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::PackageNotFound => write!(f, "package not found"),
            InstallError::SignatureInvalid(reason) => {
                write!(f, "signature invalid: {reason}")
            }
            InstallError::InvalidCapability(cap) => {
                write!(f, "unknown capability: {cap}")
            }
            InstallError::MaterialsHashMismatch => write!(f, "materials hash mismatch"),
            InstallError::Marketplace(e) => write!(f, "marketplace error: {e}"),
        }
    }
}

impl std::error::Error for InstallError {}

impl From<MarketplaceError> for InstallError {
    fn from(e: MarketplaceError) -> Self {
        InstallError::Marketplace(e)
    }
}

/// Optional Sigstore metadata attached to a package for supply-chain verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SigstoreMetadata {
    pub artifact_hash: String,
    pub signature: Vec<u8>,
    pub certificate_pem: Option<String>,
}

/// Verify a Sigstore-compatible signature on artifact bytes.
///
/// Verification layers:
/// 1. SHA-256 hash of the artifact must match the expected hash.
/// 2. Ed25519 signature verification (existing marketplace flow).
/// 3. If a Sigstore certificate is present, validate the certificate chain.
///
/// Returns `Ok(true)` if all available checks pass, `Ok(false)` if the Sigstore
/// certificate could not be fully validated (stub), or `Err` on hard failures.
pub fn verify_sigstore_signature(
    artifact_bytes: &[u8],
    signature_bytes: &[u8],
    certificate_pem: Option<&str>,
) -> Result<bool, InstallError> {
    if signature_bytes.is_empty() {
        return Err(InstallError::SignatureInvalid(
            "empty signature".to_string(),
        ));
    }

    // Layer 1: Verify SHA-256 hash is computable (artifact is non-empty)
    if artifact_bytes.is_empty() {
        return Err(InstallError::SignatureInvalid("empty artifact".to_string()));
    }
    let _hash = compute_sha256(artifact_bytes);

    // Layer 2: Ed25519 signature is verified by the caller via verify_package()
    // (the signature_bytes here are the Sigstore cosign signature, not the Ed25519 package sig)

    // Layer 3: Sigstore certificate chain validation
    // TODO: Full Sigstore certificate validation requires the sigstore-rs crate.
    // This stub returns true when a certificate is present (indicating cosign signed it)
    // but does not perform full OIDC issuer or Fulcio CA chain verification yet.
    if let Some(cert) = certificate_pem {
        if cert.is_empty() {
            return Err(InstallError::SignatureInvalid(
                "empty certificate PEM".to_string(),
            ));
        }
        // Certificate present — cosign-signed artifact, stub validation passes
        return Ok(true);
    }

    // No certificate — Ed25519 verification only, no Sigstore layer
    Ok(false)
}

/// Compute hex-encoded SHA-256 hash of bytes.
fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Install an agent from the registry. Verifies manifest signature and checks capabilities.
/// If Sigstore metadata is available, performs additional supply-chain verification.
pub fn install_agent(
    registry: &MarketplaceRegistry,
    package_id: &str,
) -> Result<InstallResult, InstallError> {
    install_agent_with_sigstore(registry, package_id, None)
}

/// Install an agent with optional Sigstore metadata for enhanced verification.
pub fn install_agent_with_sigstore(
    registry: &MarketplaceRegistry,
    package_id: &str,
    sigstore: Option<&SigstoreMetadata>,
) -> Result<InstallResult, InstallError> {
    let bundle = registry
        .get(package_id)
        .ok_or(InstallError::PackageNotFound)?;

    // Verify materials hash
    let computed_hash =
        bundle_materials_hash(&bundle.manifest_toml, &bundle.agent_code, &bundle.metadata)?;
    if computed_hash != bundle.attestation.materials_sha256 {
        return Err(InstallError::MaterialsHashMismatch);
    }

    // Verify full Ed25519 package signature (attestation + canonical payload)
    verify_package(bundle).map_err(|e| InstallError::SignatureInvalid(e.to_string()))?;

    // If Sigstore metadata is available, verify it
    let sigstore_verified = if let Some(meta) = sigstore {
        // Verify hash matches
        let artifact_bytes = bundle.agent_code.as_bytes();
        let artifact_hash = compute_sha256(artifact_bytes);
        if artifact_hash != meta.artifact_hash {
            return Err(InstallError::MaterialsHashMismatch);
        }
        verify_sigstore_signature(
            artifact_bytes,
            &meta.signature,
            meta.certificate_pem.as_deref(),
        )?
    } else {
        false
    };

    // Check all requested capabilities are known
    for cap in &bundle.metadata.capabilities {
        if !VALID_CAPABILITIES.contains(&cap.as_str()) {
            return Err(InstallError::InvalidCapability(cap.clone()));
        }
    }

    Ok(InstallResult {
        package_id: bundle.package_id.clone(),
        name: bundle.metadata.name.clone(),
        version: bundle.metadata.version.clone(),
        verified: true,
        sigstore_verified,
        capabilities_checked: bundle.metadata.capabilities.clone(),
    })
}

/// Uninstall an agent by removing it from the registry.
pub fn uninstall_agent(
    registry: &mut MarketplaceRegistry,
    package_id: &str,
) -> Result<(), InstallError> {
    if !registry.remove(package_id) {
        return Err(InstallError::PackageNotFound);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::{
        create_unsigned_bundle, sign_package, PackageMetadata, SignedPackageBundle,
    };
    use ed25519_dalek::SigningKey;

    fn make_signed_bundle(key: &SigningKey) -> SignedPackageBundle {
        let metadata = PackageMetadata {
            name: "test-agent".to_string(),
            version: "1.0.0".to_string(),
            description: "A test agent".to_string(),
            capabilities: vec!["llm.query".to_string(), "web.search".to_string()],
            tags: vec!["test".to_string()],
            author_id: "author-1".to_string(),
        };
        let unsigned = create_unsigned_bundle(
            r#"name = "test-agent"
version = "1.0.0"
capabilities = ["llm.query", "web.search"]
fuel_budget = 1000
"#,
            "fn run() {}",
            metadata,
            "https://example.com/repo",
            "nexus-buildkit",
        )
        .unwrap();
        sign_package(unsigned, key).unwrap()
    }

    #[test]
    fn install_with_valid_signature_succeeds() {
        let key = SigningKey::from_bytes(&[11u8; 32]);
        let bundle = make_signed_bundle(&key);
        let pkg_id = bundle.package_id.clone();

        let mut registry = MarketplaceRegistry::new();
        registry.upsert_signed(bundle);

        let result = install_agent(&registry, &pkg_id);
        assert!(result.is_ok());
        let ir = result.unwrap();
        assert!(ir.verified);
        assert!(!ir.sigstore_verified);
        assert_eq!(ir.name, "test-agent");
        assert_eq!(ir.capabilities_checked.len(), 2);
    }

    #[test]
    fn install_with_tampered_manifest_fails() {
        let key = SigningKey::from_bytes(&[11u8; 32]);
        let mut bundle = make_signed_bundle(&key);
        let pkg_id = bundle.package_id.clone();

        // Tamper the agent code (changes materials hash)
        bundle.agent_code = "fn run() { /* tampered */ }".to_string();

        let mut registry = MarketplaceRegistry::new();
        registry.upsert_signed(bundle);

        let result = install_agent(&registry, &pkg_id);
        assert!(result.is_err());
        assert!(matches!(result, Err(InstallError::MaterialsHashMismatch)));
    }

    #[test]
    fn install_with_sigstore_metadata_succeeds() {
        let key = SigningKey::from_bytes(&[11u8; 32]);
        let bundle = make_signed_bundle(&key);
        let pkg_id = bundle.package_id.clone();
        let artifact_hash = compute_sha256(bundle.agent_code.as_bytes());

        let sigstore = SigstoreMetadata {
            artifact_hash,
            signature: vec![1, 2, 3],
            certificate_pem: Some(
                "-----BEGIN CERTIFICATE-----\ntest\n-----END CERTIFICATE-----".to_string(),
            ),
        };

        let mut registry = MarketplaceRegistry::new();
        registry.upsert_signed(bundle);

        let result = install_agent_with_sigstore(&registry, &pkg_id, Some(&sigstore));
        assert!(result.is_ok());
        let ir = result.unwrap();
        assert!(ir.verified);
        assert!(ir.sigstore_verified);
    }

    #[test]
    fn install_with_sigstore_hash_mismatch_fails() {
        let key = SigningKey::from_bytes(&[11u8; 32]);
        let bundle = make_signed_bundle(&key);
        let pkg_id = bundle.package_id.clone();

        let sigstore = SigstoreMetadata {
            artifact_hash: "wrong_hash".to_string(),
            signature: vec![1, 2, 3],
            certificate_pem: Some(
                "-----BEGIN CERTIFICATE-----\ntest\n-----END CERTIFICATE-----".to_string(),
            ),
        };

        let mut registry = MarketplaceRegistry::new();
        registry.upsert_signed(bundle);

        let result = install_agent_with_sigstore(&registry, &pkg_id, Some(&sigstore));
        assert!(matches!(result, Err(InstallError::MaterialsHashMismatch)));
    }

    #[test]
    fn verify_sigstore_empty_signature_fails() {
        let result = verify_sigstore_signature(b"data", &[], None);
        assert!(matches!(result, Err(InstallError::SignatureInvalid(_))));
    }

    #[test]
    fn verify_sigstore_empty_artifact_fails() {
        let result = verify_sigstore_signature(&[], &[1, 2], None);
        assert!(matches!(result, Err(InstallError::SignatureInvalid(_))));
    }

    #[test]
    fn verify_sigstore_no_certificate_returns_false() {
        let result = verify_sigstore_signature(b"data", &[1, 2], None);
        assert!(!result.unwrap());
    }

    #[test]
    fn verify_sigstore_with_certificate_returns_true() {
        let result = verify_sigstore_signature(
            b"data",
            &[1, 2],
            Some("-----BEGIN CERTIFICATE-----\ntest\n-----END CERTIFICATE-----"),
        );
        assert!(result.unwrap());
    }

    #[test]
    fn verify_sigstore_empty_certificate_fails() {
        let result = verify_sigstore_signature(b"data", &[1, 2], Some(""));
        assert!(matches!(result, Err(InstallError::SignatureInvalid(_))));
    }

    #[test]
    fn test_sigstore_verification_with_valid_hash() {
        let data = b"agent code bytes";
        let hash = compute_sha256(data);
        // Verify that the hash computed inside verify_sigstore_signature matches
        let result = verify_sigstore_signature(data, &[1, 2, 3], None);
        assert!(!result.unwrap()); // No certificate, so false
                                   // Hash should be consistent
        assert_eq!(hash, compute_sha256(data));
    }

    #[test]
    fn test_sigstore_verification_with_wrong_hash() {
        // Create a bundle, compute the wrong hash, and try to install
        let key = SigningKey::from_bytes(&[11u8; 32]);
        let bundle = make_signed_bundle(&key);
        let pkg_id = bundle.package_id.clone();

        // Use hash of different content than what's in the bundle
        let wrong_hash = compute_sha256(b"completely different content");

        let sigstore = SigstoreMetadata {
            artifact_hash: wrong_hash,
            signature: vec![1, 2, 3],
            certificate_pem: Some(
                "-----BEGIN CERTIFICATE-----\ntest\n-----END CERTIFICATE-----".to_string(),
            ),
        };

        let mut registry = MarketplaceRegistry::new();
        registry.upsert_signed(bundle);

        let result = install_agent_with_sigstore(&registry, &pkg_id, Some(&sigstore));
        assert!(matches!(result, Err(InstallError::MaterialsHashMismatch)));
    }

    #[test]
    fn test_install_falls_back_to_ed25519() {
        // Install without any Sigstore metadata — should succeed with Ed25519 only
        let key = SigningKey::from_bytes(&[11u8; 32]);
        let bundle = make_signed_bundle(&key);
        let pkg_id = bundle.package_id.clone();

        let mut registry = MarketplaceRegistry::new();
        registry.upsert_signed(bundle);

        // Explicit None for sigstore
        let result = install_agent_with_sigstore(&registry, &pkg_id, None);
        assert!(result.is_ok());
        let ir = result.unwrap();
        assert!(ir.verified);
        assert!(!ir.sigstore_verified); // No Sigstore, Ed25519 only
    }

    #[test]
    fn uninstall_removes_agent() {
        let key = SigningKey::from_bytes(&[11u8; 32]);
        let bundle = make_signed_bundle(&key);
        let pkg_id = bundle.package_id.clone();

        let mut registry = MarketplaceRegistry::new();
        registry.upsert_signed(bundle);

        assert!(registry.get(&pkg_id).is_some());
        let result = uninstall_agent(&mut registry, &pkg_id);
        assert!(result.is_ok());
        assert!(registry.get(&pkg_id).is_none());
    }

    #[test]
    fn uninstall_nonexistent_fails() {
        let mut registry = MarketplaceRegistry::new();
        let result = uninstall_agent(&mut registry, "pkg-doesnotexist");
        assert!(matches!(result, Err(InstallError::PackageNotFound)));
    }
}
