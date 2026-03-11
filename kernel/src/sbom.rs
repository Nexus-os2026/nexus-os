//! Software Bill of Materials (SBOM) generation in CycloneDX JSON format.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type classification for an SBOM component entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SbomEntryType {
    RustCrate,
    NpmPackage,
    SystemDependency,
    WorkspaceCrate,
}

/// A single component in the SBOM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SbomEntry {
    pub name: String,
    pub version: String,
    pub purl: String,
    pub license: Option<String>,
    pub hash_sha256: Option<String>,
    pub entry_type: SbomEntryType,
}

/// Metadata about the SBOM generation context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SbomMetadata {
    pub timestamp: String,
    pub tool_name: String,
    pub tool_version: String,
    pub component_name: String,
    pub component_version: String,
    pub authors: Vec<String>,
}

/// A CycloneDX-format Software Bill of Materials document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SbomDocument {
    pub bom_format: String,
    pub spec_version: String,
    pub version: u32,
    pub serial_number: String,
    pub metadata: SbomMetadata,
    pub components: Vec<SbomEntry>,
}

impl SbomDocument {
    /// Create a new SBOM document with defaults and empty components.
    pub fn new(version: &str) -> Self {
        Self {
            bom_format: "CycloneDX".to_string(),
            spec_version: "1.5".to_string(),
            version: 1,
            serial_number: format!("urn:uuid:{}", Uuid::new_v4()),
            metadata: SbomMetadata {
                timestamp: String::new(),
                tool_name: "nexus-sbom".to_string(),
                tool_version: version.to_string(),
                component_name: "nexus-os".to_string(),
                component_version: version.to_string(),
                authors: Vec::new(),
            },
            components: Vec::new(),
        }
    }

    /// Add a component entry to this SBOM.
    pub fn add_component(&mut self, entry: SbomEntry) {
        self.components.push(entry);
    }

    /// Convenience method to add a Rust crate component.
    pub fn add_rust_crate(&mut self, name: &str, version: &str, license: Option<&str>) {
        self.components.push(SbomEntry {
            name: name.to_string(),
            version: version.to_string(),
            purl: format!("pkg:cargo/{name}@{version}"),
            license: license.map(String::from),
            hash_sha256: None,
            entry_type: SbomEntryType::RustCrate,
        });
    }

    /// Convenience method to add an npm package component.
    pub fn add_npm_package(&mut self, name: &str, version: &str, license: Option<&str>) {
        self.components.push(SbomEntry {
            name: name.to_string(),
            version: version.to_string(),
            purl: format!("pkg:npm/{name}@{version}"),
            license: license.map(String::from),
            hash_sha256: None,
            entry_type: SbomEntryType::NpmPackage,
        });
    }

