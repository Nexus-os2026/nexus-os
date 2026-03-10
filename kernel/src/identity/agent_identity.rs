//! Agent cryptographic identity: keypair generation, DID derivation, signing,
//! and persistence via [`IdentityManager`].

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::OsRng;
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Multicodec prefix for Ed25519 public keys (0xed, 0x01).
const ED25519_MULTICODEC: [u8; 2] = [0xed, 0x01];

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors produced by identity operations.
#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("identity not found for agent {0}")]
    NotFound(Uuid),

    #[error("identity not found for DID {0}")]
    DidNotFound(String),

    #[error("signature verification failed")]
    VerificationFailed,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("invalid signing key bytes")]
    InvalidKeyBytes,
}

// ---------------------------------------------------------------------------
// Persisted form (JSON on disk)
// ---------------------------------------------------------------------------

/// Serialisable representation stored to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedIdentity {
    agent_id: Uuid,
    /// Raw 32-byte secret key, hex-encoded.
    secret_key_hex: String,
    did: String,
    created_at: u64,
}

// ---------------------------------------------------------------------------
// AgentIdentity
// ---------------------------------------------------------------------------

/// A single agent's cryptographic identity.
#[derive(Debug, Clone)]
pub struct AgentIdentity {
    /// UUID of the owning agent.
    pub agent_id: Uuid,
    /// Ed25519 signing key (includes the secret scalar).
    signing_key: SigningKey,
    /// `did:key:z6Mk…` string derived from the public key.
    pub did: String,
    /// Unix-epoch timestamp (seconds) when the identity was created.
    pub created_at: u64,
}

impl AgentIdentity {
    /// Create a brand-new identity with a random keypair.
    pub fn generate(agent_id: Uuid) -> Self {
        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        let signing_key = SigningKey::from_bytes(&seed);
        let did = did_from_public_key(&signing_key.verifying_key());
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            agent_id,
            signing_key,
            did,
            created_at,
        }
    }

    /// Sign an arbitrary payload, returning the 64-byte Ed25519 signature.
    pub fn sign(&self, payload: &[u8]) -> Vec<u8> {
        self.signing_key.sign(payload).to_bytes().to_vec()
    }

    /// Verify a signature produced by this identity.
    pub fn verify(&self, payload: &[u8], signature: &[u8]) -> Result<(), IdentityError> {
        let sig = ed25519_dalek::Signature::from_slice(signature)
            .map_err(|_| IdentityError::VerificationFailed)?;
        self.signing_key
            .verifying_key()
            .verify(payload, &sig)
            .map_err(|_| IdentityError::VerificationFailed)
    }

    /// Return the raw 32-byte public key.
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    // -- serialisation helpers ------------------------------------------------

    fn to_persisted(&self) -> PersistedIdentity {
        PersistedIdentity {
            agent_id: self.agent_id,
            secret_key_hex: hex_encode(&self.signing_key.to_bytes()),
            did: self.did.clone(),
            created_at: self.created_at,
        }
    }

    fn from_persisted(p: PersistedIdentity) -> Result<Self, IdentityError> {
        let bytes = hex_decode(&p.secret_key_hex).map_err(|_| IdentityError::InvalidKeyBytes)?;
        let key_bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| IdentityError::InvalidKeyBytes)?;
        let signing_key = SigningKey::from_bytes(&key_bytes);
        Ok(Self {
            agent_id: p.agent_id,
            signing_key,
            did: p.did,
            created_at: p.created_at,
        })
    }
}

// ---------------------------------------------------------------------------
// IdentityManager
// ---------------------------------------------------------------------------

/// Manages the set of agent identities: generation, persistence, and lookup.
#[derive(Debug, Clone)]
pub struct IdentityManager {
    identities: HashMap<Uuid, AgentIdentity>,
    /// Directory where identity JSON files are persisted.
    /// `None` means in-memory only (useful for tests).
    persist_dir: Option<PathBuf>,
}

impl IdentityManager {
    /// Create a manager that persists identities to `dir`.
    pub fn new(persist_dir: impl Into<PathBuf>) -> Self {
        Self {
            identities: HashMap::new(),
            persist_dir: Some(persist_dir.into()),
        }
    }

    /// Create an in-memory-only manager (no disk persistence).
    pub fn in_memory() -> Self {
        Self {
            identities: HashMap::new(),
            persist_dir: None,
        }
    }

    /// Generate a new identity for `agent_id`, persist it, and return a
    /// reference. If an identity already exists for this agent it is returned
    /// without generating a new one.
    pub fn get_or_create(&mut self, agent_id: Uuid) -> Result<&AgentIdentity, IdentityError> {
        if !self.identities.contains_key(&agent_id) {
            let identity = AgentIdentity::generate(agent_id);
            self.persist(&identity)?;
            self.identities.insert(agent_id, identity);
        }
        Ok(self.identities.get(&agent_id).expect("just inserted"))
    }

    /// Look up an identity by agent UUID.
    pub fn get(&self, agent_id: &Uuid) -> Option<&AgentIdentity> {
        self.identities.get(agent_id)
    }

    /// Look up an identity by its DID string.
    pub fn get_by_did(&self, did: &str) -> Option<&AgentIdentity> {
        self.identities.values().find(|id| id.did == did)
    }

