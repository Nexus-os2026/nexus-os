use super::{KeyBackendKind, KeyError, KeyManager, KeyManagerConfig, KeyPurpose, RotationApproval};
use crate::audit::AuditTrail;
use crate::hardware_security::sealed_store::SealedKeyStore;
use crate::hardware_security::software::SoftwareBackend;
use crate::hardware_security::types::{
    verify_attestation, verify_attestation_with_max_age, KeyBackend,
};
use crate::identity::{AgentIdentity, IdentityManager};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use uuid::Uuid;

use super::TeeBackend;

#[cfg(feature = "hardware-secure-enclave")]
use super::SecureEnclaveBackend;
#[cfg(feature = "hardware-tpm")]
use super::TpmBackend;

fn event_exists(audit: &AuditTrail, name: &str) -> bool {
    audit
        .events()
        .iter()
        .any(|event| event.payload.get("event").and_then(|value| value.as_str()) == Some(name))
}

// ---------------------------------------------------------------------------
// (1) test_software_backend_generate_sign_verify
// ---------------------------------------------------------------------------

#[test]
fn test_software_backend_generate_sign_verify() {
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

    // Verify key is random (not deterministic): generate a second key and compare.
    let handle2 = manager
        .generate_key(KeyPurpose::AuditSigning, &mut audit, actor)
        .expect("second key generation");
    let pk2 = manager
        .public_key_bytes(&handle2)
        .expect("second public key");
    assert_ne!(
        public_key.0, pk2.0,
        "two independently generated keys must differ"
    );
}

// ---------------------------------------------------------------------------
// (2) test_sealed_storage_roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_sealed_storage_roundtrip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let secret = b"roundtrip-test-secret";

    // Phase 1: Generate a key with sealed storage.
    let (handle, public_key_bytes) = {
        let store = SealedKeyStore::new(dir.path(), secret);
        let mut backend = SoftwareBackend::with_sealed_store(store).expect("create backend");
        let handle = backend
            .generate_ed25519(KeyPurpose::AgentIdentity)
            .expect("generate");

        // Verify sealed file exists on disk.
        let sealed_files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("sealed"))
            .collect();
        assert_eq!(sealed_files.len(), 1, "one sealed file should exist");

        let pk = backend.public_key(&handle).expect("public key");
        (handle, pk.0)
    };

    // Phase 2: "Restart" — create a fresh backend from the same sealed dir.
    let store2 = SealedKeyStore::new(dir.path(), secret);
    let backend2 = SoftwareBackend::with_sealed_store(store2).expect("reload backend");

    // Same key should be available with the same public key.
    let pk2 = backend2
        .public_key(&handle)
        .expect("public key after restart");
    assert_eq!(
        public_key_bytes, pk2.0,
        "public key should match after reload"
    );

    // Signing should still work after reload.
    let msg = b"roundtrip verification message";
    let sig = backend2.sign(&handle, msg).expect("sign after reload");
    assert_eq!(sig.0.len(), 64);

    // Verify the signature.
    let vk_bytes: [u8; 32] = pk2.0.as_slice().try_into().unwrap();
    let vk = VerifyingKey::from_bytes(&vk_bytes).unwrap();
    let sig_obj = Signature::from_bytes(&<[u8; 64]>::try_from(sig.0.as_slice()).unwrap());
    vk.verify(msg, &sig_obj).expect("signature should verify");
}

// ---------------------------------------------------------------------------
// (3) test_sealed_storage_tamper_detection
// ---------------------------------------------------------------------------

