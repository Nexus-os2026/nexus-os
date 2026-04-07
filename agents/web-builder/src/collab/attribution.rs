//! Signed Change Attribution — Ed25519-signed edit records for governance.
//!
//! Every edit in a collaboration session is signed by the author.
//! The audit trail shows WHO made each change with cryptographic proof.

use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Errors ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum AttributionError {
    #[error("signing failed: {0}")]
    SignFailed(String),
    #[error("verification failed: {0}")]
    VerifyFailed(String),
}

// ─── Signed Edit ──────────────────────────────────────────────────────────

/// A cryptographically signed edit record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedEdit {
    pub edit_type: String,
    pub payload: String,
    pub author_public_key: String,
    pub timestamp: String,
    pub signature: String,
}

// ─── Signing ──────────────────────────────────────────────────────────────

/// Sign an edit with the user's Ed25519 identity.
pub fn sign_edit(
    edit_type: &str,
    payload: &str,
    identity: &CryptoIdentity,
) -> Result<SignedEdit, AttributionError> {
    let timestamp = crate::deploy::now_iso8601();
    let message = format!("{edit_type}|{payload}|{timestamp}");

    let sig = identity
        .sign(message.as_bytes())
        .map_err(|e| AttributionError::SignFailed(format!("{e}")))?;

    Ok(SignedEdit {
        edit_type: edit_type.to_string(),
        payload: payload.to_string(),
        author_public_key: hex::encode(identity.verifying_key()),
        timestamp,
        signature: hex::encode(&sig),
    })
}

/// Verify a signed edit against a public key.
pub fn verify_edit(edit: &SignedEdit) -> Result<bool, AttributionError> {
    let public_key = hex::decode(&edit.author_public_key)
        .map_err(|e| AttributionError::VerifyFailed(format!("bad public key: {e}")))?;
    let signature = hex::decode(&edit.signature)
        .map_err(|e| AttributionError::VerifyFailed(format!("bad signature: {e}")))?;

    let message = format!("{}|{}|{}", edit.edit_type, edit.payload, edit.timestamp);

    CryptoIdentity::verify(
        SignatureAlgorithm::Ed25519,
        &public_key,
        message.as_bytes(),
        &signature,
    )
    .map_err(|e| AttributionError::VerifyFailed(format!("{e}")))
}

/// Save signed edits log to project directory.
pub fn save_attribution_log(
    project_dir: &std::path::Path,
    edits: &[SignedEdit],
) -> Result<(), String> {
    let path = project_dir.join("collab_attribution.json");
    let json =
        serde_json::to_string_pretty(edits).map_err(|e| format!("serialize attribution: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write attribution: {e}"))
}

/// Load signed edits log from project directory.
pub fn load_attribution_log(project_dir: &std::path::Path) -> Vec<SignedEdit> {
    let path = project_dir.join("collab_attribution.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_identity() -> CryptoIdentity {
        CryptoIdentity::generate(SignatureAlgorithm::Ed25519).expect("keygen should work")
    }

    #[test]
    fn test_sign_and_verify_edit() {
        let identity = test_identity();
        let edit = sign_edit(
            "content",
            r#"{"slot":"headline","text":"Hello"}"#,
            &identity,
        )
        .expect("signing should work");

        let valid = verify_edit(&edit).expect("verify should work");
        assert!(valid, "signature should be valid");
    }

    #[test]
    fn test_tampered_edit_fails() {
        let identity = test_identity();
        let mut edit = sign_edit(
            "content",
            r#"{"slot":"headline","text":"Hello"}"#,
            &identity,
        )
        .unwrap();

        // Tamper with the payload
        edit.payload = r#"{"slot":"headline","text":"HACKED"}"#.to_string();

        let valid = verify_edit(&edit).expect("verify should not error");
        assert!(!valid, "tampered edit should fail verification");
    }

    #[test]
    fn test_wrong_key_fails() {
        let identity1 = test_identity();
        let identity2 = test_identity();

        let mut edit = sign_edit("content", "test payload", &identity1).unwrap();

        // Replace with different user's public key
        edit.author_public_key = hex::encode(identity2.verifying_key());

        let valid = verify_edit(&edit).expect("verify should not error");
        assert!(!valid, "wrong key should fail verification");
    }

    #[test]
    fn test_signed_edit_has_timestamp() {
        let identity = test_identity();
        let edit = sign_edit("token", "color-primary=#fff", &identity).unwrap();
        assert!(!edit.timestamp.is_empty(), "timestamp should be present");
        assert!(
            edit.timestamp.contains('T'),
            "timestamp should be ISO-8601 format"
        );
    }

    #[test]
    fn test_attribution_persistence() {
        let dir = std::env::temp_dir().join(format!("nexus-attr-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let identity = test_identity();
        let edit = sign_edit("content", "test", &identity).unwrap();

        save_attribution_log(&dir, &[edit.clone()]).unwrap();
        let loaded = load_attribution_log(&dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].edit_type, "content");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
