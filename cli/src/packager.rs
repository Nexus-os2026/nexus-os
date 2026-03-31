//! Agent packaging for `nexus package`.
//!
//! Bundles an agent project directory into a signed `.nexus-agent` package
//! using the marketplace's `create_unsigned_bundle` and `sign_package` functions.

use nexus_crypto::CryptoIdentity;
use nexus_kernel::manifest::parse_manifest;
use nexus_marketplace::package::{
    create_unsigned_bundle, sign_package, verify_package, PackageMetadata, SignedPackageBundle,
};
use std::path::Path;

/// Result of a successful packaging operation.
#[derive(Debug)]
pub struct PackageResult {
    pub package_id: String,
    pub agent_name: String,
    pub version: String,
    pub bundle_size: usize,
    pub output_path: String,
}

/// Package an agent project directory into a signed `.nexus-agent` bundle.
///
/// The directory must contain:
/// - `manifest.toml` — valid agent manifest
/// - `src/lib.rs`    — agent source code (bundled as agent_code)
///
/// Optionally:
/// - `README.md`     — included in metadata description
pub fn package_agent(
    project_dir: &Path,
    signing_key: &CryptoIdentity,
) -> Result<PackageResult, String> {
    let manifest_path = project_dir.join("manifest.toml");
    let source_path = project_dir.join("src/lib.rs");

    let manifest_content = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Cannot read manifest.toml: {e}"))?;
    let manifest =
        parse_manifest(&manifest_content).map_err(|e| format!("Invalid manifest.toml: {e}"))?;

    let agent_code = std::fs::read_to_string(&source_path)
        .map_err(|e| format!("Cannot read src/lib.rs: {e}"))?;

    let description = read_description(project_dir);

    let bundle = build_signed_bundle(
        &manifest_content,
        &agent_code,
        &manifest.name,
        &manifest.version,
        &description,
        &manifest.capabilities,
        signing_key,
    )?;

    // Verify the bundle we just created
    verify_package(&bundle).map_err(|e| format!("Self-verification failed: {e}"))?;

    let bundle_json =
        serde_json::to_vec(&bundle).map_err(|e| format!("Failed to serialize bundle: {e}"))?;
    let bundle_size = bundle_json.len();

    let output_filename = format!("{}-{}.nexus-agent", manifest.name, manifest.version);
    let output_path = project_dir.join(&output_filename);
    std::fs::write(&output_path, &bundle_json)
        .map_err(|e| format!("Failed to write {}: {e}", output_path.display()))?;

    Ok(PackageResult {
        package_id: bundle.package_id,
        agent_name: manifest.name,
        version: manifest.version,
        bundle_size,
        output_path: output_path.display().to_string(),
    })
}

/// Build a signed bundle from raw components (useful for testing without filesystem).
pub fn build_signed_bundle(
    manifest_toml: &str,
    agent_code: &str,
    name: &str,
    version: &str,
    description: &str,
    capabilities: &[String],
    signing_key: &CryptoIdentity,
) -> Result<SignedPackageBundle, String> {
    let metadata = PackageMetadata {
        name: name.to_string(),
        version: version.to_string(),
        description: description.to_string(),
        capabilities: capabilities.to_vec(),
        tags: vec![],
        author_id: format!("dev-{}", hex::encode(&signing_key.verifying_key()[..8])),
    };

    let unsigned = create_unsigned_bundle(
        manifest_toml,
        agent_code,
        metadata,
        &format!("local://{name}"),
        "nexus-cli",
    )
    .map_err(|e| format!("Failed to create bundle: {e}"))?;

    sign_package(unsigned, signing_key).map_err(|e| format!("Failed to sign bundle: {e}"))
}

/// Read the project README for use as description, or return a default.
fn read_description(project_dir: &Path) -> String {
    let readme_path = project_dir.join("README.md");
    match std::fs::read_to_string(&readme_path) {
        Ok(content) => {
            // Use first non-heading, non-empty line as description
            content
                .lines()
                .find(|line| {
                    let trimmed = line.trim();
                    !trimmed.is_empty() && !trimmed.starts_with('#')
                })
                .unwrap_or("A Nexus agent")
                .to_string()
        }
        Err(_) => "A Nexus agent".to_string(),
    }
}

