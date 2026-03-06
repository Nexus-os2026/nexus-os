use crate::package::{
    sign_package, verify_attestation, verify_package, MarketplaceError, SignedPackageBundle,
    UnsignedPackageBundle,
};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageSummary {
    pub package_id: String,
    pub name: String,
    pub description: String,
    pub author_id: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Default)]
pub struct MarketplaceRegistry {
    packages: HashMap<String, SignedPackageBundle>,
}

impl MarketplaceRegistry {
    pub fn new() -> Self {
        Self {
            packages: HashMap::new(),
        }
    }

    pub fn publish(
        &mut self,
        package: UnsignedPackageBundle,
        author_key: &SigningKey,
    ) -> Result<String, MarketplaceError> {
        let signed = sign_package(package, author_key)?;
        verify_attestation(&signed)?;
        let id = signed.package_id.clone();
        self.packages.insert(id.clone(), signed);
        Ok(id)
    }

    pub fn search(&self, query: &str) -> Vec<PackageSummary> {
        let query_lower = query.trim().to_lowercase();
        let mut matches = self
            .packages
            .values()
            .filter(|package| {
                if query_lower.is_empty() {
                    return true;
                }
                let name_hit = package
                    .metadata
                    .name
                    .to_lowercase()
                    .contains(query_lower.as_str());
                let desc_hit = package
                    .metadata
                    .description
                    .to_lowercase()
                    .contains(query_lower.as_str());
                let author_hit = package
                    .metadata
                    .author_id
                    .to_lowercase()
                    .contains(query_lower.as_str());
                let tag_hit = package
                    .metadata
                    .tags
                    .iter()
                    .any(|tag| tag.to_lowercase().contains(query_lower.as_str()));
                name_hit || desc_hit || author_hit || tag_hit
            })
            .map(|package| PackageSummary {
                package_id: package.package_id.clone(),
                name: package.metadata.name.clone(),
                description: package.metadata.description.clone(),
                author_id: package.metadata.author_id.clone(),
                tags: package.metadata.tags.clone(),
            })
            .collect::<Vec<_>>();

        matches.sort_by(|left, right| left.name.cmp(&right.name));
        matches
    }

    pub fn install(&self, package_id: &str) -> Result<SignedPackageBundle, MarketplaceError> {
        let package = self
            .packages
            .get(package_id)
            .ok_or(MarketplaceError::PackageNotFound)?;
        verify_package(package)?;
        Ok(package.clone())
    }

    pub fn get(&self, package_id: &str) -> Option<&SignedPackageBundle> {
        self.packages.get(package_id)
    }

    pub fn remove(&mut self, package_id: &str) -> bool {
        self.packages.remove(package_id).is_some()
    }

    pub fn upsert_signed(&mut self, package: SignedPackageBundle) {
        self.packages.insert(package.package_id.clone(), package);
    }

    pub fn tamper_signature_for_test(&mut self, package_id: &str) -> Result<(), MarketplaceError> {
        let package = self
            .packages
            .get_mut(package_id)
            .ok_or(MarketplaceError::PackageNotFound)?;
        if package.signature.is_empty() {
            package.signature = vec![0; 64];
        } else {
            package.signature[0] ^= 0x7f;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::MarketplaceRegistry;
    use crate::package::{create_unsigned_bundle, MarketplaceError, PackageMetadata};
    use ed25519_dalek::SigningKey;

    #[test]
    fn test_install_checks_signature() {
        let metadata = PackageMetadata {
            name: "nightly-backup".to_string(),
            version: "1.0.0".to_string(),
            description: "Back up photos every night".to_string(),
            capabilities: vec!["fs.write".to_string(), "web.search".to_string()],
            tags: vec!["backup".to_string(), "photos".to_string()],
            author_id: "author-backup".to_string(),
        };
        let package = create_unsigned_bundle(
            r#"name = "nightly-backup"
version = "1.0.0"
capabilities = ["fs.write", "web.search"]
fuel_budget = 6000
"#,
            "fn run_backup() { /* copy */ }",
            metadata,
            "https://github.com/example/nightly-backup",
            "nexus-buildkit",
        )
        .expect("bundle should build");

        let key = SigningKey::from_bytes(&[9_u8; 32]);
        let mut registry = MarketplaceRegistry::new();
        let package_id = registry
            .publish(package, &key)
            .expect("publish should sign and store package");
        registry
            .tamper_signature_for_test(package_id.as_str())
            .expect("tampering should succeed for test");

        let installed = registry.install(package_id.as_str());
        assert_eq!(installed, Err(MarketplaceError::SignatureInvalid));
    }
}