#[test]
fn test_sealed_storage_tamper_detection() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = SealedKeyStore::new(dir.path(), b"tamper-test-secret");

    store
        .seal_key("tamper-handle", b"secret-key-material-32bytes!!!!!")
        .expect("seal");

    // Find the sealed file and tamper with it.
    let sealed_path = dir.path().join("tamper-handle.sealed");
    assert!(sealed_path.exists(), "sealed file should exist");

    let mut blob = std::fs::read(&sealed_path).expect("read sealed file");
    assert!(
        blob.len() > 12,
        "sealed file should contain nonce + ciphertext"
    );

    // Flip a byte in the ciphertext (after the 12-byte nonce).
    blob[14] ^= 0xff;
    std::fs::write(&sealed_path, &blob).expect("write tampered file");

    // Attempt to unseal — should fail due to AES-GCM authentication.
    let result = store.unseal_key("tamper-handle");
    assert!(
        result.is_err(),
        "tampered sealed file should fail to unseal"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("decryption failed") || err_msg.contains("corrupted"),
        "error should indicate decryption/corruption failure: {err_msg}"
    );
}

// ---------------------------------------------------------------------------
// (4) test_tee_backend_falls_back_to_software
// ---------------------------------------------------------------------------

#[test]
fn test_tee_backend_falls_back_to_software() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Create TeeBackend without SGX/Nitro (configured=false → skip hardware probe).
    let mut backend = TeeBackend::new(false, Some(dir.path().to_path_buf()));

    // Should fall back to software-tee provider.
    assert!(backend.is_available());
    assert_eq!(backend.backend_name(), "tee-software");

    // Full end-to-end: generate, sign, verify.
    let handle = backend
        .generate_ed25519(KeyPurpose::AgentIdentity)
        .expect("generate via tee fallback");
    let pub_key = backend.public_key(&handle).expect("public key");
    assert_eq!(pub_key.0.len(), 32);

    let msg = b"tee fallback test message";
    let sig = backend.sign(&handle, msg).expect("sign");
    assert_eq!(sig.0.len(), 64);

    let vk_bytes: [u8; 32] = pub_key.0.as_slice().try_into().unwrap();
    let vk = VerifyingKey::from_bytes(&vk_bytes).unwrap();
    let sig_obj = Signature::from_bytes(&<[u8; 64]>::try_from(sig.0.as_slice()).unwrap());
    vk.verify(msg, &sig_obj)
        .expect("signature should verify with software-tee fallback");
}

// ---------------------------------------------------------------------------
// (5) test_key_handle_opaque
// ---------------------------------------------------------------------------

#[test]
fn test_key_handle_opaque() {
    let mut manager = KeyManager::new();
    let mut audit = AuditTrail::new();
    let actor = Uuid::nil();

    let handle = manager
        .generate_key(KeyPurpose::AgentIdentity, &mut audit, actor)
        .expect("generate");
    let pub_key = manager.public_key_bytes(&handle).expect("public key");

    // The handle ID should NOT contain the raw key material.
    // Public key is 32 bytes = 64 hex chars. The handle should not embed the full key.
    let pub_hex: String = pub_key.0.iter().map(|b| format!("{b:02x}")).collect();
    assert!(
        !handle.id.contains(&pub_hex),
        "handle ID must not contain the full public key hex"
    );

    // Handle format is "sw-{purpose}-{seq}-{short_hash}" — verify it's structured.
    assert!(
        handle.id.starts_with("sw-"),
        "software backend handle should start with 'sw-'"
    );
    assert!(
        handle.id.contains("agent_identity"),
        "handle should reference the purpose"
    );

    // The handle alone should not allow extracting the private key.
    // We verify this by checking that KeyHandle has no secret key field —
    // it only contains an id (String) and purpose (KeyPurpose).
    let handle_json = serde_json::to_string(&handle).expect("serialize handle");
    assert!(
        !handle_json.contains("signing_key"),
        "handle serialization must not contain signing key"
    );
    assert!(
        !handle_json.contains("secret"),
        "handle serialization must not contain secret"
    );
    assert!(
        !handle_json.contains("seed"),
        "handle serialization must not contain seed"
    );
}

// ---------------------------------------------------------------------------
// (6) test_agent_identity_uses_key_manager
// ---------------------------------------------------------------------------

