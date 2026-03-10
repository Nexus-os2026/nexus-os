use super::{KeyBackendKind, KeyError, KeyManager, KeyManagerConfig, KeyPurpose, RotationApproval};
use crate::audit::AuditTrail;
#[cfg(any(
    feature = "hardware-tpm",
    feature = "hardware-secure-enclave",
    feature = "hardware-tee"
))]
use crate::hardware_security::types::KeyBackend;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use uuid::Uuid;

#[cfg(feature = "hardware-secure-enclave")]
use super::SecureEnclaveBackend;
#[cfg(feature = "hardware-tee")]
use super::TeeBackend;
#[cfg(feature = "hardware-tpm")]
use super::TpmBackend;

fn event_exists(audit: &AuditTrail, name: &str) -> bool {
    audit
        .events()
        .iter()
        .any(|event| event.payload.get("event").and_then(|value| value.as_str()) == Some(name))
}

#[test]
fn test_software_generate_sign_verify() {
    let mut manager = KeyManager::new();
    let mut audit = AuditTrail::new();
    let actor = Uuid::nil();

    let handle = manager
        .generate_key(KeyPurpose::AuditSigning, &mut audit, actor)
        .expect("software backend should generate key");
    let message = b"audit integrity";
    let signature = manager
        .sign_with_key(&handle, message)
        .expect("software backend should sign");
    let public_key = manager
        .public_key_bytes(&handle)
        .expect("public key should be available");

    let public_key_array: [u8; 32] = public_key
        .0
        .as_slice()
        .try_into()
        .expect("public key length should be 32 bytes");
    let signature_array: [u8; 64] = signature
        .0
        .as_slice()
        .try_into()
        .expect("signature length should be 64 bytes");

    let verifying_key =
        VerifyingKey::from_bytes(&public_key_array).expect("public key should parse");
    let signature = Signature::from_bytes(&signature_array);
    let verify_result = verifying_key.verify(message, &signature);
    assert!(verify_result.is_ok());
    assert!(event_exists(&audit, "keys.generated"));
}

#[test]
fn test_rotation_emits_audit_event() {
    let mut manager = KeyManager::new();
    let mut audit = AuditTrail::new();
    let actor = Uuid::nil();

    let original = manager
        .generate_key(KeyPurpose::AgentIdentity, &mut audit, actor)
        .expect("initial key generation should succeed");
    let rotated = manager
        .rotate_key(&original, RotationApproval::new(1), &mut audit, actor)
        .expect("rotation with tier2-equivalent approval should succeed");

    assert_ne!(original.id, rotated.id);
    assert!(event_exists(&audit, "keys.rotated"));
    assert!(manager
        .deprecated_handles()
        .iter()
        .any(|handle| handle.id == original.id));
}

#[test]
fn test_rotation_requires_higher_approval_for_node_identity() {
    let mut manager = KeyManager::new();
    let mut audit = AuditTrail::new();
    let actor = Uuid::nil();
    let handle = manager
        .generate_key(KeyPurpose::NodeIdentity, &mut audit, actor)
        .expect("key generation should succeed");

    let result = manager.rotate_key(&handle, RotationApproval::new(1), &mut audit, actor);
    assert_eq!(
        result,
        Err(KeyError::ApprovalRequired {
            required: 2,
            provided: 1,
            purpose: KeyPurpose::NodeIdentity,
        })
    );
}

#[test]
fn test_attestation_deterministic_and_audited() {
    let manager = KeyManager::new();
    let mut audit = AuditTrail::new();
    let actor = Uuid::nil();

    let first = manager
        .generate_attestation(&mut audit, actor)
        .expect("attestation should succeed");
    let second = manager
        .generate_attestation(&mut audit, actor)
        .expect("attestation should be deterministic");

    assert_eq!(first, second);
    assert!(event_exists(&audit, "attestation.generated"));
}

#[test]
fn test_hardware_selection_falls_back_to_software_when_unavailable() {
    let manager = KeyManager::from_config(KeyManagerConfig {
        preferred_backend: KeyBackendKind::Tpm,
        enable_hardware: false,
    });
    assert_eq!(manager.backend_kind(), KeyBackendKind::Software);
    assert_eq!(manager.backend_name(), "software");
}

#[cfg(feature = "hardware-tpm")]
#[test]
fn test_tpm_backend_feature_enabled_compiles() {
    let mut backend = TpmBackend::new(true);
    assert!(backend.is_available());
    let result = backend.generate_ed25519(KeyPurpose::AuditSigning);
    assert!(matches!(result, Err(KeyError::BackendFailure(_))));
}

#[cfg(feature = "hardware-secure-enclave")]
#[test]
fn test_secure_enclave_backend_feature_enabled_compiles() {
    let mut backend = SecureEnclaveBackend::new(true);
    assert!(backend.is_available());
    let result = backend.generate_ed25519(KeyPurpose::AgentIdentity);
    assert!(matches!(result, Err(KeyError::BackendFailure(_))));
}

#[cfg(feature = "hardware-tee")]
#[test]
fn test_tee_backend_feature_enabled_compiles() {
    let mut backend = TeeBackend::new(true);
    assert!(backend.is_available());
    let result = backend.generate_ed25519(KeyPurpose::NodeIdentity);
    assert!(matches!(result, Err(KeyError::BackendFailure(_))));
}
