//! # nexus-crypto — Crypto-Agility Layer for Nexus OS
//!
//! This crate abstracts all cryptographic operations behind algorithm-agnostic
//! interfaces, enabling future migration to post-quantum cryptography (PQC)
//! without modifying consumer crates.
//!
//! ## Current Algorithms
//! - Ed25519 (signing/verification) — FIPS 186-5
//! - X25519 (key exchange) — Elliptic-Curve Diffie-Hellman
//!
//! ## Migration Guide for Other Crates
//! 1. Replace `ed25519_dalek::SigningKey` with `nexus_crypto::CryptoIdentity`
//! 2. Replace direct `sign()` calls with `identity.sign(message)`
//! 3. Replace direct `verify()` calls with `CryptoIdentity::verify(algo, key, msg, sig)`
//! 4. Store `SignatureAlgorithm` alongside any persisted signatures
//! 5. Use `CryptoConfig` to read system-wide algorithm policy
//!
//! ## Future PQC Algorithms (Phase 2)
//! - ML-DSA (FIPS 204) — post-quantum digital signatures
//! - ML-KEM (FIPS 203) — post-quantum key encapsulation
//! - SLH-DSA (FIPS 205) — stateless hash-based signatures
//! - Hybrid signatures (classical + PQC simultaneously)
//!
//! ## Example
//!
//! ```
//! use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};
//!
//! let identity = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
//! let signature = identity.sign(b"hello world").unwrap();
//! let valid = CryptoIdentity::verify(
//!     SignatureAlgorithm::Ed25519,
//!     identity.verifying_key(),
//!     b"hello world",
//!     &signature,
//! ).unwrap();
//! assert!(valid);
//! ```

mod ed25519_backend;
#[cfg(test)]
mod tests;
pub(crate) mod x25519_backend;

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════
// Algorithm enums
// ═══════════════════════════════════════════════════════════════════════════

/// Supported digital signature algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignatureAlgorithm {
    /// Ed25519 — Edwards-curve Digital Signature Algorithm (classical).
    Ed25519,
    // Future Phase 2:
    // MlDsa44,         // FIPS 204 — ML-DSA-44 (128-bit security)
    // MlDsa65,         // FIPS 204 — ML-DSA-65 (192-bit security)
    // MlDsa87,         // FIPS 204 — ML-DSA-87 (256-bit security)
    // SlhDsaShake128s, // FIPS 205 — SLH-DSA-SHAKE-128s
}

impl std::fmt::Display for SignatureAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ed25519 => write!(f, "Ed25519"),
        }
    }
}

/// Supported key exchange algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyExchangeAlgorithm {
    /// X25519 — Elliptic-curve Diffie-Hellman (classical).
    X25519,
    // Future Phase 2:
    // MlKem512,  // FIPS 203 — ML-KEM-512
    // MlKem768,  // FIPS 203 — ML-KEM-768
    // MlKem1024, // FIPS 203 — ML-KEM-1024
}