#[test]
fn test_agent_identity_uses_key_manager() {
    let mut km = KeyManager::new();
    let agent_id = Uuid::new_v4();
    let identity = AgentIdentity::generate(agent_id, &mut km).expect("generate identity");

    // Sign a message via AgentIdentity → KeyManager delegation.
    let payload = b"governed agent action";
    let sig = identity.sign(payload, &km).expect("sign via identity");
    assert_eq!(sig.len(), 64);

    // Verify using the identity's verify method.
    identity
        .verify(payload, &sig)
        .expect("signature should verify");

    // Cross-verify with raw ed25519-dalek.
    let vk = VerifyingKey::from_bytes(&identity.public_key_bytes()).unwrap();
    let sig_obj = ed25519_dalek::Signature::from_slice(&sig).unwrap();
    vk.verify(payload, &sig_obj)
        .expect("raw ed25519 verify should pass");

    // AgentIdentity struct should NOT contain any secret key field.
    // It only has: agent_id, key_handle_id, public_key, did, created_at.
    let debug_repr = format!("{identity:?}");
    assert!(
        !debug_repr.contains("SigningKey"),
        "AgentIdentity debug output must not contain SigningKey"
    );
    assert!(
        !debug_repr.contains("secret"),
        "AgentIdentity debug output must not contain 'secret'"
    );
}

// ---------------------------------------------------------------------------
// (7) test_agent_identity_persistence_no_plaintext
// ---------------------------------------------------------------------------

#[test]
fn test_agent_identity_persistence_no_plaintext() {
    let dir = tempfile::tempdir().expect("tempdir");
    let agent_id = Uuid::new_v4();

    // Create and persist an identity.
    {
        let mut mgr = IdentityManager::new(dir.path());
        mgr.get_or_create(agent_id).expect("create identity");
    }

    // Read the persisted JSON file directly.
    let json_path = dir.path().join(format!("{agent_id}.json"));
    assert!(json_path.exists(), "identity JSON file should exist");

    let json_content = std::fs::read_to_string(&json_path).expect("read identity JSON");

    // Must contain key_handle_id (new format).
    assert!(
        json_content.contains("key_handle_id"),
        "persisted JSON should contain key_handle_id"
    );

    // Must NOT contain secret_key_hex (old format).
    assert!(
        !json_content.contains("secret_key_hex"),
        "persisted JSON must NOT contain secret_key_hex"
    );

    // Must NOT contain raw signing key bytes.
    assert!(
        !json_content.contains("signing_key"),
        "persisted JSON must NOT contain signing_key"
    );

    // Verify the JSON parses as expected structure.
    let parsed: serde_json::Value =
        serde_json::from_str(&json_content).expect("parse identity JSON");
    assert!(parsed.get("agent_id").is_some());
    assert!(parsed.get("key_handle_id").is_some());
    assert!(parsed.get("public_key_hex").is_some());
    assert!(parsed.get("did").is_some());
    assert!(parsed.get("created_at").is_some());

    // public_key_hex should be 64 hex chars (32 bytes).
    let pk_hex = parsed["public_key_hex"].as_str().unwrap();
    assert_eq!(pk_hex.len(), 64, "public key hex should be 64 chars");
}

// ---------------------------------------------------------------------------
// (8) test_attestation_report_valid
// ---------------------------------------------------------------------------

#[test]
fn test_attestation_report_valid() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut backend = TeeBackend::new(false, Some(dir.path().to_path_buf()));

    // Generate a key so attestation can be signed.
    backend
        .generate_ed25519(KeyPurpose::AgentIdentity)
        .expect("generate key for attestation");

    let report = backend.attest("freshness-nonce-42").expect("attestation");

    // Check all fields are populated.
    assert_eq!(report.backend, "software-tee");
    assert!(report.available);
    assert_eq!(report.protocol_version, 1);
    assert_eq!(report.provider, "software");
    assert_eq!(report.nonce, "freshness-nonce-42");
    assert!(!report.public_key_hash.is_empty());
    assert!(
        report.public_key_hash.len() == 64,
        "SHA-256 hex should be 64 chars"
    );
    assert!(!report.platform_info.is_empty());
    assert!(report.platform_info.contains("nexus-os-v7"));
    assert!(report.timestamp > 0);
    assert_eq!(report.signature.len(), 96, "signature = 32 pubkey + 64 sig");
    assert_ne!(report.device_claims_hash, [0u8; 32]);

    // Verify the attestation report.
    let valid = verify_attestation(&report).expect("verify should not error");
    assert!(valid, "signed attestation report should verify");
}

