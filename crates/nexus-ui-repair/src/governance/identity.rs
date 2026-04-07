//! Per-session Ed25519 identity. See v1.1 §7 `governance/identity.rs`.

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

/// A scout session's cryptographic identity.
///
/// Holds an Ed25519 signing key and a stable session id derived from
/// the public key. The session id is used in audit log entries and in
/// report headers so that any artifact produced by the scout can be
/// traced back to the session that produced it.
pub struct SessionIdentity {
    keypair: SigningKey,
    session_id: String,
}

impl SessionIdentity {
    /// Generate a fresh session identity with a random Ed25519 keypair.
    ///
    /// The session id is `"ses_"` followed by the first eight hex
    /// characters of the public key (giving a 12-character total).
    pub fn new() -> Self {
        let keypair = SigningKey::generate(&mut OsRng);
        let public_bytes = keypair.verifying_key().to_bytes();
        let mut hex = String::with_capacity(8);
        for byte in public_bytes.iter().take(4) {
            hex.push_str(&format!("{:02x}", byte));
        }
        let session_id = format!("ses_{}", hex);
        Self {
            keypair,
            session_id,
        }
    }

    /// The session id, e.g. `ses_8a3f1c20`.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Access to the underlying signing key (for future audit signing
    /// in Phase 1.2; unused in Phase 1.1).
    pub fn signing_key(&self) -> &SigningKey {
        &self.keypair
    }
}

impl Default for SessionIdentity {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_has_expected_shape() {
        let id = SessionIdentity::new();
        let s = id.session_id();
        assert!(s.starts_with("ses_"), "session_id must start with ses_");
        assert_eq!(s.len(), 12, "session_id must be 12 characters total");
    }
}
