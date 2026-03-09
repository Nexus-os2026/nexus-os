//! Ed25519 signature verification for wasm modules.
//!
//! Before instantiation, `WasmtimeSandbox` checks that the wasm bytecode carries
//! a valid Ed25519 signature from a trusted publisher key. Unsigned modules are
//! rejected unless the sandbox's `SignaturePolicy` explicitly allows them.
//!
//! Signature format: the last 64 bytes of `agent_code` are the Ed25519 signature
//! over the preceding bytes (the raw wasm module). This keeps the format simple
//! and appendable — a publisher signs the .wasm binary and appends the signature.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};

/// Policy controlling whether unsigned wasm modules are accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignaturePolicy {
    /// Reject unsigned modules — only signed + verified modules run.
    RequireSigned,
    /// Allow unsigned modules (e.g. for development/testing).
    AllowUnsigned,
}

impl Default for SignaturePolicy {
    fn default() -> Self {
        Self::RequireSigned
    }
}

/// Result of signature verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureVerification {
    /// Signature present and valid.
    Valid,
    /// Module is unsigned and policy allows it.
    UnsignedAllowed,
    /// Module is unsigned but policy requires signatures.
    UnsignedRejected,
    /// Signature present but invalid (wrong key, tampered bytes, etc).
    Invalid { reason: String },
}

impl SignatureVerification {
    pub fn is_accepted(&self) -> bool {
        matches!(self, Self::Valid | Self::UnsignedAllowed)
    }
}

/// The Ed25519 signature is the last 64 bytes appended to the wasm binary.
const SIGNATURE_LEN: usize = 64;
/// Verify the Ed25519 signature on a wasm module.
///
/// `agent_code` is the full payload: `wasm_bytes || signature_bytes(64)`.
/// `trusted_keys` are the Ed25519 public keys accepted by this sandbox.
/// If `trusted_keys` is empty, any module is treated as unsigned.
///
/// Returns the verification result and, if valid, the wasm bytes (without the
/// appended signature) so the caller can compile only the wasm portion.
pub fn verify_wasm_signature<'a>(
    agent_code: &'a [u8],
    trusted_keys: &[VerifyingKey],
    policy: &SignaturePolicy,
) -> (SignatureVerification, &'a [u8]) {
    // If no trusted keys configured, everything is "unsigned"
    if trusted_keys.is_empty() {
        let result = match policy {
            SignaturePolicy::AllowUnsigned => SignatureVerification::UnsignedAllowed,
            SignaturePolicy::RequireSigned => SignatureVerification::UnsignedRejected,
        };
        return (result, agent_code);
    }

    // Need at least SIGNATURE_LEN bytes for the signature + some wasm bytes
    if agent_code.len() <= SIGNATURE_LEN {
        let result = match policy {
            SignaturePolicy::AllowUnsigned => SignatureVerification::UnsignedAllowed,
            SignaturePolicy::RequireSigned => SignatureVerification::UnsignedRejected,
        };
        return (result, agent_code);
    }

    let split_point = agent_code.len() - SIGNATURE_LEN;
    let wasm_bytes = &agent_code[..split_point];
    let sig_bytes = &agent_code[split_point..];

    // Try to parse the signature
    let signature = match Signature::from_slice(sig_bytes) {
        Ok(sig) => sig,
        Err(_) => {
            // Last 64 bytes aren't a valid signature format — treat as unsigned
            let result = match policy {
                SignaturePolicy::AllowUnsigned => SignatureVerification::UnsignedAllowed,
                SignaturePolicy::RequireSigned => SignatureVerification::UnsignedRejected,
            };
            return (result, agent_code);
        }
    };

    // Try each trusted key
    for key in trusted_keys {
        if key.verify(wasm_bytes, &signature).is_ok() {
            return (SignatureVerification::Valid, wasm_bytes);
        }
    }

    // Signature was parseable but didn't match any trusted key
    (
        SignatureVerification::Invalid {
            reason: "signature does not match any trusted key".to_string(),
        },
        agent_code,
    )
}