// ---------------------------------------------------------------------------
// (9) test_attestation_report_expired
// ---------------------------------------------------------------------------

#[test]
fn test_attestation_report_expired() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut backend = TeeBackend::new(false, Some(dir.path().to_path_buf()));

    backend
        .generate_ed25519(KeyPurpose::AgentIdentity)
        .expect("generate");

    let mut report = backend.attest("expiry-test").expect("attestation");

    // Manually set the timestamp to 48 hours ago.
    report.timestamp = report.timestamp.saturating_sub(48 * 3600);

    // Default verify (24h window) should reject it.
    let valid = verify_attestation(&report).expect("verify should not error");
    assert!(!valid, "expired attestation (48h old) should be rejected");

    // With a large enough max_age it should still pass (signature is valid).
    // But we tampered with the timestamp AFTER signing, so the signature
    // won't match the modified payload. Let's verify that too.
    // Actually: the signature was computed over the original payload (with
    // original timestamp). Since we changed the timestamp, the payload
    // reconstructed during verification will differ → signature mismatch.
    let result = verify_attestation_with_max_age(&report, 100 * 3600);
    let valid = result.expect("verify should not error");
    assert!(
        !valid,
        "attestation with tampered timestamp should fail signature check"
    );

    // Test with a report whose timestamp is far in the future beyond tolerance.
    let mut future_report = backend.attest("future-test").expect("future attestation");
    future_report.timestamp += 120; // 2 minutes in the future, beyond 60s tolerance
    let valid = verify_attestation(&future_report).expect("verify");
    assert!(
        !valid,
        "attestation with far-future timestamp should be rejected"
    );
}

// ---------------------------------------------------------------------------
// (10) test_key_rotation_seals_new_key
// ---------------------------------------------------------------------------

#[test]
fn test_key_rotation_seals_new_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = SealedKeyStore::new(dir.path(), b"rotation-seal-test");
    let mut backend = SoftwareBackend::with_sealed_store(store).expect("create backend");

    let original = backend
        .generate_ed25519(KeyPurpose::AgentIdentity)
        .expect("generate original");

    // One sealed file should exist.
    let count_sealed = || -> usize {
        std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("sealed"))
            .count()
    };
    assert_eq!(count_sealed(), 1);

    let rotated = backend.rotate(&original).expect("rotate");
    assert_ne!(original.id, rotated.id);
    assert_eq!(original.purpose, rotated.purpose);

    // Two sealed files should exist (deprecated original + new).
    assert_eq!(
        count_sealed(),
        2,
        "both original (deprecated) and rotated keys should be sealed"
    );

    // Original key should still be accessible (deprecated but not deleted).
    let orig_pk = backend
        .public_key(&original)
        .expect("original still accessible");
    let new_pk = backend.public_key(&rotated).expect("rotated accessible");
    assert_ne!(
        orig_pk.0, new_pk.0,
        "rotated key must have different public key"
    );

    // New key should be usable for signing.
    let sig = backend
        .sign(&rotated, b"post-rotation message")
        .expect("sign with rotated key");
    assert_eq!(sig.0.len(), 64);
}

// ---------------------------------------------------------------------------
// (11) test_backward_compat_import_plaintext
// ---------------------------------------------------------------------------

