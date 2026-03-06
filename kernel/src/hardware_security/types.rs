use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
    fn attest(&self) -> Result<AttestationReport, KeyError>;
}

pub(crate) fn deterministic_attestation(backend: &str, available: bool) -> AttestationReport {
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

    AttestationReport {
        backend: backend.to_string(),
        available,
        device_claims_hash: sha256_bytes(input.as_slice()),
        protocol_version: 1,
    }
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
