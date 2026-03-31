//! TEE (Trusted Execution Environment) key backend with pluggable providers.
//!
//! Abstracts over hardware TEE providers (SGX, Nitro) with a software emulation
//! fallback. On initialization the backend probes available providers in order:
//! SGX → Nitro → SoftwareTee. The first available provider is selected.
//!
//! The [`TeeProvider`] trait defines the enclave boundary: key generation, signing,
//! and sealed storage all happen "inside" the provider. For real hardware, private
//! keys never leave the enclave. For the software fallback, the sealed-storage
//! abstraction from [`SealedKeyStore`] provides encryption-at-rest.

use crate::hardware_security::sealed_store::{derive_machine_secret, SealedKeyStore};
use crate::hardware_security::types::{
    attestation_payload, deterministic_attestation, hex_encode, platform_info_string, sha256_bytes,
    short_hash_prefix, AttestationReport, KeyBackend, KeyError, KeyHandle, KeyPurpose,
    PublicKeyBytes, SignatureBytes,
};
use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::OsRng;
use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// TeeProvider trait — the enclave abstraction
// ---------------------------------------------------------------------------

/// Abstraction over a TEE provider. Each method represents an operation that
/// would happen inside a hardware enclave on real TEE hardware.
pub trait TeeProvider: Send {
    /// Human-readable provider name (e.g. `"sgx"`, `"nitro"`, `"software-tee"`).
    fn provider_name(&self) -> &'static str;

    /// Whether this provider is functional on the current machine.
    fn is_available(&self) -> bool;

    /// Generate an Ed25519 keypair inside the enclave.
    /// Returns `(public_key_32_bytes, opaque_handle)`.
    /// For hardware TEEs the handle is an enclave-internal reference;
    /// for the software provider it is the sealed key ID.
    fn generate_key_in_enclave(&mut self, purpose: &str) -> Result<(Vec<u8>, Vec<u8>), KeyError>;

    /// Sign `message` using the key identified by `handle`, inside the enclave.
    fn sign_in_enclave(&self, handle: &[u8], message: &[u8]) -> Result<Vec<u8>, KeyError>;

    /// Retrieve the public key for a given handle.
    fn public_key_for_handle(&self, handle: &[u8]) -> Result<Vec<u8>, KeyError>;

    /// Seal arbitrary data using the enclave's sealing key.
    fn seal_data(&self, data: &[u8]) -> Result<Vec<u8>, KeyError>;

    /// Unseal data previously sealed by this enclave.
    fn unseal_data(&self, sealed: &[u8]) -> Result<Vec<u8>, KeyError>;

    /// Produce a remote attestation report with a caller-provided nonce for freshness.
    fn remote_attest(&self, nonce: &str) -> Result<AttestationReport, KeyError>;
}

// ---------------------------------------------------------------------------
// SgxProvider — Intel SGX (stub with documented ecall signatures)
// ---------------------------------------------------------------------------

/// Intel SGX provider. Requires the `hardware-tee` feature flag.
///
/// In a real deployment this would load an SGX enclave via `sgx_urts` and
/// make ecalls for key operations. The ecall signatures would be:
///
/// ```c
/// // Enclave EDL definitions (enclave.edl):
/// trusted {
///     public sgx_status_t ecall_generate_ed25519(
///         [in, string] const char* purpose,
///         [out, size=32] uint8_t* public_key,
///         [out, size=64] uint8_t* sealed_handle
///     );
///     public sgx_status_t ecall_sign(
///         [in, size=handle_len] const uint8_t* sealed_handle, size_t handle_len,
///         [in, size=msg_len] const uint8_t* message, size_t msg_len,
///         [out, size=64] uint8_t* signature
///     );
///     public sgx_status_t ecall_get_public_key(
///         [in, size=handle_len] const uint8_t* sealed_handle, size_t handle_len,
///         [out, size=32] uint8_t* public_key
///     );
///     public sgx_status_t ecall_seal(
///         [in, size=data_len] const uint8_t* data, size_t data_len,
///         [out, size=sealed_len] uint8_t* sealed, size_t* sealed_len
///     );
///     public sgx_status_t ecall_unseal(
///         [in, size=sealed_len] const uint8_t* sealed, size_t sealed_len,
///         [out, size=data_len] uint8_t* data, size_t* data_len
///     );
///     public sgx_status_t ecall_remote_attest(
///         [out] attestation_report_t* report
///     );
/// };
/// ```
pub struct SgxProvider {
    _configured: bool,
}

