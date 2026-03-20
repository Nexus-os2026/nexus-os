//! Device pairing — Ed25519 keypair management, one-time pairing codes, and
//! persistent paired-device registry.
//!
//! Each device generates an Ed25519 keypair on first run, stored at a
//! configurable path. Pairing uses a one-time code that encodes the device's
//! public key and a random nonce. The accepting device validates the code and
//! derives a shared secret via SHA-256(local_pub || remote_pub || nonce).

use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Status of a device pairing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PairingStatus {
    Active,
    Revoked,
}

/// A pairing relationship between two devices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicePairing {
    pub local_node: Uuid,
    pub remote_node: Uuid,
    pub shared_secret: Vec<u8>,
    pub paired_at: u64,
    pub status: PairingStatus,
}

/// A one-time pairing code encoding a device's public key and nonce.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingCode {
    /// The node ID of the device that generated the code.
    pub node_id: Uuid,
    /// Ed25519 public key bytes (32 bytes).
    pub public_key: Vec<u8>,
    /// Random nonce for this pairing attempt (32 bytes).
    pub nonce: Vec<u8>,
}

impl PairingCode {
    /// Encode the pairing code as a hex string for transfer.
    pub fn encode(&self) -> String {
        let json = serde_json::to_vec(self).unwrap_or_default();
        hex::encode(&json)
    }

    /// Decode a hex-encoded pairing code.
    pub fn decode(hex_str: &str) -> Result<Self, String> {
        let bytes = hex::decode(hex_str).map_err(|e| format!("invalid hex: {e}"))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid pairing code: {e}"))
    }
}

/// Hex encoding/decoding helpers (no external dep needed).
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        if !s.len().is_multiple_of(2) {
            return Err("odd-length hex string".to_string());
        }
        (0..s.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| format!("invalid hex at {i}: {e}"))
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// DevicePairingManager
// ---------------------------------------------------------------------------

/// Manages Ed25519 device identity, pairing codes, and paired-device registry.
///
/// Keypair is stored at `key_path` (generated on first run).
/// Pairings are stored as JSON files in `pairings_dir`.
#[derive(Debug)]
pub struct DevicePairingManager {
    /// This device's node identity.
    local_node_id: Uuid,
    /// Ed25519 signing key for this device.
    signing_key: SigningKey,
    /// Directory where pairing JSON files are stored.
    pairings_dir: PathBuf,
    /// In-memory pairing registry keyed by remote node ID.
    pairings: HashMap<Uuid, DevicePairing>,
}