/// Sign wasm bytes with an Ed25519 signing key. Returns wasm || signature (64 bytes).
/// Used by publishers and test helpers.
pub fn sign_wasm_bytes(wasm_bytes: &[u8], signing_key: &ed25519_dalek::SigningKey) -> Vec<u8> {
    use ed25519_dalek::Signer;
    let signature = signing_key.sign(wasm_bytes);
    let mut signed = Vec::with_capacity(wasm_bytes.len() + SIGNATURE_LEN);
    signed.extend_from_slice(wasm_bytes);
    signed.extend_from_slice(&signature.to_bytes());
    signed
}

/// Create a test keypair for use in tests.
pub fn test_keypair() -> (ed25519_dalek::SigningKey, VerifyingKey) {
    use sha2::Digest;
    let seed = sha2::Sha256::digest(b"nexus-test-key-deterministic");
    let mut seed_bytes = [0u8; 32];
    seed_bytes.copy_from_slice(&seed);
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed_bytes);
    let verifying_key = signing_key.verifying_key();
    (signing_key, verifying_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;

    #[test]
    fn signed_module_accepted() {
        let (sk, vk) = test_keypair();
        let wasm = wat::parse_str("(module)").unwrap();
        let signed = sign_wasm_bytes(&wasm, &sk);

        let (result, extracted) =
            verify_wasm_signature(&signed, &[vk], &SignaturePolicy::RequireSigned);
        assert_eq!(result, SignatureVerification::Valid);
        assert_eq!(extracted, &wasm[..]);
    }

    #[test]
    fn unsigned_module_rejected_under_require_signed() {
        let wasm = wat::parse_str("(module)").unwrap();
        let (result, _) =
            verify_wasm_signature(&wasm, &[], &SignaturePolicy::RequireSigned);
        assert_eq!(result, SignatureVerification::UnsignedRejected);
    }

    #[test]
    fn unsigned_module_allowed_under_allow_unsigned() {
        let wasm = wat::parse_str("(module)").unwrap();
        let (result, _) =
            verify_wasm_signature(&wasm, &[], &SignaturePolicy::AllowUnsigned);
        assert_eq!(result, SignatureVerification::UnsignedAllowed);
    }

    #[test]
    fn wrong_key_rejected() {
        let (sk, _vk) = test_keypair();
        let wasm = wat::parse_str("(module)").unwrap();
        let signed = sign_wasm_bytes(&wasm, &sk);

        // Use a different key to verify
        let other_seed = sha2::Sha256::digest(b"other-key");
        let mut other_bytes = [0u8; 32];
        other_bytes.copy_from_slice(&other_seed);
        let other_sk = ed25519_dalek::SigningKey::from_bytes(&other_bytes);
        let other_vk = other_sk.verifying_key();

        let (result, _) =
            verify_wasm_signature(&signed, &[other_vk], &SignaturePolicy::RequireSigned);
        assert!(matches!(result, SignatureVerification::Invalid { .. }));
    }

    #[test]
    fn tampered_module_rejected() {
        let (sk, vk) = test_keypair();
        let wasm = wat::parse_str("(module)").unwrap();
        let mut signed = sign_wasm_bytes(&wasm, &sk);

        // Tamper with a byte in the wasm portion
        if !signed.is_empty() {
            signed[0] ^= 0xff;
        }

        let (result, _) =
            verify_wasm_signature(&signed, &[vk], &SignaturePolicy::RequireSigned);
        assert!(matches!(result, SignatureVerification::Invalid { .. }));
    }

    #[test]
    fn too_short_treated_as_unsigned() {
        let short = vec![0u8; 10]; // way too short for wasm + sig
        let (result, _) =
            verify_wasm_signature(&short, &[], &SignaturePolicy::AllowUnsigned);
        assert_eq!(result, SignatureVerification::UnsignedAllowed);
    }
}