impl Default for SgxProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SgxProvider {
    pub fn new() -> Self {
        Self {
            _configured: Self::detect_sgx(),
        }
    }

    /// Attempt to detect SGX support at runtime.
    /// Checks for `/dev/sgx_enclave` (SGX DCAP) or `/dev/isgx` (legacy driver).
    fn detect_sgx() -> bool {
        std::path::Path::new("/dev/sgx_enclave").exists()
            || std::path::Path::new("/dev/isgx").exists()
    }
}

impl TeeProvider for SgxProvider {
    fn provider_name(&self) -> &'static str {
        "sgx"
    }

    fn is_available(&self) -> bool {
        cfg!(feature = "hardware-tee") && Self::detect_sgx()
    }

    fn generate_key_in_enclave(&mut self, _purpose: &str) -> Result<(Vec<u8>, Vec<u8>), KeyError> {
        Err(KeyError::BackendFailure(
            "SGX enclave not available: no sgx_urts runtime linked".to_string(),
        ))
    }

    fn sign_in_enclave(&self, _handle: &[u8], _message: &[u8]) -> Result<Vec<u8>, KeyError> {
        Err(KeyError::BackendFailure(
            "SGX enclave not available: no sgx_urts runtime linked".to_string(),
        ))
    }

    fn public_key_for_handle(&self, _handle: &[u8]) -> Result<Vec<u8>, KeyError> {
        Err(KeyError::BackendFailure(
            "SGX enclave not available: no sgx_urts runtime linked".to_string(),
        ))
    }

    fn seal_data(&self, _data: &[u8]) -> Result<Vec<u8>, KeyError> {
        Err(KeyError::BackendFailure(
            "SGX enclave not available: no sgx_urts runtime linked".to_string(),
        ))
    }

    fn unseal_data(&self, _sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        Err(KeyError::BackendFailure(
            "SGX enclave not available: no sgx_urts runtime linked".to_string(),
        ))
    }

    fn remote_attest(&self, nonce: &str) -> Result<AttestationReport, KeyError> {
        Ok(deterministic_attestation(
            self.provider_name(),
            false,
            nonce,
        ))
    }
}

// ---------------------------------------------------------------------------
// NitroProvider — AWS Nitro Enclaves
// ---------------------------------------------------------------------------

/// AWS Nitro Enclaves provider.
///
/// In a real deployment this would communicate with the Nitro Secure Module
/// (NSM) via `/dev/nsm` using the `aws-nitro-enclaves-nsm-api` crate.
/// Key operations would use the NSM's built-in attestation and sealing.
///
/// Detection: checks for `/dev/nsm` device and `/sys/devices/virtual/misc/nsm`.
pub struct NitroProvider {
    _configured: bool,
}

impl Default for NitroProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl NitroProvider {
    pub fn new() -> Self {
        Self {
            _configured: Self::detect_nitro(),
        }
    }

    /// Detect AWS Nitro Enclave environment at runtime.
    fn detect_nitro() -> bool {
        std::path::Path::new("/dev/nsm").exists()
    }
}

impl TeeProvider for NitroProvider {
    fn provider_name(&self) -> &'static str {
        "nitro"
    }

    fn is_available(&self) -> bool {
        cfg!(feature = "hardware-tee") && Self::detect_nitro()
    }

    fn generate_key_in_enclave(&mut self, _purpose: &str) -> Result<(Vec<u8>, Vec<u8>), KeyError> {
        Err(KeyError::BackendFailure(
            "Nitro enclave not available: not running inside an AWS Nitro Enclave".to_string(),
        ))
    }

    fn sign_in_enclave(&self, _handle: &[u8], _message: &[u8]) -> Result<Vec<u8>, KeyError> {
        Err(KeyError::BackendFailure(
            "Nitro enclave not available: not running inside an AWS Nitro Enclave".to_string(),
        ))
    }

    fn public_key_for_handle(&self, _handle: &[u8]) -> Result<Vec<u8>, KeyError> {
        Err(KeyError::BackendFailure(
            "Nitro enclave not available: not running inside an AWS Nitro Enclave".to_string(),
        ))
    }

    fn seal_data(&self, _data: &[u8]) -> Result<Vec<u8>, KeyError> {
        Err(KeyError::BackendFailure(
            "Nitro enclave not available: not running inside an AWS Nitro Enclave".to_string(),
        ))
    }

    fn unseal_data(&self, _sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        Err(KeyError::BackendFailure(
            "Nitro enclave not available: not running inside an AWS Nitro Enclave".to_string(),
        ))
    }

    fn remote_attest(&self, nonce: &str) -> Result<AttestationReport, KeyError> {
        Ok(deterministic_attestation(
            self.provider_name(),
            false,
            nonce,
        ))
    }
}

