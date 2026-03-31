//! Ed25519 backend — the ONLY module that directly imports `ed25519_dalek`.
//!
//! All other code in the workspace should use [`CryptoIdentity`] and never
//! import `ed25519_dalek` directly. This makes it trivial to swap backends.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;

use crate::{CryptoError, CryptoIdentity, SignatureAlgorithm};

/// Ed25519 signing key: 32 bytes.  Verifying key: 32 bytes.
pub(crate) const ED25519_SIGNING_KEY_LEN: usize = 32;
pub(crate) const ED25519_VERIFYING_KEY_LEN: usize = 32;
pub(crate) const ED25519_SIGNATURE_LEN: usize = 64;

/// Generate a new Ed25519 identity with a random keypair.
pub(crate) fn generate() -> Result<CryptoIdentity, CryptoError> {
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    Ok(CryptoIdentity::from_raw_keys(
        SignatureAlgorithm::Ed25519,
        signing_key.to_bytes().to_vec(),
        verifying_key.to_bytes().to_vec(),
    ))
}

/// Sign a message with an Ed25519 signing key.
pub(crate) fn sign(signing_key_bytes: &[u8], message: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let key_bytes: [u8; ED25519_SIGNING_KEY_LEN] =
        signing_key_bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidKeyLength {
                expected: ED25519_SIGNING_KEY_LEN,
                actual: signing_key_bytes.len(),
            })?;

    let signing_key = SigningKey::from_bytes(&key_bytes);
    let signature = signing_key.sign(message);
    Ok(signature.to_bytes().to_vec())
}

/// Verify an Ed25519 signature.
pub(crate) fn verify(
    verifying_key_bytes: &[u8],
    message: &[u8],
    signature_bytes: &[u8],
) -> Result<bool, CryptoError> {
    let vk_bytes: [u8; ED25519_VERIFYING_KEY_LEN] =
        verifying_key_bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidKeyLength {
                expected: ED25519_VERIFYING_KEY_LEN,
                actual: verifying_key_bytes.len(),
            })?;

    let sig_bytes: [u8; ED25519_SIGNATURE_LEN] =
        signature_bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidKeyLength {
                expected: ED25519_SIGNATURE_LEN,
                actual: signature_bytes.len(),
            })?;

    let verifying_key =
        VerifyingKey::from_bytes(&vk_bytes).map_err(|_| CryptoError::VerificationFailed)?;
    let signature = Signature::from_bytes(&sig_bytes);

    match verifying_key.verify(message, &signature) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Reconstruct a CryptoIdentity from raw bytes (32-byte signing key).
/// The verifying key is derived from the signing key.
pub(crate) fn from_bytes(bytes: &[u8]) -> Result<CryptoIdentity, CryptoError> {
    if bytes.len() != ED25519_SIGNING_KEY_LEN {
        return Err(CryptoError::InvalidKeyLength {
            expected: ED25519_SIGNING_KEY_LEN,
            actual: bytes.len(),
        });
    }

    let key_bytes: [u8; ED25519_SIGNING_KEY_LEN] = bytes.try_into().map_err(|_| {
        CryptoError::DeserializationError("failed to convert bytes to array".into())
    })?;

    let signing_key = SigningKey::from_bytes(&key_bytes);
    let verifying_key = signing_key.verifying_key();

    Ok(CryptoIdentity::from_raw_keys(
        SignatureAlgorithm::Ed25519,
        signing_key.to_bytes().to_vec(),
        verifying_key.to_bytes().to_vec(),
    ))
}
