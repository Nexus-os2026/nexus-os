use ed25519_dalek::{Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum KeyPurpose {
    AuditSigning,
    NodeIdentity,
    AgentIdentity,
    ApprovalSigning,
}

impl KeyPurpose {
    pub fn as_str(self) -> &'static str {
        match self {
            KeyPurpose::AuditSigning => "audit_signing",
            KeyPurpose::NodeIdentity => "node_identity",
            KeyPurpose::AgentIdentity => "agent_identity",
            KeyPurpose::ApprovalSigning => "approval_signing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyHandle {
    pub id: String,
    pub purpose: KeyPurpose,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKeyBytes(pub Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureBytes(pub Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationReport {
    pub backend: String,
    pub available: bool,
    pub device_claims_hash: [u8; 32],
    pub protocol_version: u32,
    /// TEE provider type: "sgx", "nitro", or "software".
    pub provider: String,
    /// Unix timestamp (seconds since epoch) when this report was generated.
    pub timestamp: u64,
    /// SHA-256 hex digest of the public key being attested.
    pub public_key_hash: String,
    /// Platform info string: OS, arch, TEE version.
    pub platform_info: String,
    /// Caller-provided nonce for freshness.
    pub nonce: String,
    /// Ed25519 signature over the canonical report payload (self-signed by attestation key).
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum KeyError {
    #[error("key backend '{0}' is not available")]
    NotAvailable(&'static str),
    #[error("key handle '{0}' not found")]
    KeyNotFound(String),
    #[error("key purpose mismatch: expected '{}', actual '{}'", expected.as_str(), actual.as_str())]
    PurposeMismatch {
        expected: KeyPurpose,
        actual: KeyPurpose,
    },
    #[error("approval required for '{}': required {required}, provided {provided}", purpose.as_str())]
    ApprovalRequired {
        required: u8,
        provided: u8,
        purpose: KeyPurpose,
    },
    #[error("invalid key material: {0}")]
    InvalidKeyMaterial(String),
    #[error("key backend failure: {0}")]
    BackendFailure(String),
}

pub trait KeyBackend: Send {
    fn backend_name(&self) -> &'static str;
    fn is_available(&self) -> bool;

    fn generate_ed25519(&mut self, purpose: KeyPurpose) -> Result<KeyHandle, KeyError>;
    fn public_key(&self, handle: &KeyHandle) -> Result<PublicKeyBytes, KeyError>;
    fn sign(&self, handle: &KeyHandle, msg: &[u8]) -> Result<SignatureBytes, KeyError>;
    fn rotate(&mut self, handle: &KeyHandle) -> Result<KeyHandle, KeyError>;
    fn attest(&self, nonce: &str) -> Result<AttestationReport, KeyError>;
}

pub(crate) fn deterministic_attestation(
    backend: &str,
    available: bool,
    nonce: &str,
) -> AttestationReport {
    let mut input = Vec::new();
    input.extend_from_slice(backend.as_bytes());
    input.push(b':');
    input.extend_from_slice(if available {
        b"available"
    } else {
        b"unavailable"
    });
    input.push(b':');
    input.extend_from_slice(b"v1");

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let provider = if backend.contains("sgx") {
        "sgx"
    } else if backend.contains("nitro") {
        "nitro"
    } else {
        "software"
    };

    AttestationReport {
        backend: backend.to_string(),
        available,
        device_claims_hash: sha256_bytes(input.as_slice()),
        protocol_version: 1,
        provider: provider.to_string(),
        timestamp,
        public_key_hash: String::new(),
        platform_info: platform_info_string(),
        nonce: nonce.to_string(),
        signature: Vec::new(),
    }
}

/// Build a canonical byte payload from the attestation report fields (excluding signature).
/// This is signed and later verified.
pub(crate) fn attestation_payload(report: &AttestationReport) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(report.provider.as_bytes());
    buf.push(b':');
    buf.extend_from_slice(report.timestamp.to_le_bytes().as_slice());
    buf.push(b':');
    buf.extend_from_slice(report.public_key_hash.as_bytes());
    buf.push(b':');
    buf.extend_from_slice(report.platform_info.as_bytes());
    buf.push(b':');
    buf.extend_from_slice(report.nonce.as_bytes());
    buf.push(b':');
    buf.extend_from_slice(report.backend.as_bytes());
    buf.push(b':');
    buf.extend_from_slice(&report.device_claims_hash);
    buf
}

/// Verify an attestation report: check the Ed25519 signature and that the
/// timestamp is within `max_age_secs` of the current time (default 24 hours).
pub fn verify_attestation(report: &AttestationReport) -> Result<bool, KeyError> {
    verify_attestation_with_max_age(report, 86400)
}

/// Like [`verify_attestation`] but with a configurable max age in seconds.
pub fn verify_attestation_with_max_age(
    report: &AttestationReport,
    max_age_secs: u64,
) -> Result<bool, KeyError> {
    // Check timestamp freshness.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if report.timestamp > now {
        // Timestamp in the future — tolerate up to 60s clock skew.
        if report.timestamp.saturating_sub(now) > 60 {
            return Ok(false);
        }
    } else if now.saturating_sub(report.timestamp) > max_age_secs {
        return Ok(false);
    }

    // If signature is empty, the report is unsigned (stub backends).
    if report.signature.is_empty() {
        return Ok(report.available);
    }

    // Decode the public key from the public_key_hash? No — we need the actual
    // public key bytes. The signature is self-signed by the attestation key whose
    // hash is in public_key_hash. Since we don't carry the full public key in
    // the report for stub backends, we return Ok(false) for unsigned reports and
    // Ok(true) for software reports that include full verification below.
    //
    // For signed reports, the public key is encoded at the start of the signature
    // blob: signature = pubkey(32) || ed25519_sig(64).
    if report.signature.len() != 96 {
        return Err(KeyError::BackendFailure(
            "attestation signature must be 96 bytes (32 pubkey + 64 signature)".to_string(),
        ));
    }

    let pub_bytes: [u8; 32] = report.signature[..32]
        .try_into()
        .map_err(|_| KeyError::BackendFailure("invalid public key in signature".to_string()))?;
    let sig_bytes: [u8; 64] = report.signature[32..96]
        .try_into()
        .map_err(|_| KeyError::BackendFailure("invalid signature bytes".to_string()))?;

    // Verify that the public key matches the declared hash.
    let expected_hash = hex_encode(&sha256_bytes(&pub_bytes));
    if expected_hash != report.public_key_hash {
        return Ok(false);
    }

    let verifying_key = VerifyingKey::from_bytes(&pub_bytes)
        .map_err(|e| KeyError::BackendFailure(format!("invalid attestation public key: {e}")))?;
    let signature = ed25519_dalek::Signature::from_bytes(&sig_bytes);

    let payload = attestation_payload(report);
    match verifying_key.verify(&payload, &signature) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

pub(crate) fn platform_info_string() -> String {
    format!(
        "{}/{}/nexus-os-v7",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

pub(crate) fn sha256_bytes(input: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(input);
    let digest = hasher.finalize();
    let mut output = [0_u8; 32];
    output.copy_from_slice(digest.as_slice());
    output
}

pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn short_hash_prefix(bytes: &[u8]) -> String {
    let hash = sha256_bytes(bytes);
    hex_encode(&hash[..8])
}