// ---------------------------------------------------------------------------
// SoftwareTeeProvider — software emulation using SealedKeyStore
// ---------------------------------------------------------------------------

/// Metadata for each key managed by the software TEE provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TeeKeyMeta {
    purpose: String,
    deprecated: bool,
}

/// Software TEE emulation. Always available as the last-resort fallback.
///
/// Keys are generated with [`OsRng`], stored encrypted at rest via
/// [`SealedKeyStore`]. During signing, the key is unsealed into memory,
/// used, then dropped. This mirrors the TEE pattern where private keys
/// only exist in plaintext inside the enclave's protected memory.
pub struct SoftwareTeeProvider {
    sealed_store: SealedKeyStore,
    /// In-memory cache of key handles → (public_key, purpose).
    /// The private key is NOT cached — it is only unsealed for signing.
    key_index: BTreeMap<String, (Vec<u8>, String)>,
    generation_counter: u64,
}

impl SoftwareTeeProvider {
    /// Create a provider with sealed storage at `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let master_secret = derive_machine_secret();
        let sealed_store = SealedKeyStore::new(&dir, &master_secret);

        let mut provider = Self {
            sealed_store,
            key_index: BTreeMap::new(),
            generation_counter: 0,
        };

        // Load existing sealed keys into the index.
        if let Ok(ids) = provider.sealed_store.list_sealed() {
            for id in ids {
                if let Ok(plaintext) = provider.sealed_store.unseal_key(&id) {
                    if let Ok((meta, pub_key)) = Self::decode_payload(&plaintext) {
                        // Track highest sequence for collision avoidance.
                        if let Some(seq) = Self::parse_sequence(&id) {
                            if seq > provider.generation_counter {
                                provider.generation_counter = seq;
                            }
                        }
                        provider.key_index.insert(id, (pub_key, meta.purpose));
                    }
                }
            }
        }

        provider
    }

    fn next_sequence(&mut self) -> u64 {
        self.generation_counter = self.generation_counter.saturating_add(1);
        self.generation_counter
    }

    /// Sealed payload format: `meta_len(4 LE) || meta_json || seed(32)`
    fn encode_payload(meta: &TeeKeyMeta, seed: &[u8; 32]) -> Vec<u8> {
        let meta_json = serde_json::to_vec(meta).unwrap_or_default();
        let meta_len = (meta_json.len() as u32).to_le_bytes();
        let mut buf = Vec::with_capacity(4 + meta_json.len() + 32);
        buf.extend_from_slice(&meta_len);
        buf.extend_from_slice(&meta_json);
        buf.extend_from_slice(seed);
        buf
    }

    fn decode_payload(plaintext: &[u8]) -> Result<(TeeKeyMeta, Vec<u8>), KeyError> {
        if plaintext.len() < 4 + 32 {
            return Err(KeyError::InvalidKeyMaterial(
                "tee sealed payload too short".to_string(),
            ));
        }
        let meta_len = u32::from_le_bytes(plaintext[..4].try_into().map_err(|_| {
            KeyError::InvalidKeyMaterial("tee payload: expected 4 bytes for length".to_string())
        })?) as usize;
        if plaintext.len() < 4 + meta_len + 32 {
            return Err(KeyError::InvalidKeyMaterial(
                "tee sealed payload truncated".to_string(),
            ));
        }
        let meta: TeeKeyMeta = serde_json::from_slice(&plaintext[4..4 + meta_len])
            .map_err(|e| KeyError::InvalidKeyMaterial(format!("tee metadata parse: {e}")))?;
        let seed_start = 4 + meta_len;
        let seed: [u8; 32] = plaintext[seed_start..seed_start + 32]
            .try_into()
            .map_err(|_| {
                KeyError::InvalidKeyMaterial("tee payload: expected 32 bytes for seed".to_string())
            })?;
        let identity = CryptoIdentity::from_bytes(SignatureAlgorithm::Ed25519, &seed)
            .map_err(|e| KeyError::InvalidKeyMaterial(format!("tee key reconstruct: {e}")))?;
        let pub_key = identity.verifying_key().to_vec();
        Ok((meta, pub_key))
    }

    fn parse_sequence(handle_id: &str) -> Option<u64> {
        for part in handle_id.split('-') {
            if part.len() == 8 {
                if let Ok(seq) = u64::from_str_radix(part, 16) {
                    return Some(seq);
                }
            }
        }
        None
    }

    /// Unseal the signing key from disk, use it, then drop it.
    /// This mirrors TEE behavior: private key only in protected memory during use.
    fn with_identity<F, T>(&self, handle_id: &str, f: F) -> Result<T, KeyError>
    where
        F: FnOnce(&CryptoIdentity) -> Result<T, KeyError>,
    {
        let plaintext = self.sealed_store.unseal_key(handle_id)?;
        if plaintext.len() < 4 + 32 {
            return Err(KeyError::InvalidKeyMaterial(
                "tee sealed payload too short".to_string(),
            ));
        }
        let meta_len = u32::from_le_bytes(plaintext[..4].try_into().map_err(|_| {
            KeyError::InvalidKeyMaterial("tee payload: expected 4 bytes for length".to_string())
        })?) as usize;
        let seed_start = 4 + meta_len;
        if plaintext.len() < seed_start + 32 {
            return Err(KeyError::InvalidKeyMaterial(
                "tee sealed payload truncated".to_string(),
            ));
        }
        let seed: [u8; 32] = plaintext[seed_start..seed_start + 32]
            .try_into()
            .map_err(|_| {
                KeyError::InvalidKeyMaterial("tee payload: expected 32 bytes for seed".to_string())
            })?;
        let identity = CryptoIdentity::from_bytes(SignatureAlgorithm::Ed25519, &seed)
            .map_err(|e| KeyError::InvalidKeyMaterial(format!("tee key: {e}")))?;
        // identity is dropped when this scope ends — private key leaves memory.
        f(&identity)
    }
}

