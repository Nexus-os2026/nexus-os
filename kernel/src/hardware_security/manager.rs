use crate::audit::{AuditTrail, EventType};
use crate::hardware_security::software::SoftwareBackend;
use crate::hardware_security::stubs::{SecureEnclaveBackend, TeeBackend, TpmBackend};
use crate::hardware_security::types::{hex_encode, KeyBackend, KeyError, KeyHandle, KeyPurpose};
use crate::hardware_security::{AttestationReport, PublicKeyBytes, SignatureBytes};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyBackendKind {
    Software,
    Tpm,
    SecureEnclave,
    Tee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyManagerConfig {
    pub preferred_backend: KeyBackendKind,
    pub enable_hardware: bool,
}

impl Default for KeyManagerConfig {
    fn default() -> Self {
        Self {
            preferred_backend: KeyBackendKind::Software,
            enable_hardware: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RotationApproval {
    pub approver_count: u8,
}

impl RotationApproval {
    pub fn new(approver_count: u8) -> Self {
        Self { approver_count }
    }
}

pub struct KeyManager {
    backend_kind: KeyBackendKind,
    backend: Box<dyn KeyBackend>,
    deprecated_handles: Vec<KeyHandle>,
}

impl Default for KeyManager {
    fn default() -> Self {
        Self::from_config(KeyManagerConfig::default())
    }
}

impl KeyManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_config(config: KeyManagerConfig) -> Self {
        let mut backend_kind = config.preferred_backend;
        let mut backend = build_backend(config.preferred_backend, config.enable_hardware);

        if backend_kind != KeyBackendKind::Software && !backend.is_available() {
            backend_kind = KeyBackendKind::Software;
            backend = Box::new(SoftwareBackend::default());
        }

        Self {
            backend_kind,
            backend,
            deprecated_handles: Vec::new(),
        }
    }

    pub fn backend_kind(&self) -> KeyBackendKind {
        self.backend_kind
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend.backend_name()
    }

    pub fn generate_key(
        &mut self,
        purpose: KeyPurpose,
        audit: &mut AuditTrail,
        actor_id: Uuid,
    ) -> Result<KeyHandle, KeyError> {
        let handle = self.backend.generate_ed25519(purpose)?;
        let public_key = self.backend.public_key(&handle)?;
        let public_key_hash = public_key_hash(public_key);

        let _ = audit.append_event(
            actor_id,
            EventType::StateChange,
            json!({
                "event": "keys.generated",
                "key_handle_id": handle.id,
                "purpose": handle.purpose.as_str(),
                "backend_name": self.backend_name(),
                "public_key_hash": public_key_hash
            }),
        );
        Ok(handle)
    }

    pub fn public_key_bytes(&self, handle: &KeyHandle) -> Result<PublicKeyBytes, KeyError> {
        self.backend.public_key(handle)
    }

    pub fn sign_with_key(
        &self,
        handle: &KeyHandle,
        msg: &[u8],
    ) -> Result<SignatureBytes, KeyError> {
        self.backend.sign(handle, msg)
    }

    pub fn sign_audit_event_hash(
        &self,
        handle: &KeyHandle,
        event_hash: &str,
    ) -> Result<SignatureBytes, KeyError> {
        self.sign_with_key(handle, event_hash.as_bytes())
    }

    pub fn rotate_key(
        &mut self,
        handle: &KeyHandle,
        approval: RotationApproval,
        audit: &mut AuditTrail,
        actor_id: Uuid,
    ) -> Result<KeyHandle, KeyError> {
        let required = required_approvals(handle.purpose);
        if approval.approver_count < required {
            return Err(KeyError::ApprovalRequired {
                required,
                provided: approval.approver_count,
                purpose: handle.purpose,
            });
        }

        // TODO: enforce rotation via centralized HITL ConsentPolicyEngine once that module is
        // available in this kernel branch. This API currently requires explicit approvals input.
        let old_public_hash = public_key_hash(self.backend.public_key(handle)?);
        let new_handle = self.backend.rotate(handle)?;
        let new_public_hash = public_key_hash(self.backend.public_key(&new_handle)?);

        self.deprecated_handles.push(handle.clone());
        let _ = audit.append_event(
            actor_id,
            EventType::StateChange,
            json!({
                "event": "keys.rotated",
                "old_key_handle_id": handle.id,
                "new_key_handle_id": new_handle.id,
                "purpose": handle.purpose.as_str(),
                "backend_name": self.backend_name(),
                "old_public_key_hash": old_public_hash,
                "new_public_key_hash": new_public_hash
            }),
        );

        Ok(new_handle)
    }

    pub fn deprecated_handles(&self) -> &[KeyHandle] {
        &self.deprecated_handles
    }

    pub fn generate_attestation(
        &self,
        audit: &mut AuditTrail,
        actor_id: Uuid,
    ) -> Result<AttestationReport, KeyError> {
        let report = self.backend.attest()?;
        let _ = audit.append_event(
            actor_id,
            EventType::StateChange,
            json!({
                "event": "attestation.generated",
                "backend_name": report.backend,
                "available": report.available,
                "device_claims_hash": hex_encode(&report.device_claims_hash),
                "protocol_version": report.protocol_version
            }),
        );
        Ok(report)
    }
}

fn build_backend(kind: KeyBackendKind, enable_hardware: bool) -> Box<dyn KeyBackend> {
    match kind {
        KeyBackendKind::Software => Box::new(SoftwareBackend::default()),
        KeyBackendKind::Tpm => Box::new(TpmBackend::new(enable_hardware)),
        KeyBackendKind::SecureEnclave => Box::new(SecureEnclaveBackend::new(enable_hardware)),
        KeyBackendKind::Tee => Box::new(TeeBackend::new(enable_hardware)),
    }
}

fn required_approvals(purpose: KeyPurpose) -> u8 {
    match purpose {
        KeyPurpose::AgentIdentity => 1,
        KeyPurpose::AuditSigning | KeyPurpose::NodeIdentity | KeyPurpose::ApprovalSigning => 2,
    }
}

fn public_key_hash(public_key: PublicKeyBytes) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(public_key.0);
    let digest = hasher.finalize();
    format!("{digest:x}")
}