#[test]
fn test_backward_compat_import_plaintext() {
    let dir = tempfile::tempdir().expect("tempdir");
    let agent_id = Uuid::new_v4();

    // Create a legacy-format identity JSON file with secret_key_hex.
    let seed = [42u8; 32];
    let legacy_hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    let legacy_json = serde_json::json!({
        "agent_id": agent_id,
        "secret_key_hex": legacy_hex,
        "did": "did:key:z6MkLegacyPlaceholder",
        "created_at": 1700000000u64,
    });

    let json_path = dir.path().join(format!("{agent_id}.json"));
    std::fs::write(&json_path, legacy_json.to_string()).expect("write legacy JSON");

    // Load with IdentityManager.
    let mut mgr = IdentityManager::new(dir.path());
    let loaded = mgr.load_all().expect("load identities");
    assert_eq!(loaded, 1);

    let identity = mgr.get(&agent_id).expect("identity should exist");
    assert_eq!(identity.agent_id, agent_id);
    assert!(identity.did.starts_with("did:key:z6Mk"));
    assert_eq!(identity.created_at, 1700000000);
    assert!(!identity.key_handle_id().is_empty());

    // Verify the persisted file was updated to new format.
    let updated_json = std::fs::read_to_string(&json_path).expect("read updated JSON");
    assert!(
        updated_json.contains("key_handle_id"),
        "updated file should have key_handle_id"
    );
    assert!(
        !updated_json.contains("secret_key_hex"),
        "updated file should NOT have secret_key_hex"
    );

    // The imported identity should be able to sign (using the new key in KeyManager).
    let sig = identity
        .sign(b"after import", mgr.key_manager())
        .expect("sign after legacy import");
    assert_eq!(sig.len(), 64);
    identity
        .verify(b"after import", &sig)
        .expect("verify after import");
}