impl TeeProvider for SoftwareTeeProvider {
    fn provider_name(&self) -> &'static str {
        "software-tee"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn generate_key_in_enclave(&mut self, purpose: &str) -> Result<(Vec<u8>, Vec<u8>), KeyError> {
        let seq = self.next_sequence();

        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        let identity = CryptoIdentity::from_bytes(SignatureAlgorithm::Ed25519, &seed)
            .map_err(|e| KeyError::BackendFailure(format!("tee keygen: {e}")))?;
        let pub_key = identity.verifying_key().to_vec();

        let handle_id = format!("tee-{purpose}-{seq:08x}-{}", short_hash_prefix(&pub_key));

        let meta = TeeKeyMeta {
            purpose: purpose.to_string(),
            deprecated: false,
        };
        let payload = Self::encode_payload(&meta, &seed);
        self.sealed_store.seal_key(&handle_id, &payload)?;

        // Cache public key in index; private key is NOT cached.
        self.key_index
            .insert(handle_id.clone(), (pub_key.clone(), purpose.to_string()));

        Ok((pub_key, handle_id.into_bytes()))
    }

    fn sign_in_enclave(&self, handle: &[u8], message: &[u8]) -> Result<Vec<u8>, KeyError> {
        let handle_id = std::str::from_utf8(handle)
            .map_err(|_| KeyError::InvalidKeyMaterial("handle is not UTF-8".to_string()))?;

        // Unseal key, sign, drop key.
        self.with_identity(handle_id, |identity| {
            identity
                .sign(message)
                .map_err(|e| KeyError::BackendFailure(format!("tee sign: {e}")))
        })
    }

    fn public_key_for_handle(&self, handle: &[u8]) -> Result<Vec<u8>, KeyError> {
        let handle_id = std::str::from_utf8(handle)
            .map_err(|_| KeyError::InvalidKeyMaterial("handle is not UTF-8".to_string()))?;

        // Try in-memory index first (no need to unseal for public key).
        if let Some((pub_key, _)) = self.key_index.get(handle_id) {
            return Ok(pub_key.clone());
        }

        // Fallback: unseal to extract public key.
        let plaintext = self.sealed_store.unseal_key(handle_id)?;
        let (_meta, pub_key) = Self::decode_payload(&plaintext)?;
        Ok(pub_key)
    }

