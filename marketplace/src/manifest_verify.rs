//! Manifest signature verification and signing using SHA-256 + Ed25519.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
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
            ManifestVerifyError::SignatureMismatch => write!(f, "signature does not match manifest"),
        }
    }
}

impl std::error::Error for ManifestVerifyError {}

/// Sign a manifest hash with an Ed25519 key. Returns the signature bytes.
pub fn sign_manifest(manifest_hash: &str, signing_key: &SigningKey) -> Vec<u8> {
    let digest = sha256_digest(manifest_hash);
    signing_key.sign(&digest).to_bytes().to_vec()
}

/// Verify a manifest hash against a signature and public key.
pub fn verify_manifest_signature(
    manifest_hash: &str,
    signature_bytes: &[u8],
    public_key_bytes: &[u8],
) -> Result<bool, ManifestVerifyError> {
    let pk_array: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| ManifestVerifyError::InvalidPublicKey)?;
    let sig_array: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| ManifestVerifyError::InvalidSignature)?;

    let verifying_key =
        VerifyingKey::from_bytes(&pk_array).map_err(|_| ManifestVerifyError::InvalidPublicKey)?;
    let signature = Signature::from_bytes(&sig_array);
    let digest = sha256_digest(manifest_hash);

    verifying_key
        .verify(&digest, &signature)
        .map(|_| true)
        .map_err(|_| ManifestVerifyError::SignatureMismatch)
}

fn sha256_digest(input: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_round_trip() {
        let key = SigningKey::from_bytes(&[42u8; 32]);
        let hash = "abc123def456";

        let sig = sign_manifest(hash, &key);
        let result = verify_manifest_signature(hash, &sig, &key.verifying_key().to_bytes());
        assert_eq!(result, Ok(true));
    }

    #[test]
    fn tampered_hash_fails_verification() {
        let key = SigningKey::from_bytes(&[42u8; 32]);
        let hash = "abc123def456";
        let sig = sign_manifest(hash, &key);

        let result =
            verify_manifest_signature("tampered-hash", &sig, &key.verifying_key().to_bytes());
        assert_eq!(result, Err(ManifestVerifyError::SignatureMismatch));
    }

    #[test]
    fn wrong_key_fails() {
        let key = SigningKey::from_bytes(&[42u8; 32]);
        let other_key = SigningKey::from_bytes(&[99u8; 32]);
        let hash = "test-hash";
        let sig = sign_manifest(hash, &key);

        let result =
            verify_manifest_signature(hash, &sig, &other_key.verifying_key().to_bytes());
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