// ---------------------------------------------------------------------------
// (12) test_multiple_agents_independent_keys
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_agents_independent_keys() {
    let mut km = KeyManager::new();
    let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

    let identities: Vec<AgentIdentity> = ids
        .iter()
        .map(|&id| AgentIdentity::generate(id, &mut km).expect("generate identity"))
        .collect();

    // All three should have unique key handles, DIDs, and public keys.
    let handles: Vec<&str> = identities.iter().map(|i| i.key_handle_id()).collect();
    let dids: Vec<&str> = identities.iter().map(|i| i.did.as_str()).collect();
    let pks: Vec<[u8; 32]> = identities.iter().map(|i| i.public_key_bytes()).collect();

    for i in 0..3 {
        for j in (i + 1)..3 {
            assert_ne!(handles[i], handles[j], "key handles must be unique");
            assert_ne!(dids[i], dids[j], "DIDs must be unique");
            assert_ne!(pks[i], pks[j], "public keys must be unique");
        }
    }

    // Each identity should sign independently and only its own verification succeeds.
    let message = b"independent signing test";
    for (idx, identity) in identities.iter().enumerate() {
        let sig = identity.sign(message, &km).expect("sign");
        assert_eq!(sig.len(), 64);

        // Own verification succeeds.
        identity.verify(message, &sig).expect("own verify");

        // Other identities' verification fails.
        for (other_idx, other) in identities.iter().enumerate() {
            if other_idx != idx {
                let result = other.verify(message, &sig);
                assert!(
                    result.is_err(),
                    "agent {other_idx}'s verify should reject agent {idx}'s signature"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Existing tests preserved below
// ---------------------------------------------------------------------------

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
fn test_attestation_and_audited() {
    let manager = KeyManager::new();
    let mut audit = AuditTrail::new();
    let actor = Uuid::nil();

    let report = manager
        .generate_attestation("test-nonce-123", &mut audit, actor)
        .expect("attestation should succeed");

    assert_eq!(report.nonce, "test-nonce-123");
    assert_eq!(report.provider, "software");
    assert!(!report.platform_info.is_empty());
    assert!(report.timestamp > 0);
    assert!(event_exists(&audit, "attestation.generated"));
}

#[test]
fn test_hardware_selection_falls_back_to_software_when_unavailable() {
    let manager = KeyManager::from_config(KeyManagerConfig {
        preferred_backend: KeyBackendKind::Tpm,
        enable_hardware: false,
        sealed_store_dir: None,
    });
    assert_eq!(manager.backend_kind(), KeyBackendKind::Software);
    assert_eq!(manager.backend_name(), "software");
}

#[test]
fn test_sealed_keys_survive_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let secret = b"restart-test-secret";

    let (handle, public_key_bytes) = {
        let store = SealedKeyStore::new(dir.path(), secret);
        let mut backend = SoftwareBackend::with_sealed_store(store).expect("create backend");
        let handle = backend
            .generate_ed25519(KeyPurpose::NodeIdentity)
            .expect("generate");
        let pk = backend.public_key(&handle).expect("public key");
        (handle, pk.0)
    };

    let store2 = SealedKeyStore::new(dir.path(), secret);
    let backend2 = SoftwareBackend::with_sealed_store(store2).expect("reload backend");

    let pk2 = backend2
        .public_key(&handle)
        .expect("public key after restart");
    assert_eq!(
        public_key_bytes, pk2.0,
        "public key should match after reload"
    );

    let sig = backend2
        .sign(&handle, b"survived restart")
        .expect("sign after reload");
    assert_eq!(sig.0.len(), 64);
}

#[test]
fn test_key_manager_with_sealed_store_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut manager = KeyManager::from_config(KeyManagerConfig {
        preferred_backend: KeyBackendKind::Software,
        enable_hardware: false,
        sealed_store_dir: Some(dir.path().to_path_buf()),
    });
    let mut audit = AuditTrail::new();
    let actor = Uuid::nil();

    let handle = manager
        .generate_key(KeyPurpose::AuditSigning, &mut audit, actor)
        .expect("generate with sealed config");

    let sig = manager
        .sign_with_key(&handle, b"sealed manager test")
        .expect("sign");
    assert_eq!(sig.0.len(), 64);

    let sealed_exists = std::fs::read_dir(dir.path()).unwrap().any(|e| {
        e.ok()
            .and_then(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str().map(|s| s == "sealed"))
            })
            .unwrap_or(false)
    });
    assert!(sealed_exists, "sealed key file should exist on disk");
}

// ---------------------------------------------------------------------------
// Feature-gated stub tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// TEE backend tests (always available via software-tee fallback)
// ---------------------------------------------------------------------------

#[test]
fn test_tee_backend_generates_and_signs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut backend = TeeBackend::new(false, Some(dir.path().to_path_buf()));

    assert!(backend.is_available());
    assert_eq!(backend.backend_name(), "tee-software");

    let handle = backend
        .generate_ed25519(KeyPurpose::NodeIdentity)
        .expect("generate via tee backend");
    let pub_key = backend.public_key(&handle).expect("public key");
    assert_eq!(pub_key.0.len(), 32);

    let sig = backend
        .sign(&handle, b"tee integration test")
        .expect("sign");
    assert_eq!(sig.0.len(), 64);

    let vk_bytes: [u8; 32] = pub_key.0.as_slice().try_into().unwrap();
    let vk = VerifyingKey::from_bytes(&vk_bytes).unwrap();
    let sig_obj = Signature::from_bytes(&<[u8; 64]>::try_from(sig.0.as_slice()).unwrap());
    vk.verify(b"tee integration test", &sig_obj)
        .expect("signature should verify");
}

#[test]
fn test_tee_backend_via_key_manager() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut manager = KeyManager::from_config(KeyManagerConfig {
        preferred_backend: KeyBackendKind::Tee,
        enable_hardware: false,
        sealed_store_dir: Some(dir.path().to_path_buf()),
    });
    let mut audit = AuditTrail::new();
    let actor = Uuid::nil();

    assert_eq!(manager.backend_kind(), KeyBackendKind::Tee);
    assert_eq!(manager.backend_name(), "tee-software");

    let handle = manager
        .generate_key(KeyPurpose::AgentIdentity, &mut audit, actor)
        .expect("generate via tee manager");
    let sig = manager
        .sign_with_key(&handle, b"tee manager test")
        .expect("sign");
    assert_eq!(sig.0.len(), 64);
    assert!(event_exists(&audit, "keys.generated"));
}