    fn seal_data(&self, data: &[u8]) -> Result<Vec<u8>, KeyError> {
        // Use the sealed store's encryption for arbitrary data.
        // Store under a random handle.
        let mut id_bytes = [0u8; 16];
        OsRng.fill_bytes(&mut id_bytes);
        let data_id = format!(
            "sealed-data-{}",
            id_bytes
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        self.sealed_store.seal_key(&data_id, data)?;
        Ok(data_id.into_bytes())
    }

    fn unseal_data(&self, sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        let data_id = std::str::from_utf8(sealed)
            .map_err(|_| KeyError::InvalidKeyMaterial("sealed ref is not UTF-8".to_string()))?;
        self.sealed_store.unseal_key(data_id)
    }

    fn remote_attest(&self, nonce: &str) -> Result<AttestationReport, KeyError> {
        // Pick the first key in the index to attest, or return an unsigned report.
        let (handle_id, (pub_key, _purpose)) = match self.key_index.iter().next() {
            Some(entry) => (entry.0.clone(), entry.1.clone()),
            None => {
                return Ok(deterministic_attestation(self.provider_name(), true, nonce));
            }
        };

        let public_key_hash = hex_encode(&sha256_bytes(&pub_key));

        let mut input = Vec::new();
        input.extend_from_slice(self.provider_name().as_bytes());
        input.push(b':');
        input.extend_from_slice(b"available:v1");

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut report = AttestationReport {
            backend: self.provider_name().to_string(),
            available: true,
            device_claims_hash: sha256_bytes(&input),
            protocol_version: 1,
            provider: "software".to_string(),
            timestamp,
            public_key_hash,
            platform_info: platform_info_string(),
            nonce: nonce.to_string(),
            signature: Vec::new(),
        };

        // Self-sign the report: signature = pubkey(32) || ed25519_sig(64).
        let payload = attestation_payload(&report);
        self.with_identity(&handle_id, |identity| {
            let sig = identity
                .sign(&payload)
                .map_err(|e| KeyError::BackendFailure(format!("attest sign: {e}")))?;
            let mut sig_blob = Vec::with_capacity(96);
            sig_blob.extend_from_slice(identity.verifying_key());
            sig_blob.extend_from_slice(&sig);
            report.signature = sig_blob;
            Ok(())
        })?;

        Ok(report)
    }
}

// ---------------------------------------------------------------------------
// TeeBackend — KeyBackend implementation delegating to a TeeProvider
// ---------------------------------------------------------------------------

/// TEE key backend. Wraps a [`TeeProvider`] and implements [`KeyBackend`].
///
/// On construction, probes providers in order: SGX → Nitro → SoftwareTee.
/// The first available provider is selected.
pub struct TeeBackend {
    provider: Box<dyn TeeProvider>,
    /// Maps KeyHandle.id → opaque provider handle bytes.
    handle_map: BTreeMap<String, Vec<u8>>,
    /// Maps KeyHandle.id → KeyPurpose.
    purpose_map: BTreeMap<String, KeyPurpose>,
    generation_counter: u64,
}

impl TeeBackend {
    /// Create a TEE backend, auto-detecting the best available provider.
    /// `sealed_dir` is used by the SoftwareTeeProvider fallback.
    pub fn new(configured: bool, sealed_dir: Option<PathBuf>) -> Self {
        let provider = Self::select_provider(configured, sealed_dir);
        let mut backend = Self {
            provider,
            handle_map: BTreeMap::new(),
            purpose_map: BTreeMap::new(),
            generation_counter: 0,
        };
        backend.load_existing_keys();
        backend
    }

    /// Legacy constructor for backward compatibility with manager.rs.
    /// Uses a temp dir for sealed storage when no explicit dir is given.
    pub fn new_configured(configured: bool) -> Self {
        Self::new(configured, None)
    }

    fn select_provider(configured: bool, sealed_dir: Option<PathBuf>) -> Box<dyn TeeProvider> {
        if configured {
            // Try hardware providers first.
            let sgx = SgxProvider::new();
            if sgx.is_available() {
                return Box::new(sgx);
            }

            let nitro = NitroProvider::new();
            if nitro.is_available() {
                return Box::new(nitro);
            }
        }

        // Fallback: software TEE emulation.
        let dir = sealed_dir.unwrap_or_else(|| {
            let base = std::env::temp_dir().join("nexus-tee-keys");
            // Optional: dir creation failure is non-fatal here, SealedKeyStore will error later if needed
            std::fs::create_dir_all(&base).ok();
            base
        });
        Box::new(SoftwareTeeProvider::new(dir))
    }