impl DevicePairingManager {
    /// Create or load a DevicePairingManager.
    ///
    /// - `local_node_id`: this device's UUID
    /// - `key_path`: path to the Ed25519 keypair file (created if missing)
    /// - `pairings_dir`: directory for persisted pairing JSON files
    pub fn open(
        local_node_id: Uuid,
        key_path: impl AsRef<Path>,
        pairings_dir: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let key_path = key_path.as_ref();
        let pairings_dir = pairings_dir.as_ref().to_path_buf();

        // Load or generate keypair
        let signing_key = load_or_generate_keypair(key_path)?;

        // Ensure pairings dir exists
        if !pairings_dir.exists() {
            fs::create_dir_all(&pairings_dir)
                .map_err(|e| format!("failed to create pairings dir: {e}"))?;
        }

        // Load existing pairings from disk
        let mut pairings = HashMap::new();
        if pairings_dir.is_dir() {
            let entries = fs::read_dir(&pairings_dir)
                .map_err(|e| format!("failed to read pairings dir: {e}"))?;
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    let content = fs::read_to_string(&path).map_err(|e| {
                        format!("failed to read pairing file '{}': {e}", path.display())
                    })?;
                    let pairing: DevicePairing = serde_json::from_str(&content).map_err(|e| {
                        format!("failed to parse pairing file '{}': {e}", path.display())
                    })?;
                    pairings.insert(pairing.remote_node, pairing);
                }
            }
        }

        Ok(Self {
            local_node_id,
            signing_key,
            pairings_dir,
            pairings,
        })
    }

    /// This device's Ed25519 public key bytes.
    pub fn public_key_bytes(&self) -> Vec<u8> {
        self.signing_key.verifying_key().to_bytes().to_vec()
    }

    /// This device's verifying key.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Generate a one-time pairing code for this device.
    pub fn generate_pairing_code(&self) -> PairingCode {
        let nonce = Uuid::new_v4().as_bytes().to_vec(); // 16 random bytes
                                                        // Extend to 32 bytes for consistency
        let mut nonce_32 = [0u8; 32];
        nonce_32[..16].copy_from_slice(&nonce);
        let extra = Uuid::new_v4();
        nonce_32[16..].copy_from_slice(extra.as_bytes());

        PairingCode {
            node_id: self.local_node_id,
            public_key: self.public_key_bytes(),
            nonce: nonce_32.to_vec(),
        }
    }

    /// Accept a pairing code from another device.
    ///
    /// Validates the code, derives a shared secret, stores the pairing, and
    /// persists to disk.
    pub fn accept_pairing(&mut self, code_hex: &str) -> Result<&DevicePairing, String> {
        let code = PairingCode::decode(code_hex)?;

        // Reject self-pairing
        if code.node_id == self.local_node_id {
            return Err("cannot pair with self".to_string());
        }

        // Reject if already paired (active)
        if let Some(existing) = self.pairings.get(&code.node_id) {
            if existing.status == PairingStatus::Active {
                return Err(format!("already paired with {}", code.node_id));
            }
            // If revoked, allow re-pairing by replacing
        }

        // Reject if public key is wrong length
        if code.public_key.len() != 32 {
            return Err(format!(
                "invalid public key length: expected 32, got {}",
                code.public_key.len()
            ));
        }

        // Derive shared secret: SHA-256(local_pub || remote_pub || nonce)
        let local_pub = self.public_key_bytes();
        let shared_secret = derive_shared_secret(&local_pub, &code.public_key, &code.nonce);

        let pairing = DevicePairing {
            local_node: self.local_node_id,
            remote_node: code.node_id,
            shared_secret,
            paired_at: current_unix_timestamp(),
            status: PairingStatus::Active,
        };

        self.pairings.insert(code.node_id, pairing);
        self.persist_pairing(code.node_id)?;

        self.pairings.get(&code.node_id)
            .ok_or_else(|| "pairing lookup failed after insert".to_string())
    }

    /// List all active paired devices.
    pub fn list_paired_devices(&self) -> Vec<&DevicePairing> {
        self.pairings
            .values()
            .filter(|p| p.status == PairingStatus::Active)
            .collect()
    }

    /// Revoke a pairing with a specific node.
    pub fn revoke_pairing(&mut self, node_id: Uuid) -> Result<(), String> {
        let pairing = self
            .pairings
            .get_mut(&node_id)
            .ok_or_else(|| format!("no pairing found for {node_id}"))?;

        pairing.status = PairingStatus::Revoked;
        self.persist_pairing(node_id)
    }

    /// Check whether a specific node is actively paired.
    pub fn is_paired(&self, node_id: Uuid) -> bool {
        self.pairings
            .get(&node_id)
            .map(|p| p.status == PairingStatus::Active)
            .unwrap_or(false)
    }

    /// The local node ID.
    pub fn local_node_id(&self) -> Uuid {
        self.local_node_id
    }

    fn persist_pairing(&self, remote_node: Uuid) -> Result<(), String> {
        let pairing = self
            .pairings
            .get(&remote_node)
            .ok_or_else(|| format!("pairing {remote_node} not in memory"))?;
        let encoded = serde_json::to_string_pretty(pairing)
            .map_err(|e| format!("failed to serialize pairing: {e}"))?;
        let path = self.pairings_dir.join(format!("{remote_node}.json"));
        fs::write(&path, encoded)
            .map_err(|e| format!("failed to write pairing file '{}': {e}", path.display()))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Derive a shared secret from two public keys and a nonce.
fn derive_shared_secret(local_pub: &[u8], remote_pub: &[u8], nonce: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(local_pub);
    hasher.update(remote_pub);
    hasher.update(nonce);
    hasher.finalize().to_vec()
}

/// Load an Ed25519 keypair from disk, or generate and persist a new one.
fn load_or_generate_keypair(path: &Path) -> Result<SigningKey, String> {
    if path.exists() {
        let bytes = fs::read(path)
            .map_err(|e| format!("failed to read keypair from '{}': {e}", path.display()))?;
        if bytes.len() != 32 {
            return Err(format!(
                "invalid keypair file '{}': expected 32 bytes, got {}",
                path.display(),
                bytes.len()
            ));
        }
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&bytes);
        Ok(SigningKey::from_bytes(&seed))
    } else {
        // Generate a new keypair from random seed
        let seed_uuid1 = Uuid::new_v4();
        let seed_uuid2 = Uuid::new_v4();
        let mut seed = [0u8; 32];
        seed[..16].copy_from_slice(seed_uuid1.as_bytes());
        seed[16..].copy_from_slice(seed_uuid2.as_bytes());

        let signing_key = SigningKey::from_bytes(&seed);

        // Persist seed bytes
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| format!("failed to create key dir: {e}"))?;
            }
        }
        fs::write(path, seed)
            .map_err(|e| format!("failed to write keypair to '{}': {e}", path.display()))?;

        Ok(signing_key)
    }
}

fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("nexus_device_pairing_tests_{}", std::process::id()))
            .join(name)
    }

    fn clean_dir(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    fn make_manager(name: &str) -> (DevicePairingManager, PathBuf) {
        let dir = test_dir(name);
        clean_dir(&dir);
        let key_path = dir.join("device.key");
        let pairings_dir = dir.join("pairings");
        let node_id = Uuid::new_v4();
        let mgr = DevicePairingManager::open(node_id, &key_path, &pairings_dir).unwrap();
        (mgr, dir)
    }

    #[test]
    fn generate_and_accept_pairing_roundtrip() {
        let (mgr_a, dir_a) = make_manager("roundtrip_a");
        let (mut mgr_b, dir_b) = make_manager("roundtrip_b");

        // Device A generates a pairing code
        let code = mgr_a.generate_pairing_code();
        let code_hex = code.encode();

        // Device B accepts it
        mgr_b.accept_pairing(&code_hex).unwrap();

        let pairing = mgr_b.pairings.get(&mgr_a.local_node_id()).unwrap();
        assert_eq!(pairing.remote_node, mgr_a.local_node_id());
        assert_eq!(pairing.local_node, mgr_b.local_node_id());
        assert_eq!(pairing.status, PairingStatus::Active);
        assert_eq!(pairing.shared_secret.len(), 32);

        // Device B now sees A as paired
        assert!(mgr_b.is_paired(mgr_a.local_node_id()));
        assert_eq!(mgr_b.list_paired_devices().len(), 1);

        clean_dir(&dir_a);
        clean_dir(&dir_b);
    }

    #[test]
    fn revoke_pairing() {
        let (mgr_a, dir_a) = make_manager("revoke_a");
        let (mut mgr_b, dir_b) = make_manager("revoke_b");

        let code = mgr_a.generate_pairing_code();
        mgr_b.accept_pairing(&code.encode()).unwrap();
        assert!(mgr_b.is_paired(mgr_a.local_node_id()));

        // Revoke
        mgr_b.revoke_pairing(mgr_a.local_node_id()).unwrap();
        assert!(!mgr_b.is_paired(mgr_a.local_node_id()));
        assert_eq!(mgr_b.list_paired_devices().len(), 0);

        clean_dir(&dir_a);
        clean_dir(&dir_b);
    }

    #[test]
    fn reject_invalid_code() {
        let (mut mgr, dir) = make_manager("invalid_code");

        let result = mgr.accept_pairing("not_valid_hex_at_all!!!");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid hex"));

        // Valid hex but not valid JSON
        let result = mgr.accept_pairing("deadbeef");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid pairing code"));

        clean_dir(&dir);
    }

    #[test]
    fn reject_self_pairing() {
        let (mut mgr, dir) = make_manager("self_pair");

        let code = mgr.generate_pairing_code();
        let result = mgr.accept_pairing(&code.encode());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot pair with self"));

        clean_dir(&dir);
    }

    #[test]
    fn reject_already_paired_device() {
        let (mgr_a, dir_a) = make_manager("dup_a");
        let (mut mgr_b, dir_b) = make_manager("dup_b");

        let code1 = mgr_a.generate_pairing_code();
        mgr_b.accept_pairing(&code1.encode()).unwrap();

        // Try to pair again while still active
        let code2 = mgr_a.generate_pairing_code();
        let result = mgr_b.accept_pairing(&code2.encode());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already paired"));

        clean_dir(&dir_a);
        clean_dir(&dir_b);
    }

    #[test]
    fn keypair_generated_and_persisted() {
        let dir = test_dir("keypair_persist");
        clean_dir(&dir);
        let key_path = dir.join("device.key");
        let pairings_dir = dir.join("pairings");
        let node_id = Uuid::new_v4();

        // First open generates keypair
        let pub_key_1 = {
            let mgr = DevicePairingManager::open(node_id, &key_path, &pairings_dir).unwrap();
            assert!(key_path.exists());
            assert_eq!(fs::read(&key_path).unwrap().len(), 32);
            mgr.public_key_bytes()
        };

        // Second open loads same keypair
        let pub_key_2 = {
            let mgr = DevicePairingManager::open(node_id, &key_path, &pairings_dir).unwrap();
            mgr.public_key_bytes()
        };

        assert_eq!(pub_key_1, pub_key_2);

        clean_dir(&dir);
    }

    #[test]
    fn list_returns_active_only() {
        let dir = test_dir("list_active");
        clean_dir(&dir);
        let key_path = dir.join("device.key");
        let pairings_dir = dir.join("pairings");
        let local_id = Uuid::new_v4();

        let mut mgr = DevicePairingManager::open(local_id, &key_path, &pairings_dir).unwrap();

        // Create two fake remote managers and pair both
        let (remote_a, dir_a) = make_manager("list_remote_a");
        let (remote_b, dir_b) = make_manager("list_remote_b");

        let code_a = remote_a.generate_pairing_code();
        let code_b = remote_b.generate_pairing_code();

        mgr.accept_pairing(&code_a.encode()).unwrap();
        mgr.accept_pairing(&code_b.encode()).unwrap();
        assert_eq!(mgr.list_paired_devices().len(), 2);

        // Revoke one
        mgr.revoke_pairing(remote_a.local_node_id()).unwrap();
        let active = mgr.list_paired_devices();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].remote_node, remote_b.local_node_id());

        clean_dir(&dir);
        clean_dir(&dir_a);
        clean_dir(&dir_b);
    }
}
