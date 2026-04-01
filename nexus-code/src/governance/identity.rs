use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;

use crate::error::NxError;

/// Ed25519 session identity providing non-repudiable cryptographic identity per session.
pub struct SessionIdentity {
    session_id: String,
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
    created_at: DateTime<Utc>,
}

impl SessionIdentity {
    /// Generate a new session identity with a fresh Ed25519 keypair.
    pub fn new() -> Result<Self, NxError> {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        Ok(Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            signing_key,
            verifying_key,
            created_at: Utc::now(),
        })
    }

    /// Sign arbitrary bytes, return the signature.
    pub fn sign(&self, data: &[u8]) -> Signature {
        self.signing_key.sign(data)
    }

    /// Verify a signature against this session's public key.
    pub fn verify(&self, data: &[u8], signature: &Signature) -> bool {
        self.verifying_key.verify(data, signature).is_ok()
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get the public key bytes (32 bytes, for audit entries).
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    /// Get creation timestamp.
    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }
}