    /// Load key index from the provider (for SoftwareTeeProvider, reads sealed dir).
    fn load_existing_keys(&mut self) {
        // For software-tee, the provider already loaded its index.
        // We need to reconstruct our handle_map and purpose_map from the provider's
        // sealed store listing. We can do this by querying each known handle.
        if self.provider.provider_name() == "software-tee" {
            // The SoftwareTeeProvider has an internal key_index but we can't access
            // it through the trait. Instead, we rely on the fact that keys are loaded
            // when the provider is created and will be found by handle ID.
            // The handle_map will be populated as keys are generated or used.
        }
    }

    fn next_sequence(&mut self) -> u64 {
        self.generation_counter = self.generation_counter.saturating_add(1);
        self.generation_counter
    }
}

impl KeyBackend for TeeBackend {
    fn backend_name(&self) -> &'static str {
        match self.provider.provider_name() {
            "sgx" => "tee-sgx",
            "nitro" => "tee-nitro",
            "software-tee" => "tee-software",
            other => {
                // Static str needed — use a safe fallback.
                if other == "sgx" {
                    "tee-sgx"
                } else {
                    "tee-software"
                }
            }
        }
    }

    fn is_available(&self) -> bool {
        self.provider.is_available()
    }

    fn generate_ed25519(&mut self, purpose: KeyPurpose) -> Result<KeyHandle, KeyError> {
        let (pub_key, opaque_handle) = self.provider.generate_key_in_enclave(purpose.as_str())?;

        let handle_id = format!(
            "tee-{}-{:08x}-{}",
            purpose.as_str(),
            self.next_sequence(),
            short_hash_prefix(&pub_key)
        );

        self.handle_map.insert(handle_id.clone(), opaque_handle);
        self.purpose_map.insert(handle_id.clone(), purpose);

        Ok(KeyHandle {
            id: handle_id,
            purpose,
        })
    }

    fn public_key(&self, handle: &KeyHandle) -> Result<PublicKeyBytes, KeyError> {
        let opaque = self
            .handle_map
            .get(&handle.id)
            .ok_or_else(|| KeyError::KeyNotFound(handle.id.clone()))?;

        if let Some(&stored_purpose) = self.purpose_map.get(&handle.id) {
            if stored_purpose != handle.purpose {
                return Err(KeyError::PurposeMismatch {
                    expected: handle.purpose,
                    actual: stored_purpose,
                });
            }
        }

        let pub_bytes = self.provider.public_key_for_handle(opaque)?;
        Ok(PublicKeyBytes(pub_bytes))
    }

    fn sign(&self, handle: &KeyHandle, msg: &[u8]) -> Result<SignatureBytes, KeyError> {
        let opaque = self
            .handle_map
            .get(&handle.id)
            .ok_or_else(|| KeyError::KeyNotFound(handle.id.clone()))?;

        if let Some(&stored_purpose) = self.purpose_map.get(&handle.id) {
            if stored_purpose != handle.purpose {
                return Err(KeyError::PurposeMismatch {
                    expected: handle.purpose,
                    actual: stored_purpose,
                });
            }
        }

        let sig_bytes = self.provider.sign_in_enclave(opaque, msg)?;
        Ok(SignatureBytes(sig_bytes))
    }

    fn rotate(&mut self, handle: &KeyHandle) -> Result<KeyHandle, KeyError> {
        let purpose = self
            .purpose_map
            .get(&handle.id)
            .copied()
            .ok_or_else(|| KeyError::KeyNotFound(handle.id.clone()))?;

        if purpose != handle.purpose {
            return Err(KeyError::PurposeMismatch {
                expected: handle.purpose,
                actual: purpose,
            });
        }

        // Generate a new key with the same purpose.
        self.generate_ed25519(purpose)
    }

    fn attest(&self, nonce: &str) -> Result<AttestationReport, KeyError> {
        self.provider.remote_attest(nonce)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};

    #[test]
    fn software_tee_provider_generate_and_sign() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut provider = SoftwareTeeProvider::new(dir.path());

        let (pub_key, handle) = provider
            .generate_key_in_enclave("agent_identity")
            .expect("generate key");
        assert_eq!(pub_key.len(), 32);
        assert!(!handle.is_empty());

