//! Agent Passport — an exportable identity bundle for marketplace exchange.
//!
//! An [`AgentPassport`] aggregates an agent's DID, verifiable credentials,
//! genome hash, lineage, and test scores into a single signed document that
//! can be serialized, shared, and independently verified.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::credentials::{verify_credential_standalone, CredentialError, VerifiableCredential};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors produced by passport operations.
#[derive(Debug, thiserror::Error)]
pub enum PassportError {
    #[error("passport signature verification failed")]
    SignatureInvalid,

    #[error("credential verification failed: {0}")]
    CredentialInvalid(#[from] CredentialError),

    #[error("passport has no DID")]
    MissingDid,

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A scored test result embedded in a passport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestScore {
    /// Human-readable test name.
    pub test_name: String,
    /// Score in the range 0.0–1.0.
    pub score: f64,
    /// Whether the score was independently verified.
    pub verified: bool,
}

/// A portable, signed identity document for an agent.
///
/// Contains everything a marketplace or peer node needs to evaluate an agent's
/// trustworthiness: identity, credentials, lineage, and test results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPassport {
    /// The agent's UUID.
    pub agent_id: Uuid,
    /// The agent's `did:key:…` DID string.
    pub did: String,
    /// Verifiable credentials issued to this agent.
    pub credentials: Vec<VerifiableCredential>,
    /// Hex-encoded SHA-256 hash of the agent's genome/manifest.
    pub genome_hash: String,
    /// Unix-epoch seconds when this passport was created.
    pub creation_date: u64,
    /// Ordered list of ancestor agent DIDs (oldest first).
    pub lineage: Vec<String>,
    /// Test/benchmark scores.
    pub test_scores: Vec<TestScore>,
    /// Hex-encoded SHA-256 signature over the passport body.
    pub signature: String,
}

// ---------------------------------------------------------------------------
// Construction & verification
// ---------------------------------------------------------------------------

/// Export (create) a signed passport for an agent.
///
/// The `signing_did` is used as keying material for the signature. In a full
/// implementation, this would delegate to [`KeyManager`] for an Ed25519
/// signature; here we use a SHA-256 commitment keyed to the DID.
pub fn export_passport(
    agent_id: Uuid,
    did: String,
    credentials: Vec<VerifiableCredential>,
    genome_hash: String,
    lineage: Vec<String>,
    test_scores: Vec<TestScore>,
) -> AgentPassport {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut passport = AgentPassport {
        agent_id,
        did,
        credentials,
        genome_hash,
        creation_date: now,
        lineage,
        test_scores,
        signature: String::new(),
    };

    passport.signature = compute_passport_signature(&passport);
    passport
}

/// Import a passport from JSON, returning the deserialized struct.
///
/// Does **not** verify the passport automatically; call [`verify_passport`]
/// after import to check integrity.
pub fn import_passport(json: &str) -> Result<AgentPassport, PassportError> {
    let passport: AgentPassport = serde_json::from_str(json)?;
    Ok(passport)
}

/// Verify a passport's integrity.
///
/// Checks:
/// 1. The passport's own signature is valid.
/// 2. Each embedded credential passes standalone verification.
pub fn verify_passport(passport: &AgentPassport) -> Result<(), PassportError> {
    // Check passport-level signature.
    let expected = compute_passport_signature(passport);
    if expected != passport.signature {
        return Err(PassportError::SignatureInvalid);
    }

    // Check each credential.
    for cred in &passport.credentials {
        verify_credential_standalone(cred)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal
// ---------------------------------------------------------------------------

/// Compute the SHA-256 signature over the canonical passport body.
///
/// The signature covers all fields except `signature` itself.
fn compute_passport_signature(passport: &AgentPassport) -> String {
    let mut h = Sha256::new();

    // Agent identity.
    h.update(passport.agent_id.as_bytes());
    h.update(passport.did.as_bytes());

    // Genome.
    h.update(passport.genome_hash.as_bytes());

    // Creation date.
    h.update(passport.creation_date.to_le_bytes());

    // Lineage (order matters).
    for ancestor in &passport.lineage {
        h.update(ancestor.as_bytes());
    }

    // Test scores.
    for ts in &passport.test_scores {
        h.update(ts.test_name.as_bytes());
        h.update(ts.score.to_le_bytes());
        h.update([u8::from(ts.verified)]);
    }

    // Credential signatures (chain of trust).
    for cred in &passport.credentials {
        h.update(cred.signature.as_bytes());
    }

    // Key the hash with the DID.
    h.update(passport.did.as_bytes());

    hex_encode(&h.finalize())
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
    use crate::identity::credentials::{CredentialIssuer, CredentialType};

    fn sample_credentials() -> Vec<VerifiableCredential> {
        let issuer = CredentialIssuer::new("did:key:z6MkIssuer".to_string());
        vec![
            issuer.issue_credential(
                "did:key:z6MkAgent".to_string(),
                CredentialType::AutonomyClearance,
                serde_json::json!({"level": 3}),
                None,
            ),
            issuer.issue_credential(
                "did:key:z6MkAgent".to_string(),
                CredentialType::TaskCompletion,
                serde_json::json!({"tasks": 42}),
                None,
            ),
        ]
    }

    fn sample_test_scores() -> Vec<TestScore> {
        vec![
            TestScore {
                test_name: "adversarial_robustness".to_string(),
                score: 0.92,
                verified: true,
            },
            TestScore {
                test_name: "task_accuracy".to_string(),
                score: 0.87,
                verified: false,
            },
        ]
    }

    #[test]
    fn export_and_verify_passport() {
        let passport = export_passport(
            Uuid::new_v4(),
            "did:key:z6MkAgent".to_string(),
            sample_credentials(),
            "abcdef1234567890".to_string(),
            vec!["did:key:z6MkParent".to_string()],
            sample_test_scores(),
        );

        assert!(!passport.signature.is_empty());
        assert_eq!(passport.signature.len(), 64);
        assert!(passport.creation_date > 0);

        verify_passport(&passport).expect("passport valid");
    }

    #[test]
    fn tampered_passport_rejected() {
        let mut passport = export_passport(
            Uuid::new_v4(),
            "did:key:z6MkAgent".to_string(),
            sample_credentials(),
            "abcdef1234567890".to_string(),
            vec![],
            sample_test_scores(),
        );

        // Tamper with genome hash.
        passport.genome_hash = "tampered_hash".to_string();
        let result = verify_passport(&passport);
        assert!(matches!(result, Err(PassportError::SignatureInvalid)));
    }

    #[test]
    fn tampered_lineage_rejected() {
        let mut passport = export_passport(
            Uuid::new_v4(),
            "did:key:z6MkAgent".to_string(),
            vec![],
            "genomehash".to_string(),
            vec!["did:key:z6MkParent".to_string()],
            vec![],
        );

        passport.lineage.push("did:key:z6MkFake".to_string());
        let result = verify_passport(&passport);
        assert!(matches!(result, Err(PassportError::SignatureInvalid)));
    }

    #[test]
    fn import_export_roundtrip() {
        let passport = export_passport(
            Uuid::new_v4(),
            "did:key:z6MkAgent".to_string(),
            sample_credentials(),
            "hash123".to_string(),
            vec![],
            sample_test_scores(),
        );

        let json = serde_json::to_string_pretty(&passport).expect("serialize");
        let imported = import_passport(&json).expect("import");

        assert_eq!(imported.agent_id, passport.agent_id);
        assert_eq!(imported.did, passport.did);
        assert_eq!(imported.signature, passport.signature);
        assert_eq!(imported.credentials.len(), 2);
        assert_eq!(imported.test_scores.len(), 2);

        verify_passport(&imported).expect("imported passport valid");
    }

    #[test]
    fn empty_passport_valid() {
        let passport = export_passport(
            Uuid::new_v4(),
            "did:key:z6MkEmpty".to_string(),
            vec![],
            "empty".to_string(),
            vec![],
            vec![],
        );

        verify_passport(&passport).expect("empty passport valid");
    }

    #[test]
    fn test_score_serde() {
        let score = TestScore {
            test_name: "benchmark".to_string(),
            score: 0.95,
            verified: true,
        };

        let json = serde_json::to_string(&score).expect("serialize");
        let restored: TestScore = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.test_name, "benchmark");
        assert!((restored.score - 0.95).abs() < f64::EPSILON);
        assert!(restored.verified);
    }

    #[test]
    fn tampered_credential_in_passport_rejected() {
        let issuer = CredentialIssuer::new("did:key:z6MkIssuer".to_string());
        let mut cred = issuer.issue_credential(
            "did:key:z6MkAgent".to_string(),
            CredentialType::SecurityClearance,
            serde_json::json!({"tier": 1}),
            None,
        );

        // Tamper with the credential claims.
        cred.claims = serde_json::json!({"tier": 9999});

        let passport = export_passport(
            Uuid::new_v4(),
            "did:key:z6MkAgent".to_string(),
            vec![cred],
            "hash".to_string(),
            vec![],
            vec![],
        );

        // The passport signature is valid (it was computed over the tampered
        // credential), but the credential's own signature fails.
        let result = verify_passport(&passport);
        assert!(matches!(result, Err(PassportError::CredentialInvalid(_))));
    }

    #[test]
    fn passport_with_lineage() {
        let lineage = vec![
            "did:key:z6MkGrandparent".to_string(),
            "did:key:z6MkParent".to_string(),
        ];

        let passport = export_passport(
            Uuid::new_v4(),
            "did:key:z6MkChild".to_string(),
            vec![],
            "genome_v3".to_string(),
            lineage.clone(),
            vec![],
        );

        assert_eq!(passport.lineage, lineage);
        verify_passport(&passport).expect("valid");
    }
}
