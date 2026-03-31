//! X25519 key exchange backend — the ONLY module that directly imports `x25519_dalek`.
//!
//! Implements Elliptic-Curve Diffie-Hellman (ECDH) using Curve25519. Each party
//! generates an ephemeral keypair, exchanges public keys, and derives a 32-byte
//! shared secret. The shared secret can then be used as input to a KDF for
//! symmetric encryption.

use rand::rngs::OsRng;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::CryptoError;

/// Length of an X25519 public key in bytes.
pub(crate) const X25519_PUBLIC_KEY_LEN: usize = 32;
/// Length of the derived shared secret in bytes.
pub(crate) const X25519_SHARED_SECRET_LEN: usize = 32;

/// An X25519 key exchange keypair.
///
/// Uses `StaticSecret` (not `EphemeralSecret`) so the private key can be
/// serialised and restored across process restarts — required for long-lived
/// agent identities that negotiate sessions over time.
#[derive(Clone)]
pub(crate) struct X25519Keypair {
    secret_bytes: [u8; 32],
    public_key: PublicKey,
}

impl std::fmt::Debug for X25519Keypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("X25519Keypair")
            .field("public_key", &hex::encode(self.public_key.as_bytes()))
            .finish_non_exhaustive()
    }
}

// tiny hex encoder to avoid adding a dep
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}

impl X25519Keypair {
    /// Generate a fresh random keypair.
    pub(crate) fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public_key = PublicKey::from(&secret);
        Self {
            secret_bytes: secret.to_bytes(),
            public_key,
        }
    }

    /// Restore from raw 32-byte secret key.
    pub(crate) fn from_secret_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidKeyLength {
                expected: 32,
                actual: bytes.len(),
            })?;
        let secret = StaticSecret::from(arr);
        let public_key = PublicKey::from(&secret);
        Ok(Self {
            secret_bytes: arr,
            public_key,
        })
    }

    /// Return the public key bytes (32 bytes, safe to share).
    pub(crate) fn public_key_bytes(&self) -> [u8; X25519_PUBLIC_KEY_LEN] {
        *self.public_key.as_bytes()
    }

    /// Return the secret key bytes (32 bytes, NEVER share).
    pub(crate) fn secret_key_bytes(&self) -> &[u8; 32] {
        &self.secret_bytes
    }

    /// Perform ECDH: derive a 32-byte shared secret from our secret key and
    /// the peer's public key.
    pub(crate) fn diffie_hellman(
        &self,
        peer_public_key: &[u8],
    ) -> Result<[u8; X25519_SHARED_SECRET_LEN], CryptoError> {
        let peer_bytes: [u8; 32] =
            peer_public_key
                .try_into()
                .map_err(|_| CryptoError::InvalidKeyLength {
                    expected: X25519_PUBLIC_KEY_LEN,
                    actual: peer_public_key.len(),
                })?;
        let peer_pk = PublicKey::from(peer_bytes);
        let secret = StaticSecret::from(self.secret_bytes);
        let shared = secret.diffie_hellman(&peer_pk);
        Ok(*shared.as_bytes())
    }
}
