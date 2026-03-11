use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketplaceError {
    SignatureInvalid,
    AttestationInvalid,
    PackageNotFound,
    SerializationError(String),
}

impl std::fmt::Display for MarketplaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketplaceError::SignatureInvalid => write!(f, "package signature is invalid"),
            MarketplaceError::AttestationInvalid => write!(f, "attestation validation failed"),
            MarketplaceError::PackageNotFound => write!(f, "package not found"),
            MarketplaceError::SerializationError(reason) => {
                write!(f, "serialization failed: {reason}")
            }
        }
    }
}

impl std::error::Error for MarketplaceError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub tags: Vec<String>,
    pub author_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InTotoAttestation {
    pub predicate_type: String,
    pub builder_id: String,
    pub source_uri: String,
    pub invocation_id: String,
    pub materials_sha256: String,
    pub generated_at_unix: u64,
    #[serde(default)]
    pub slsa_level: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_environment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sbom_reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedPackageBundle {
    pub manifest_toml: String,
    pub agent_code: String,
    pub metadata: PackageMetadata,
    pub attestation: InTotoAttestation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedPackageBundle {
    pub package_id: String,
    pub manifest_toml: String,
    pub agent_code: String,
    pub metadata: PackageMetadata,
    pub attestation: InTotoAttestation,
    pub signature: Vec<u8>,
    pub author_public_key: Vec<u8>,
}

#[derive(Debug, Serialize)]
struct SigningPayload<'a> {
    package_id: &'a str,
    manifest_toml: &'a str,
    agent_code: &'a str,
    metadata: &'a PackageMetadata,
    attestation: &'a InTotoAttestation,
}

pub fn bundle_materials_hash(
    manifest_toml: &str,
    agent_code: &str,
    metadata: &PackageMetadata,
) -> Result<String, MarketplaceError> {
    let serialized_metadata = serde_json::to_vec(metadata)
        .map_err(|error| MarketplaceError::SerializationError(error.to_string()))?;

    let mut hasher = Sha256::new();
    hasher.update(manifest_toml.as_bytes());
    hasher.update(b"\n---manifest-agent-boundary---\n");
    hasher.update(agent_code.as_bytes());
    hasher.update(b"\n---agent-metadata-boundary---\n");
    hasher.update(&serialized_metadata);
    Ok(hex::encode(hasher.finalize()))
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn create_attestation(
    manifest_toml: &str,
    agent_code: &str,
    metadata: &PackageMetadata,
    source_uri: &str,
    builder_id: &str,
) -> Result<InTotoAttestation, MarketplaceError> {
    create_attestation_with_options(
        manifest_toml,
        agent_code,
        metadata,
        source_uri,
        builder_id,
        None,
    )
}

/// Create an attestation with optional overrides for testing.
/// Pass `timestamp_override` to use a deterministic timestamp instead of real time.
pub fn create_attestation_with_options(
    manifest_toml: &str,
    agent_code: &str,
    metadata: &PackageMetadata,
    source_uri: &str,
    builder_id: &str,
    timestamp_override: Option<u64>,
) -> Result<InTotoAttestation, MarketplaceError> {
    Ok(InTotoAttestation {
        predicate_type: "https://in-toto.io/Statement/v1".to_string(),
        builder_id: builder_id.to_string(),
        source_uri: source_uri.to_string(),
        invocation_id: uuid::Uuid::new_v4().to_string(),
        materials_sha256: bundle_materials_hash(manifest_toml, agent_code, metadata)?,
        generated_at_unix: timestamp_override.unwrap_or_else(current_unix_timestamp),
        slsa_level: 1,
        build_environment: None,
        sbom_reference: None,
    })
}

pub fn create_unsigned_bundle(
    manifest_toml: &str,
    agent_code: &str,
    metadata: PackageMetadata,
    source_uri: &str,
    builder_id: &str,
) -> Result<UnsignedPackageBundle, MarketplaceError> {
    let attestation =
        create_attestation(manifest_toml, agent_code, &metadata, source_uri, builder_id)?;

    Ok(UnsignedPackageBundle {
        manifest_toml: manifest_toml.to_string(),
        agent_code: agent_code.to_string(),
        metadata,
        attestation,
    })
}

pub fn sign_package(
    bundle: UnsignedPackageBundle,
    author_key: &SigningKey,
) -> Result<SignedPackageBundle, MarketplaceError> {
    let package_id = derive_package_id(
        bundle.metadata.name.as_str(),
        bundle.metadata.version.as_str(),
        bundle.attestation.materials_sha256.as_str(),
    );
    let payload = canonical_signing_payload(
        package_id.as_str(),
        bundle.manifest_toml.as_str(),
        bundle.agent_code.as_str(),
        &bundle.metadata,
        &bundle.attestation,
    )?;
    let signature = author_key.sign(payload.as_slice()).to_bytes().to_vec();

    Ok(SignedPackageBundle {
        package_id,
        manifest_toml: bundle.manifest_toml,
        agent_code: bundle.agent_code,
        metadata: bundle.metadata,
        attestation: bundle.attestation,
        signature,
        author_public_key: author_key.verifying_key().to_bytes().to_vec(),
    })
}

pub fn verify_package(bundle: &SignedPackageBundle) -> Result<(), MarketplaceError> {
    verify_attestation(bundle)?;

    let public_key_bytes: [u8; 32] = bundle
        .author_public_key
        .as_slice()
        .try_into()
        .map_err(|_| MarketplaceError::SignatureInvalid)?;
    let signature_bytes: [u8; 64] = bundle
        .signature
        .as_slice()
        .try_into()
        .map_err(|_| MarketplaceError::SignatureInvalid)?;

    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)
        .map_err(|_| MarketplaceError::SignatureInvalid)?;
    let signature = Signature::from_bytes(&signature_bytes);
    let payload = canonical_signing_payload(
        bundle.package_id.as_str(),
        bundle.manifest_toml.as_str(),
        bundle.agent_code.as_str(),
        &bundle.metadata,
        &bundle.attestation,
    )?;

    verifying_key
        .verify(payload.as_slice(), &signature)
        .map_err(|_| MarketplaceError::SignatureInvalid)
}

pub fn verify_attestation(bundle: &SignedPackageBundle) -> Result<(), MarketplaceError> {
    if bundle.attestation.predicate_type != "https://in-toto.io/Statement/v1" {
        return Err(MarketplaceError::AttestationInvalid);
    }

    let expected_hash = bundle_materials_hash(
        bundle.manifest_toml.as_str(),
        bundle.agent_code.as_str(),
        &bundle.metadata,
    )?;
    if expected_hash != bundle.attestation.materials_sha256 {
        return Err(MarketplaceError::AttestationInvalid);
    }

    Ok(())
}

fn canonical_signing_payload(
    package_id: &str,
    manifest_toml: &str,
    agent_code: &str,
    metadata: &PackageMetadata,
    attestation: &InTotoAttestation,
) -> Result<Vec<u8>, MarketplaceError> {
    serde_json::to_vec(&SigningPayload {
        package_id,
        manifest_toml,
        agent_code,
        metadata,
        attestation,
    })
    .map_err(|error| MarketplaceError::SerializationError(error.to_string()))
}

fn derive_package_id(name: &str, version: &str, materials_sha256: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    hasher.update(b":");
    hasher.update(version.as_bytes());
    hasher.update(b":");
    hasher.update(materials_sha256.as_bytes());
    let digest = hex::encode(hasher.finalize());
    format!("pkg-{}", &digest[0..16])
}

#[cfg(test)]
mod tests {
    use super::{
        create_attestation, create_attestation_with_options, create_unsigned_bundle, sign_package,
        verify_package, InTotoAttestation, MarketplaceError, PackageMetadata,
    };
    use ed25519_dalek::SigningKey;

    fn test_metadata() -> PackageMetadata {
        PackageMetadata {
            name: "rust-social-poster".to_string(),
            version: "1.0.0".to_string(),
            description: "Posts Rust updates every morning".to_string(),
            capabilities: vec!["social.post".to_string(), "llm.query".to_string()],
            tags: vec!["social".to_string(), "rust".to_string()],
            author_id: "author-123".to_string(),
        }
    }

    const TEST_MANIFEST: &str = r#"name = "rust-social-poster"
version = "1.0.0"
capabilities = ["social.post", "llm.query"]
fuel_budget = 5000
"#;
    const TEST_CODE: &str = "fn run() { /* publish */ }";

    #[test]
    fn test_package_sign_and_verify() {
        let unsigned = create_unsigned_bundle(
            TEST_MANIFEST,
            TEST_CODE,
            test_metadata(),
            "https://github.com/example/agent",
            "nexus-buildkit",
        )
        .expect("unsigned package should be created");
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);

        let mut signed = sign_package(unsigned, &signing_key).expect("package should be signed");
        let verified = verify_package(&signed);
        assert!(verified.is_ok());

        signed.agent_code.push_str("\n// tampered payload");
        let tampered = verify_package(&signed);
        assert!(tampered.is_err());
        assert!(
            tampered == Err(MarketplaceError::AttestationInvalid)
                || tampered == Err(MarketplaceError::SignatureInvalid)
        );
    }

    #[test]
    fn test_attestation_uses_real_timestamp() {
        let attestation = create_attestation(
            TEST_MANIFEST,
            TEST_CODE,
            &test_metadata(),
            "https://github.com/example/agent",
            "nexus-buildkit",
        )
        .expect("attestation should be created");

        // Real timestamp should be well past the old hardcoded 1_700_000_000
        assert!(attestation.generated_at_unix > 1_700_000_000);
        assert_eq!(attestation.slsa_level, 1);
        assert!(attestation.build_environment.is_none());
        assert!(attestation.sbom_reference.is_none());
    }

    #[test]
    fn test_attestation_with_timestamp_override() {
        let attestation = create_attestation_with_options(
            TEST_MANIFEST,
            TEST_CODE,
            &test_metadata(),
            "https://github.com/example/agent",
            "nexus-buildkit",
            Some(1_700_000_000),
        )
        .expect("attestation should be created");

        assert_eq!(attestation.generated_at_unix, 1_700_000_000);
        assert_eq!(attestation.slsa_level, 1);
    }

    #[test]
    fn test_attestation_new_fields_default_on_deserialize() {
        // Simulate a legacy attestation without the new fields
        let legacy_json = serde_json::json!({
            "predicate_type": "https://in-toto.io/Statement/v1",
            "builder_id": "nexus-buildkit",
            "source_uri": "https://example.com",
            "invocation_id": "test-id",
            "materials_sha256": "deadbeef",
            "generated_at_unix": 1_700_000_000
        });

        let attestation: InTotoAttestation =
            serde_json::from_value(legacy_json).expect("legacy attestation should deserialize");
        assert_eq!(attestation.slsa_level, 0);
        assert!(attestation.build_environment.is_none());
        assert!(attestation.sbom_reference.is_none());
    }
}