        let msg = b"test message for tee";
        let sig = provider
            .sign_in_enclave(&handle, msg)
            .expect("sign in enclave");
        assert_eq!(sig.len(), 64);

        // Verify signature with public key.
        let ok = CryptoIdentity::verify(SignatureAlgorithm::Ed25519, &pub_key, msg, &sig)
            .expect("verify should not error");
        assert!(ok, "signature should verify");
    }

    #[test]
    fn software_tee_provider_keys_survive_reload() {
        let dir = tempfile::tempdir().expect("tempdir");

        let (pub_key, handle) = {
            let mut provider = SoftwareTeeProvider::new(dir.path());
            provider
                .generate_key_in_enclave("node_identity")
                .expect("generate")
        };

        // Recreate provider from same directory.
        let provider2 = SoftwareTeeProvider::new(dir.path());
        let pub_key2 = provider2
            .public_key_for_handle(&handle)
            .expect("public key after reload");
        assert_eq!(pub_key, pub_key2);

        // Sign should also work after reload.
        let sig = provider2
            .sign_in_enclave(&handle, b"after reload")
            .expect("sign after reload");
        assert_eq!(sig.len(), 64);
    }

    #[test]
    fn software_tee_seal_unseal_data() {
        let dir = tempfile::tempdir().expect("tempdir");
        let provider = SoftwareTeeProvider::new(dir.path());

        let data = b"sensitive configuration data";
        let sealed_ref = provider.seal_data(data).expect("seal");
        let recovered = provider.unseal_data(&sealed_ref).expect("unseal");
        assert_eq!(recovered.as_slice(), data);
    }

    #[test]
    fn tee_backend_with_software_fallback() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut backend = TeeBackend::new(false, Some(dir.path().to_path_buf()));

        assert!(backend.is_available());
        assert_eq!(backend.backend_name(), "tee-software");

        let handle = backend
            .generate_ed25519(KeyPurpose::AgentIdentity)
            .expect("generate");
        let pub_key = backend.public_key(&handle).expect("public key");
        assert_eq!(pub_key.0.len(), 32);

        let sig = backend.sign(&handle, b"tee backend test").expect("sign");
        assert_eq!(sig.0.len(), 64);

        // Verify.
        let ok = CryptoIdentity::verify(
            SignatureAlgorithm::Ed25519,
            &pub_key.0,
            b"tee backend test",
            &sig.0,
        )
        .expect("verify should not error");
        assert!(ok, "verify should pass");
    }

    #[test]
    fn tee_backend_rotation_produces_new_key() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut backend = TeeBackend::new(false, Some(dir.path().to_path_buf()));

        let original = backend
            .generate_ed25519(KeyPurpose::AgentIdentity)
            .expect("generate");
        let rotated = backend.rotate(&original).expect("rotate");

        assert_ne!(original.id, rotated.id);
        assert_eq!(original.purpose, rotated.purpose);

        // Both keys should work.
        backend
            .public_key(&original)
            .expect("original still accessible");
        backend.public_key(&rotated).expect("rotated accessible");
    }

    #[test]
    fn tee_backend_attestation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut backend = TeeBackend::new(false, Some(dir.path().to_path_buf()));

        // Generate a key so the software-tee provider can produce a signed report.
        backend
            .generate_ed25519(KeyPurpose::AgentIdentity)
            .expect("generate");

        let report = backend.attest("test-nonce").expect("attestation");
        assert_eq!(report.backend, "software-tee");
        assert!(report.available);
        assert_eq!(report.protocol_version, 1);
        assert_eq!(report.provider, "software");
        assert_eq!(report.nonce, "test-nonce");
        assert!(!report.public_key_hash.is_empty());
        assert!(!report.platform_info.is_empty());
        assert_eq!(report.signature.len(), 96);

        // Verify the attestation report.
        let valid = crate::hardware_security::types::verify_attestation(&report)
            .expect("verify should not error");
        assert!(valid, "signed attestation report should verify");
    }

    #[test]
    fn sgx_provider_not_available_in_ci() {
        let sgx = SgxProvider::new();
        // In CI there's no SGX hardware.
        assert!(!sgx.is_available());
    }

    #[test]
    fn nitro_provider_not_available_in_ci() {
        let nitro = NitroProvider::new();
        // In CI there's no Nitro enclave.
        assert!(!nitro.is_available());
    }
}