impl std::fmt::Display for KeyExchangeAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::X25519 => write!(f, "X25519"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// CryptoIdentity — algorithm-agnostic signing keypair
// ═══════════════════════════════════════════════════════════════════════════

/// A signing identity with algorithm metadata.
///
/// Keys are stored as `Vec<u8>` to support variable-size PQC keys:
/// - Ed25519: 32-byte signing key, 32-byte verifying key
/// - ML-DSA-65 (future): 4,032-byte signing key, 1,952-byte verifying key
#[derive(Debug, Clone)]
pub struct CryptoIdentity {
    algorithm: SignatureAlgorithm,
    signing_key: Vec<u8>,
    verifying_key: Vec<u8>,
}

impl CryptoIdentity {
    /// Generate a new random signing identity for the given algorithm.
    pub fn generate(algorithm: SignatureAlgorithm) -> Result<Self, CryptoError> {
        match algorithm {
            SignatureAlgorithm::Ed25519 => ed25519_backend::generate(),
        }
    }

    /// Sign a message.
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>, CryptoError> {
        match self.algorithm {
            SignatureAlgorithm::Ed25519 => ed25519_backend::sign(&self.signing_key, message),
        }
    }

    /// Verify a signature (static — does not require a CryptoIdentity instance).
    pub fn verify(
        algorithm: SignatureAlgorithm,
        verifying_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<bool, CryptoError> {
        match algorithm {
            SignatureAlgorithm::Ed25519 => {
                ed25519_backend::verify(verifying_key, message, signature)
            }
        }
    }

    /// Get the verifying (public) key bytes.
    pub fn verifying_key(&self) -> &[u8] {
        &self.verifying_key
    }

    /// Get the signing (private) key bytes.
    pub fn signing_key_bytes(&self) -> &[u8] {
        &self.signing_key
    }

    /// Get the algorithm.
    pub fn algorithm(&self) -> SignatureAlgorithm {
        self.algorithm
    }

    /// Serialize to bytes: [algorithm_byte | signing_key | verifying_key].
    pub fn to_bytes(&self) -> Vec<u8> {
        let algo_byte = match self.algorithm {
            SignatureAlgorithm::Ed25519 => 0x01u8,
        };
        let mut bytes = Vec::with_capacity(1 + self.signing_key.len() + self.verifying_key.len());
        bytes.push(algo_byte);
        bytes.extend_from_slice(&self.signing_key);
        bytes.extend_from_slice(&self.verifying_key);
        bytes
    }

    /// Deserialize from bytes.
    pub fn from_bytes(algorithm: SignatureAlgorithm, bytes: &[u8]) -> Result<Self, CryptoError> {
        match algorithm {
            SignatureAlgorithm::Ed25519 => ed25519_backend::from_bytes(bytes),
        }
    }

    /// Create from raw key bytes (for wrapping existing keys).
    pub fn from_raw_keys(
        algorithm: SignatureAlgorithm,
        signing_key: Vec<u8>,
        verifying_key: Vec<u8>,
    ) -> Self {
        Self {
            algorithm,
            signing_key,
            verifying_key,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// KeyExchange — algorithm-agnostic ECDH
// ═══════════════════════════════════════════════════════════════════════════

/// An X25519 key exchange keypair for Diffie-Hellman.
#[derive(Clone)]
pub struct KeyExchange {
    algorithm: KeyExchangeAlgorithm,
    secret_key: Vec<u8>,
    public_key: Vec<u8>,
}

impl std::fmt::Debug for KeyExchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyExchange")
            .field("algorithm", &self.algorithm)
            .field("public_key_len", &self.public_key.len())
            .finish_non_exhaustive()
    }
}

impl KeyExchange {
    /// Generate a new random key exchange keypair.
    pub fn generate(algorithm: KeyExchangeAlgorithm) -> Result<Self, CryptoError> {
        match algorithm {
            KeyExchangeAlgorithm::X25519 => {
                let kp = x25519_backend::X25519Keypair::generate();
                Ok(Self {
                    algorithm,
                    secret_key: kp.secret_key_bytes().to_vec(),
                    public_key: kp.public_key_bytes().to_vec(),
                })
            }
        }
    }

    /// Restore from a 32-byte secret key.
    pub fn from_secret_bytes(
        algorithm: KeyExchangeAlgorithm,
        bytes: &[u8],
    ) -> Result<Self, CryptoError> {
        match algorithm {
            KeyExchangeAlgorithm::X25519 => {
                let kp = x25519_backend::X25519Keypair::from_secret_bytes(bytes)?;
                Ok(Self {
                    algorithm,
                    secret_key: kp.secret_key_bytes().to_vec(),
                    public_key: kp.public_key_bytes().to_vec(),
                })
            }
        }
    }

    /// Get the public key bytes (safe to share with the peer).
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    /// Perform Diffie-Hellman: derive a shared secret from our secret key
    /// and the peer's public key. Both sides get the same 32-byte secret.
    pub fn diffie_hellman(&self, peer_public_key: &[u8]) -> Result<Vec<u8>, CryptoError> {
        match self.algorithm {
            KeyExchangeAlgorithm::X25519 => {
                let kp = x25519_backend::X25519Keypair::from_secret_bytes(&self.secret_key)?;
                let shared = kp.diffie_hellman(peer_public_key)?;
                Ok(shared.to_vec())
            }
        }
    }

    /// Get the algorithm.
    pub fn algorithm(&self) -> KeyExchangeAlgorithm {
        self.algorithm
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// HybridSignature — for future Phase 2 hybrid classical + PQC
// ═══════════════════════════════════════════════════════════════════════════

/// A hybrid signature combining classical and post-quantum signatures.
///
/// This is a data structure placeholder for Phase 2. In hybrid mode, BOTH
/// signatures must verify for the combined signature to be valid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSignature {
    pub classical: Option<Vec<u8>>,
    pub post_quantum: Option<Vec<u8>>,
    pub classical_algorithm: Option<SignatureAlgorithm>,
    pub post_quantum_algorithm: Option<SignatureAlgorithm>,
}

// ═══════════════════════════════════════════════════════════════════════════
// CryptoConfig — system-wide algorithm policy
// ═══════════════════════════════════════════════════════════════════════════

/// System-wide cryptographic policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoConfig {
    pub default_signature_algorithm: SignatureAlgorithm,
    pub default_key_exchange_algorithm: KeyExchangeAlgorithm,
    /// Phase 2: require both classical + PQC signatures on all operations.
    pub require_hybrid_signatures: bool,
    /// Minimum acceptable signature algorithm (policy enforcement).
    pub min_signature_algorithm: SignatureAlgorithm,
}

impl Default for CryptoConfig {
    fn default() -> Self {
        Self {
            default_signature_algorithm: SignatureAlgorithm::Ed25519,
            default_key_exchange_algorithm: KeyExchangeAlgorithm::X25519,
            require_hybrid_signatures: false,
            min_signature_algorithm: SignatureAlgorithm::Ed25519,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Errors
// ═══════════════════════════════════════════════════════════════════════════

/// Errors from cryptographic operations.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("unsupported algorithm: {0}")]
    UnsupportedAlgorithm(String),
    #[error("invalid key length: expected {expected}, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },
    #[error("signature verification failed")]
    VerificationFailed,
    #[error("key generation failed: {0}")]
    KeyGenerationFailed(String),
    #[error("signing failed: {0}")]
    SigningFailed(String),
    #[error("deserialization error: {0}")]
    DeserializationError(String),
}
