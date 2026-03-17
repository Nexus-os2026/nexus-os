//! Verifiable Credentials for Nexus OS agents.
//!
//! A [`CredentialIssuer`] generates [`VerifiableCredential`]s that attest to
//! agent properties (autonomy clearance, task completion, etc.). Credentials
//! are signed using a SHA-256 HMAC-like scheme keyed to the issuer's DID,
//! providing tamper-evidence without requiring direct access to private keys.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors produced by credential operations.
#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("credential not found: {0}")]
    NotFound(Uuid),

    #[error("credential has been revoked: {0}")]
    Revoked(Uuid),

    #[error("credential has expired")]
    Expired,

    #[error("signature verification failed")]
    VerificationFailed,

    #[error("issuer DID mismatch")]
    IssuerMismatch,

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The kind of claim a credential attests to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CredentialType {
    /// Agent is cleared for the specified autonomy level.
    AutonomyClearance,
    /// Agent successfully completed a task.
    TaskCompletion,
    /// Agent passed adversarial / red-team testing.
    AdversarialTesting,
    /// Agent was created through the Genesis system.
    GenesisCreated,
    /// Agent holds a security clearance tier.
    SecurityClearance,
}

/// A signed, verifiable credential issued by one DID about another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiableCredential {
    /// Unique credential identifier.
    pub id: Uuid,
    /// DID of the agent this credential describes.
    pub subject_did: String,
    /// DID of the issuing authority.
    pub issuer_did: String,
    /// What the credential attests to.
    pub credential_type: CredentialType,
    /// Arbitrary structured claims payload.
    pub claims: serde_json::Value,
    /// Unix-epoch seconds when the credential was issued.
    pub issued_at: u64,
    /// Optional expiration (Unix-epoch seconds).
    pub expires_at: Option<u64>,
    /// Hex-encoded SHA-256 signature over the canonical credential body.
    pub signature: String,
}

// ---------------------------------------------------------------------------
// CredentialIssuer
// ---------------------------------------------------------------------------

/// Issues, verifies, and revokes [`VerifiableCredential`]s.
///
/// Signing uses a deterministic SHA-256 hash over the canonical JSON of the
/// credential body concatenated with the issuer DID (acting as a shared
/// secret). This provides tamper-evidence; a future version will delegate to
/// the [`KeyManager`] for real Ed25519 signatures.
#[derive(Debug, Clone)]
pub struct CredentialIssuer {
    /// DID of this issuer.
    issuer_did: String,
    /// Set of revoked credential IDs.
    revoked: HashSet<Uuid>,
}

impl CredentialIssuer {
    /// Create a new issuer bound to a DID.
    pub fn new(issuer_did: String) -> Self {
        Self {
            issuer_did,
            revoked: HashSet::new(),
        }
    }

    /// Return the issuer's DID.
    pub fn did(&self) -> &str {
        &self.issuer_did
    }

    /// Issue a new credential for `subject_did`.
    pub fn issue_credential(
        &self,
        subject_did: String,
        credential_type: CredentialType,
        claims: serde_json::Value,
        ttl_secs: Option<u64>,
    ) -> VerifiableCredential {
        let now = now_secs();
        let expires_at = ttl_secs.map(|ttl| now + ttl);

        let id = Uuid::new_v4();
        let signature = self.compute_signature(
            &id,
            &subject_did,
            &credential_type,
            &claims,
            now,
            expires_at,
        );

        VerifiableCredential {
            id,
            subject_did,
            issuer_did: self.issuer_did.clone(),
            credential_type,
            claims,
            issued_at: now,
            expires_at,
            signature,
        }
    }

    /// Verify a credential's integrity and revocation status.
    pub fn verify_credential(&self, cred: &VerifiableCredential) -> Result<(), CredentialError> {
        // Check issuer matches.
        if cred.issuer_did != self.issuer_did {
            return Err(CredentialError::IssuerMismatch);
        }

        // Check revocation.
        if self.revoked.contains(&cred.id) {
            return Err(CredentialError::Revoked(cred.id));
        }

        // Check expiration.
        if let Some(exp) = cred.expires_at {
            if now_secs() > exp {
                return Err(CredentialError::Expired);
            }
        }

        // Verify signature.
        let expected = self.compute_signature(
            &cred.id,
            &cred.subject_did,
            &cred.credential_type,
            &cred.claims,
            cred.issued_at,
            cred.expires_at,
        );

        if expected != cred.signature {
            return Err(CredentialError::VerificationFailed);
        }

        Ok(())
    }

    /// Revoke a credential by ID. Returns `true` if it was not already revoked.
    pub fn revoke_credential(&mut self, credential_id: Uuid) -> bool {
        self.revoked.insert(credential_id)
    }

    /// Check whether a credential ID has been revoked.
    pub fn is_revoked(&self, credential_id: &Uuid) -> bool {
        self.revoked.contains(credential_id)
    }

    // -- internal -------------------------------------------------------------

    fn compute_signature(
        &self,
        id: &Uuid,
        subject_did: &str,
        credential_type: &CredentialType,
        claims: &serde_json::Value,
        issued_at: u64,
        expires_at: Option<u64>,
    ) -> String {
        let canonical = format!(
            "{}|{}|{}|{:?}|{}|{}|{:?}",
            id,
            subject_did,
            self.issuer_did,
            credential_type,
            serde_json::to_string(claims).unwrap_or_default(),
            issued_at,
            expires_at,
        );

        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        // Mix in the issuer DID as a keying material.
        hasher.update(self.issuer_did.as_bytes());
        hex_encode(&hasher.finalize())
    }
}