/// Format a package result for CLI display.
pub fn format_result(result: &PackageResult) -> String {
    format!(
        "nexus package: OK\n\
         Agent:      {} v{}\n\
         Package ID: {}\n\
         Bundle:     {} bytes\n\
         Output:     {}\n\n\
         The bundle is signed with Ed25519 and includes an In-Toto attestation.\n\
         Publish to marketplace with: nexus marketplace publish {}\n",
        result.agent_name,
        result.version,
        result.package_id,
        result.bundle_size,
        result.output_path,
        result.output_path,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_crypto::SignatureAlgorithm;
    use nexus_marketplace::package::verify_package;

    fn test_signing_key() -> CryptoIdentity {
        CryptoIdentity::from_bytes(SignatureAlgorithm::Ed25519, &[42u8; 32]).unwrap()
    }

    fn valid_manifest() -> &'static str {
        r#"name = "test-agent"
version = "0.1.0"
capabilities = ["llm.query"]
fuel_budget = 10000
"#
    }

    fn valid_agent_code() -> &'static str {
        "pub fn run() { /* agent logic */ }"
    }

    #[test]
    fn build_signed_bundle_succeeds() {
        let key = test_signing_key();
        let bundle = build_signed_bundle(
            valid_manifest(),
            valid_agent_code(),
            "test-agent",
            "0.1.0",
            "A test agent",
            &["llm.query".to_string()],
            &key,
        );
        assert!(bundle.is_ok());
        let bundle = bundle.unwrap();
        assert!(bundle.package_id.starts_with("pkg-"));
        assert_eq!(bundle.metadata.name, "test-agent");
    }

    #[test]
    fn signed_bundle_verifies() {
        let key = test_signing_key();
        let bundle = build_signed_bundle(
            valid_manifest(),
            valid_agent_code(),
            "test-agent",
            "0.1.0",
            "A test agent",
            &["llm.query".to_string()],
            &key,
        )
        .unwrap();

        let verified = verify_package(&bundle);
        assert!(verified.is_ok());
    }

    #[test]
    fn tampered_bundle_fails_verification() {
        let key = test_signing_key();
        let mut bundle = build_signed_bundle(
            valid_manifest(),
            valid_agent_code(),
            "test-agent",
            "0.1.0",
            "A test agent",
            &["llm.query".to_string()],
            &key,
        )
        .unwrap();

        // Tamper with the agent code after signing
        bundle.agent_code.push_str("\n// injected malware");
        let verified = verify_package(&bundle);
        assert!(verified.is_err());
    }

    #[test]
    fn package_agent_from_directory() {
        let tmp = std::env::temp_dir().join("nexus-package-test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("src")).unwrap();

        std::fs::write(tmp.join("manifest.toml"), valid_manifest()).unwrap();
        std::fs::write(tmp.join("src/lib.rs"), valid_agent_code()).unwrap();
        std::fs::write(
            tmp.join("README.md"),
            "# Test Agent\nA test agent for packaging.",
        )
        .unwrap();

        let key = test_signing_key();
        let result = package_agent(&tmp, &key).unwrap();

        assert_eq!(result.agent_name, "test-agent");
        assert_eq!(result.version, "0.1.0");
        assert!(result.bundle_size > 0);
        assert!(result.output_path.ends_with(".nexus-agent"));

        // Verify the written file is a valid bundle
        let bundle_bytes = std::fs::read(&result.output_path).unwrap();
        let bundle: SignedPackageBundle = serde_json::from_slice(&bundle_bytes).unwrap();
        assert!(verify_package(&bundle).is_ok());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn package_agent_missing_manifest_fails() {
        let tmp = std::env::temp_dir().join("nexus-package-no-manifest");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let key = test_signing_key();
        let result = package_agent(&tmp, &key);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("manifest.toml"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn package_agent_missing_source_fails() {
        let tmp = std::env::temp_dir().join("nexus-package-no-source");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("manifest.toml"), valid_manifest()).unwrap();

        let key = test_signing_key();
        let result = package_agent(&tmp, &key);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("src/lib.rs"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn different_keys_produce_different_signatures() {
        let key1 = CryptoIdentity::from_bytes(SignatureAlgorithm::Ed25519, &[1u8; 32]).unwrap();
        let key2 = CryptoIdentity::from_bytes(SignatureAlgorithm::Ed25519, &[2u8; 32]).unwrap();

        let bundle1 = build_signed_bundle(
            valid_manifest(),
            valid_agent_code(),
            "test-agent",
            "0.1.0",
            "test",
            &["llm.query".to_string()],
            &key1,
        )
        .unwrap();

        let bundle2 = build_signed_bundle(
            valid_manifest(),
            valid_agent_code(),
            "test-agent",
            "0.1.0",
            "test",
            &["llm.query".to_string()],
            &key2,
        )
        .unwrap();

        assert_ne!(bundle1.signature, bundle2.signature);
        assert_ne!(bundle1.author_public_key, bundle2.author_public_key);
        // Both should still verify against their own keys
        assert!(verify_package(&bundle1).is_ok());
        assert!(verify_package(&bundle2).is_ok());
    }

    #[test]
    fn format_result_contains_key_info() {
        let result = PackageResult {
            package_id: "pkg-abc123".into(),
            agent_name: "my-agent".into(),
            version: "1.0.0".into(),
            bundle_size: 4096,
            output_path: "/tmp/my-agent-1.0.0.nexus-agent".into(),
        };
        let formatted = format_result(&result);
        assert!(formatted.contains("my-agent"));
        assert!(formatted.contains("pkg-abc123"));
        assert!(formatted.contains("4096"));
        assert!(formatted.contains(".nexus-agent"));
    }
}