    /// Load all persisted identities from the configured directory.
    pub fn load_all(&mut self) -> Result<usize, IdentityError> {
        let dir = match &self.persist_dir {
            Some(d) => d.clone(),
            None => return Ok(0),
        };
        if !dir.exists() {
            return Ok(0);
        }
        let mut count = 0usize;
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let data = std::fs::read_to_string(&path)?;
                let persisted: PersistedIdentity = serde_json::from_str(&data)?;
                let identity = AgentIdentity::from_persisted(persisted)?;
                self.identities.insert(identity.agent_id, identity);
                count += 1;
            }
        }
        Ok(count)
    }

    /// Remove an agent's identity from the manager and delete its persisted file.
    pub fn remove(&mut self, agent_id: &Uuid) -> Result<bool, IdentityError> {
        let existed = self.identities.remove(agent_id).is_some();
        if let Some(dir) = &self.persist_dir {
            let path = Self::identity_path(dir, agent_id);
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
        }
        Ok(existed)
    }

    // -- internal -------------------------------------------------------------

    fn persist(&self, identity: &AgentIdentity) -> Result<(), IdentityError> {
        let dir = match &self.persist_dir {
            Some(d) => d,
            None => return Ok(()),
        };
        std::fs::create_dir_all(dir)?;
        let path = Self::identity_path(dir, &identity.agent_id);
        let json = serde_json::to_string_pretty(&identity.to_persisted())?;
        std::fs::write(path, json)?;
        Ok(())
    }

    fn identity_path(dir: &Path, agent_id: &Uuid) -> PathBuf {
        dir.join(format!("{agent_id}.json"))
    }
}

// ---------------------------------------------------------------------------
// DID helpers
// ---------------------------------------------------------------------------

/// Derive a `did:key:z6Mk…` DID from an Ed25519 verifying (public) key.
///
/// Follows the `did:key` method specification:
///   multicodec(0xed01) ++ raw_public_key  →  base58btc  →  "did:key:z" ++ encoded
fn did_from_public_key(vk: &VerifyingKey) -> String {
    let mut buf = Vec::with_capacity(34);
    buf.extend_from_slice(&ED25519_MULTICODEC);
    buf.extend_from_slice(&vk.to_bytes());
    format!("did:key:z{}", bs58::encode(&buf).into_string())
}

// ---------------------------------------------------------------------------
// Tiny hex helpers (avoids pulling in the `hex` crate)
// ---------------------------------------------------------------------------

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(s: &str) -> Result<Vec<u8>, ()> {
    if !s.len().is_multiple_of(2) {
        return Err(());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_created_with_valid_did() {
        let agent_id = Uuid::new_v4();
        let identity = AgentIdentity::generate(agent_id);

        assert_eq!(identity.agent_id, agent_id);
        assert!(identity.did.starts_with("did:key:z6Mk"));
        assert!(identity.created_at > 0);
    }

    #[test]
    fn identity_persisted_and_reloaded() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let agent_id = Uuid::new_v4();

        // Create and persist.
        let (did, created_at) = {
            let mut mgr = IdentityManager::new(dir.path());
            let id = mgr.get_or_create(agent_id).expect("create identity");
            (id.did.clone(), id.created_at)
        };

        // Reload from disk.
        let mut mgr2 = IdentityManager::new(dir.path());
        let loaded = mgr2.load_all().expect("load identities");
        assert_eq!(loaded, 1);

        let reloaded = mgr2.get(&agent_id).expect("identity should exist");
        assert_eq!(reloaded.agent_id, agent_id);
        assert_eq!(reloaded.did, did);
        assert_eq!(reloaded.created_at, created_at);
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let identity = AgentIdentity::generate(Uuid::new_v4());
        let payload = b"hello nexus";

        let sig = identity.sign(payload);
        assert_eq!(sig.len(), 64);

        // Verify via the identity helper.
        identity.verify(payload, &sig).expect("verification ok");

        // Also verify with raw ed25519-dalek to prove interop.
        let vk = VerifyingKey::from_bytes(&identity.public_key_bytes()).unwrap();
        let sig_obj = ed25519_dalek::Signature::from_slice(&sig).unwrap();
        vk.verify(payload, &sig_obj).expect("raw verify ok");
    }

    #[test]
    fn two_agents_get_different_dids() {
        let a = AgentIdentity::generate(Uuid::new_v4());
        let b = AgentIdentity::generate(Uuid::new_v4());
        assert_ne!(a.did, b.did);
        assert_ne!(a.agent_id, b.agent_id);
    }

    #[test]
    fn lookup_by_did() {
        let mut mgr = IdentityManager::in_memory();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        mgr.get_or_create(id1).unwrap();
        mgr.get_or_create(id2).unwrap();

        let did1 = mgr.get(&id1).unwrap().did.clone();
        let found = mgr.get_by_did(&did1).expect("lookup by DID");
        assert_eq!(found.agent_id, id1);

        assert!(mgr.get_by_did("did:key:zNONEXISTENT").is_none());
    }

    #[test]
    fn get_or_create_is_idempotent() {
        let mut mgr = IdentityManager::in_memory();
        let id = Uuid::new_v4();
        let did1 = mgr.get_or_create(id).unwrap().did.clone();
        let did2 = mgr.get_or_create(id).unwrap().did.clone();
        assert_eq!(did1, did2);
    }

    #[test]
    fn tampered_signature_rejected() {
        let identity = AgentIdentity::generate(Uuid::new_v4());
        let sig = identity.sign(b"legit");
        let result = identity.verify(b"tampered", &sig);
        assert!(result.is_err());
    }
}