// ---------------------------------------------------------------------------
// Standalone verification (without issuer instance)
// ---------------------------------------------------------------------------

/// Verify a credential's signature given only the issuer DID.
///
/// This does **not** check revocation status (that requires a
/// [`CredentialIssuer`] with its revocation set).
pub fn verify_credential_standalone(cred: &VerifiableCredential) -> Result<(), CredentialError> {
    // Check expiration.
    if let Some(exp) = cred.expires_at {
        if now_secs() > exp {
            return Err(CredentialError::Expired);
        }
    }

    let canonical = format!(
        "{}|{}|{}|{:?}|{}|{}|{:?}",
        cred.id,
        cred.subject_did,
        cred.issuer_did,
        cred.credential_type,
        serde_json::to_string(&cred.claims).unwrap_or_default(),
        cred.issued_at,
        cred.expires_at,
    );

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hasher.update(cred.issuer_did.as_bytes());
    let expected = hex_encode(&hasher.finalize());

    if expected != cred.signature {
        return Err(CredentialError::VerificationFailed);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_issuer() -> CredentialIssuer {
        CredentialIssuer::new("did:key:z6MkTestIssuer".to_string())
    }

    #[test]
    fn issue_and_verify_credential() {
        let issuer = test_issuer();
        let cred = issuer.issue_credential(
            "did:key:z6MkSubject".to_string(),
            CredentialType::AutonomyClearance,
            serde_json::json!({"level": 3}),
            None,
        );

        assert_eq!(cred.issuer_did, "did:key:z6MkTestIssuer");
        assert_eq!(cred.subject_did, "did:key:z6MkSubject");
        assert!(cred.signature.len() == 64); // SHA-256 hex = 64 chars
        issuer.verify_credential(&cred).expect("valid credential");
    }

    #[test]
    fn tampered_credential_rejected() {
        let issuer = test_issuer();
        let mut cred = issuer.issue_credential(
            "did:key:z6MkSubject".to_string(),
            CredentialType::TaskCompletion,
            serde_json::json!({"task": "test"}),
            None,
        );

        // Tamper with claims.
        cred.claims = serde_json::json!({"task": "hacked"});
        let result = issuer.verify_credential(&cred);
        assert!(matches!(result, Err(CredentialError::VerificationFailed)));
    }

    #[test]
    fn revoked_credential_rejected() {
        let mut issuer = test_issuer();
        let cred = issuer.issue_credential(
            "did:key:z6MkSubject".to_string(),
            CredentialType::SecurityClearance,
            serde_json::json!({"tier": 2}),
            None,
        );

        assert!(issuer.revoke_credential(cred.id));
        assert!(issuer.is_revoked(&cred.id));

        let result = issuer.verify_credential(&cred);
        assert!(matches!(result, Err(CredentialError::Revoked(_))));
    }

    #[test]
    fn expired_credential_rejected() {
        let issuer = test_issuer();
        let mut cred = issuer.issue_credential(
            "did:key:z6MkSubject".to_string(),
            CredentialType::AdversarialTesting,
            serde_json::json!({}),
            Some(3600),
        );

        // Force expiration in the past.
        cred.expires_at = Some(1);
        let result = issuer.verify_credential(&cred);
        assert!(matches!(result, Err(CredentialError::Expired)));
    }

    #[test]
    fn issuer_mismatch_rejected() {
        let issuer = test_issuer();
        let other_issuer = CredentialIssuer::new("did:key:z6MkOther".to_string());

        let cred = other_issuer.issue_credential(
            "did:key:z6MkSubject".to_string(),
            CredentialType::GenesisCreated,
            serde_json::json!({}),
            None,
        );

        let result = issuer.verify_credential(&cred);
        assert!(matches!(result, Err(CredentialError::IssuerMismatch)));
    }

    #[test]
    fn standalone_verification() {
        let issuer = test_issuer();
        let cred = issuer.issue_credential(
            "did:key:z6MkSubject".to_string(),
            CredentialType::AutonomyClearance,
            serde_json::json!({"level": 5}),
            None,
        );

        verify_credential_standalone(&cred).expect("standalone verify ok");
    }

    #[test]
    fn credential_types_serialize() {
        let types = vec![
            CredentialType::AutonomyClearance,
            CredentialType::TaskCompletion,
            CredentialType::AdversarialTesting,
            CredentialType::GenesisCreated,
            CredentialType::SecurityClearance,
        ];

        for ct in types {
            let json = serde_json::to_string(&ct).expect("serialize");
            let round: CredentialType = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(ct, round);
        }
    }

    #[test]
    fn credential_roundtrip_serde() {
        let issuer = test_issuer();
        let cred = issuer.issue_credential(
            "did:key:z6MkSubject".to_string(),
            CredentialType::TaskCompletion,
            serde_json::json!({"score": 0.95}),
            Some(7200),
        );

        let json = serde_json::to_string(&cred).expect("serialize");
        let restored: VerifiableCredential = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.id, cred.id);
        assert_eq!(restored.signature, cred.signature);
        issuer.verify_credential(&restored).expect("still valid");
    }
}