    /// Serialize this SBOM to pretty-printed JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize an SBOM from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Return the number of components in this SBOM.
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// Find a component by name.
    pub fn find_component(&self, name: &str) -> Option<&SbomEntry> {
        self.components.iter().find(|c| c.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_document_defaults() {
        let doc = SbomDocument::new("7.0.0");
        assert_eq!(doc.bom_format, "CycloneDX");
        assert_eq!(doc.spec_version, "1.5");
        assert_eq!(doc.version, 1);
        assert!(doc.serial_number.starts_with("urn:uuid:"));
        assert_eq!(doc.metadata.tool_name, "nexus-sbom");
        assert_eq!(doc.metadata.tool_version, "7.0.0");
        assert_eq!(doc.metadata.component_name, "nexus-os");
        assert_eq!(doc.metadata.component_version, "7.0.0");
        assert_eq!(doc.component_count(), 0);
    }

    #[test]
    fn test_add_rust_crate() {
        let mut doc = SbomDocument::new("7.0.0");
        doc.add_rust_crate("serde", "1.0.210", Some("MIT OR Apache-2.0"));
        assert_eq!(doc.component_count(), 1);

        let entry = doc.find_component("serde").unwrap();
        assert_eq!(entry.purl, "pkg:cargo/serde@1.0.210");
        assert_eq!(entry.license.as_deref(), Some("MIT OR Apache-2.0"));
        assert_eq!(entry.entry_type, SbomEntryType::RustCrate);
    }

    #[test]
    fn test_add_npm_package() {
        let mut doc = SbomDocument::new("7.0.0");
        doc.add_npm_package("react", "18.3.1", Some("MIT"));
        assert_eq!(doc.component_count(), 1);

        let entry = doc.find_component("react").unwrap();
        assert_eq!(entry.purl, "pkg:npm/react@18.3.1");
        assert_eq!(entry.license.as_deref(), Some("MIT"));
        assert_eq!(entry.entry_type, SbomEntryType::NpmPackage);
    }

    #[test]
    fn test_add_component_generic() {
        let mut doc = SbomDocument::new("7.0.0");
        doc.add_component(SbomEntry {
            name: "libssl".to_string(),
            version: "3.0.0".to_string(),
            purl: "pkg:generic/libssl@3.0.0".to_string(),
            license: Some("Apache-2.0".to_string()),
            hash_sha256: Some("abcdef1234567890".to_string()),
            entry_type: SbomEntryType::SystemDependency,
        });
        assert_eq!(doc.component_count(), 1);
        assert_eq!(
            doc.find_component("libssl").unwrap().entry_type,
            SbomEntryType::SystemDependency
        );
    }

    #[test]
    fn test_json_roundtrip() {
        let mut doc = SbomDocument::new("7.0.0");
        doc.add_rust_crate("sha2", "0.10.8", Some("MIT OR Apache-2.0"));
        doc.add_rust_crate("serde", "1.0.210", Some("MIT OR Apache-2.0"));
        doc.add_npm_package("vite", "5.4.8", Some("MIT"));
        doc.add_npm_package("react", "18.3.1", Some("MIT"));
        doc.add_component(SbomEntry {
            name: "nexus-kernel".to_string(),
            version: "7.0.0".to_string(),
            purl: "pkg:cargo/nexus-kernel@7.0.0".to_string(),
            license: Some("MIT".to_string()),
            hash_sha256: Some("abc123".to_string()),
            entry_type: SbomEntryType::WorkspaceCrate,
        });

        let json = doc.to_json().expect("serialization should succeed");
        let restored = SbomDocument::from_json(&json).expect("deserialization should succeed");

        assert_eq!(restored.bom_format, doc.bom_format);
        assert_eq!(restored.spec_version, doc.spec_version);
        assert_eq!(restored.version, doc.version);
        assert_eq!(restored.serial_number, doc.serial_number);
        assert_eq!(restored.metadata.tool_name, doc.metadata.tool_name);
        assert_eq!(restored.component_count(), 5);
        assert!(restored.find_component("sha2").is_some());
        assert!(restored.find_component("serde").is_some());
        assert!(restored.find_component("vite").is_some());
        assert!(restored.find_component("react").is_some());
        let kernel = restored.find_component("nexus-kernel").unwrap();
        assert_eq!(kernel.entry_type, SbomEntryType::WorkspaceCrate);
        assert_eq!(kernel.hash_sha256.as_deref(), Some("abc123"));
    }

    #[test]
    fn test_find_component_missing() {
        let doc = SbomDocument::new("7.0.0");
        assert!(doc.find_component("nonexistent").is_none());
    }

    #[test]
    fn test_sbom_spec_version() {
        let doc = SbomDocument::new("7.0.0");
        assert_eq!(doc.spec_version, "1.5");
        assert_eq!(doc.bom_format, "CycloneDX");
    }

    #[test]
    fn test_multiple_components() {
        let mut doc = SbomDocument::new("7.0.0");
        doc.add_rust_crate("tokio", "1.40.0", None);
        doc.add_rust_crate("serde", "1.0.210", Some("MIT"));
        doc.add_npm_package("react", "18.3.1", Some("MIT"));
        doc.add_component(SbomEntry {
            name: "nexus-kernel".to_string(),
            version: "7.0.0".to_string(),
            purl: "pkg:cargo/nexus-kernel@7.0.0".to_string(),
            license: Some("MIT".to_string()),
            hash_sha256: None,
            entry_type: SbomEntryType::WorkspaceCrate,
        });
        assert_eq!(doc.component_count(), 4);
        assert!(doc.find_component("tokio").unwrap().license.is_none());
    }
}
