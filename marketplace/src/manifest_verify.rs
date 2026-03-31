//! Manifest signature verification and signing using SHA-256 + Ed25519.

use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestVerifyError {
    InvalidPublicKey,
    InvalidSignature,
    SignatureMismatch,
}

impl std::fmt::Display for ManifestVerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestVerifyError::InvalidPublicKey => write!(f, "invalid public key"),
            ManifestVerifyError::InvalidSignature => write!(f, "invalid signature bytes"),
            ManifestVerifyError::SignatureMismatch => {
                write!(f, "signature does not match manifest")
            }
        }
    }
}

impl std::error::Error for ManifestVerifyError {}

/// Sign a manifest hash with a CryptoIdentity. Returns the signature bytes.
pub fn sign_manifest(manifest_hash: &str, identity: &CryptoIdentity) -> Vec<u8> {
    let digest = sha256_digest(manifest_hash);
    identity.sign(&digest).unwrap_or_default()
}

/// Verify a manifest hash against a signature and public key.
pub fn verify_manifest_signature(
    manifest_hash: &str,
    signature_bytes: &[u8],
    public_key_bytes: &[u8],
) -> Result<bool, ManifestVerifyError> {
    let digest = sha256_digest(manifest_hash);

    CryptoIdentity::verify(
        SignatureAlgorithm::Ed25519,
        public_key_bytes,
        &digest,
        signature_bytes,
    )
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("key length") {
            if public_key_bytes.len() != 32 {
                ManifestVerifyError::InvalidPublicKey
            } else {
                ManifestVerifyError::InvalidSignature
            }
        } else {
            ManifestVerifyError::SignatureMismatch
        }
    })
    .and_then(|ok| {
        if ok {
            Ok(true)
        } else {
            Err(ManifestVerifyError::SignatureMismatch)
        }
    })
}

fn sha256_digest(input: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_identity(seed: &[u8; 32]) -> CryptoIdentity {
        CryptoIdentity::from_bytes(SignatureAlgorithm::Ed25519, seed).unwrap()
    }

    #[test]
    fn sign_and_verify_round_trip() {
        let id = test_identity(&[42u8; 32]);
        let hash = "abc123def456";

        let sig = sign_manifest(hash, &id);
        let result = verify_manifest_signature(hash, &sig, id.verifying_key());
        assert_eq!(result, Ok(true));
    }

    #[test]
    fn tampered_hash_fails_verification() {
        let id = test_identity(&[42u8; 32]);
        let hash = "abc123def456";
        let sig = sign_manifest(hash, &id);

        let result = verify_manifest_signature("tampered-hash", &sig, id.verifying_key());
        assert_eq!(result, Err(ManifestVerifyError::SignatureMismatch));
    }

    #[test]
    fn wrong_key_fails() {
        let id = test_identity(&[42u8; 32]);
        let other_id = test_identity(&[99u8; 32]);
        let hash = "test-hash";
        let sig = sign_manifest(hash, &id);

        let result = verify_manifest_signature(hash, &sig, other_id.verifying_key());
        assert_eq!(result, Err(ManifestVerifyError::SignatureMismatch));
    }

    #[test]
    fn invalid_key_length() {
        let result = verify_manifest_signature("hash", &[0u8; 64], &[0u8; 16]);
        assert_eq!(result, Err(ManifestVerifyError::InvalidPublicKey));
    }

    #[test]
    fn invalid_signature_length() {
        let result = verify_manifest_signature("hash", &[0u8; 32], &[0u8; 32]);
        assert_eq!(result, Err(ManifestVerifyError::InvalidSignature));
    }
}
