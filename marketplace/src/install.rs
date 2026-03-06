//! Agent installation and uninstallation with signature and capability verification.

use crate::package::{bundle_materials_hash, verify_package, MarketplaceError};
use crate::registry::MarketplaceRegistry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallResult {
    pub package_id: String,
    pub name: String,
    pub version: String,
    pub verified: bool,
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

/// Install an agent from the registry. Verifies manifest signature and checks capabilities.
pub fn install_agent(
    registry: &MarketplaceRegistry,
    package_id: &str,
) -> Result<InstallResult, InstallError> {
    let bundle = registry
        .get(package_id)
        .ok_or(InstallError::PackageNotFound)?;

    // Verify materials hash
    let computed_hash = bundle_materials_hash(
        &bundle.manifest_toml,
        &bundle.agent_code,
        &bundle.metadata,
    )?;
    if computed_hash != bundle.attestation.materials_sha256 {
        return Err(InstallError::MaterialsHashMismatch);
    }

    // Verify full Ed25519 package signature (attestation + canonical payload)
    verify_package(bundle).map_err(|e| InstallError::SignatureInvalid(e.to_string()))?;

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
    use crate::package::{create_unsigned_bundle, sign_package, PackageMetadata, SignedPackageBundle};
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
