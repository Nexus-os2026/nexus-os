//! Ghost Protocol — Privacy-First P2P Agent Sync
//!
//! End-to-end encrypted agent state syncing across devices.
//! State changes propagate via AES-256-GCM encrypted gossip protocol
//! so your AI brain follows you without touching the cloud.

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Global nonce counter — ensures no two encryptions in the same process
/// ever reuse a nonce, even if the system clock is coarse.
static NONCE_COUNTER: AtomicU64 = AtomicU64::new(0);

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostConfig {
    pub enabled: bool,
    pub device_id: String,
    pub device_name: String,
    pub sync_interval_secs: u64,
    pub max_state_size_bytes: u64,
    pub encryption_enabled: bool,
}

impl Default for GhostConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            device_id: uuid::Uuid::new_v4().to_string(),
            device_name: "nexus-device".to_string(),
            sync_interval_secs: 30,
            max_state_size_bytes: 10 * 1024 * 1024, // 10 MB
            encryption_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub agent_states: HashMap<String, serde_json::Value>,
    pub version: u64,
    pub timestamp: u64,
    pub device_id: String,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncMessage {
    StateAdvertise {
        device_id: String,
        version: u64,
        checksum: String,
    },
    StateRequest {
        from_version: u64,
    },
    StateDelta {
        from_version: u64,
        to_version: u64,
        changes: Vec<StateChange>,
        encrypted: bool,
    },
    StateAck {
        version: u64,
    },
    Conflict {
        device_id: String,
        version: u64,
        resolution: ConflictResolution,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    pub agent_id: String,
    pub field: String,
    pub value: serde_json::Value,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConflictResolution {
    LastWriterWins,
    HigherVersionWins,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPeer {
    pub device_id: String,
    pub device_name: String,
    pub address: String,
    pub last_synced_version: u64,
    pub last_seen: u64,
    pub is_connected: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncStats {
    pub total_syncs: u64,
    pub total_conflicts: u64,
    pub total_changes_sent: u64,
    pub total_changes_received: u64,
    pub last_sync_time: Option<u64>,
    pub connected_peers: usize,
}

// ── AES-256-GCM Encryption ─────────────────────────────────────────────

/// Derive a 256-bit encryption key from a shared secret using SHA-256.
pub fn derive_sync_key(shared_secret: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(shared_secret.as_bytes());
    hasher.finalize().into()
}

/// Generate a unique 96-bit (12-byte) nonce.
///
/// Uses SHA-256 of (system-time nanoseconds ‖ atomic counter) truncated to
/// 12 bytes.  The atomic counter guarantees uniqueness even when the clock
/// resolution is coarser than the call rate.
fn generate_nonce() -> [u8; 12] {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = NONCE_COUNTER.fetch_add(1, Ordering::Relaxed);

    let mut hasher = Sha256::new();
    hasher.update(ts.to_le_bytes());
    hasher.update(counter.to_le_bytes());
    let hash = hasher.finalize();

    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&hash[..12]);
    nonce
}

/// Encrypt `data` with AES-256-GCM.
///
/// Returns `nonce (12 bytes) || ciphertext || tag (16 bytes)`.
pub fn encrypt_state(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new(key.into());
    let nonce_bytes = generate_nonce();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| format!("AES-256-GCM encryption failed: {e}"))?;

    let mut out = Vec::with_capacity(12 + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt `encrypted` (nonce || ciphertext || tag) with AES-256-GCM.
///
/// Returns the plaintext, or an error if the key is wrong or data is tampered.
pub fn decrypt_state(encrypted: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
    if encrypted.len() < 12 + 16 {
        return Err("Encrypted data too short — need at least nonce + tag (28 bytes)".to_string());
    }

    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher = Aes256Gcm::new(key.into());

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "AES-256-GCM decryption failed — wrong key or tampered data".to_string())
}

// ── GhostProtocol ───────────────────────────────────────────────────────

pub struct GhostProtocol {
    config: GhostConfig,
    current_state: SyncState,
    peers: Vec<SyncPeer>,
    pending_changes: Vec<StateChange>,
    sync_key: Option<[u8; 32]>,
    stats: SyncStats,
    conflict_resolution: ConflictResolution,
}

impl GhostProtocol {
    pub fn new(config: GhostConfig) -> Self {
        let device_id = config.device_id.clone();
        Self {
            config,
            current_state: SyncState {
                agent_states: HashMap::new(),
                version: 0,
                timestamp: 0,
                device_id,
                checksum: String::new(),
            },
            peers: Vec::new(),
            pending_changes: Vec::new(),
            sync_key: None,
            stats: SyncStats::default(),
            conflict_resolution: ConflictResolution::LastWriterWins,
        }
    }

    /// Set the encryption key derived from a shared secret.
    pub fn set_sync_key(&mut self, shared_secret: &str) {
        self.sync_key = Some(derive_sync_key(shared_secret));
    }

    /// Record a state change for an agent field.
    pub fn record_change(&mut self, agent_id: &str, field: &str, value: serde_json::Value) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.pending_changes.push(StateChange {
            agent_id: agent_id.to_string(),
            field: field.to_string(),
            value: value.clone(),
            timestamp,
        });

        // Apply locally immediately.
        let agent_state = self
            .current_state
            .agent_states
            .entry(agent_id.to_string())
            .or_insert_with(|| serde_json::json!({}));

        if let Some(obj) = agent_state.as_object_mut() {
            obj.insert(field.to_string(), value);
        }

        self.current_state.version += 1;
        self.current_state.timestamp = timestamp;
        self.current_state.checksum = self.compute_checksum();
    }

    /// Prepare a delta message containing changes since `from_version`.
    pub fn prepare_delta(&mut self, from_version: u64) -> SyncMessage {
        let changes: Vec<StateChange> = self
            .pending_changes
            .iter()
            .filter(|_| self.current_state.version > from_version)
            .cloned()
            .collect();

        let encrypted =
            self.config.encryption_enabled && self.sync_key.is_some() && !changes.is_empty();

        let final_changes = if encrypted {
            // Encrypt serialized changes with AES-256-GCM.
            let serialized = serde_json::to_vec(&changes).unwrap_or_default();
            let key = self.sync_key.as_ref().unwrap();
            let enc = encrypt_state(&serialized, key).expect("encryption must not fail");

            // Wrap encrypted blob as a single opaque change.
            vec![StateChange {
                agent_id: "__encrypted__".to_string(),
                field: "__blob__".to_string(),
                value: serde_json::Value::String(
                    enc.iter().map(|b| format!("{b:02x}")).collect::<String>(),
                ),
                timestamp: self.current_state.timestamp,
            }]
        } else {
            changes
        };

        self.stats.total_changes_sent += final_changes.len() as u64;

        SyncMessage::StateDelta {
            from_version,
            to_version: self.current_state.version,
            changes: final_changes,
            encrypted,
        }
    }

    /// Apply an incoming delta to local state. Returns number of changes applied.
    pub fn apply_delta(&mut self, delta: &SyncMessage) -> Result<usize, String> {
        let (changes, _from, to_version, encrypted) = match delta {
            SyncMessage::StateDelta {
                changes,
                from_version,
                to_version,
                encrypted,
            } => (changes, from_version, to_version, encrypted),
            _ => return Err("Expected StateDelta message".to_string()),
        };

        let resolved_changes = if *encrypted {
            // Decrypt the opaque blob with AES-256-GCM.
            let key = self
                .sync_key
                .as_ref()
                .ok_or_else(|| "No sync key set — cannot decrypt".to_string())?;

            let blob_change = changes
                .first()
                .ok_or_else(|| "Empty encrypted delta".to_string())?;

            let hex_str = blob_change
                .value
                .as_str()
                .ok_or_else(|| "Encrypted blob is not a string".to_string())?;

            let enc_bytes: Vec<u8> = (0..hex_str.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex_str[i..i + 2], 16))
                .collect::<Result<Vec<u8>, _>>()
                .map_err(|e| format!("Invalid hex in encrypted blob: {e}"))?;

            let decrypted = decrypt_state(&enc_bytes, key)?;

            serde_json::from_slice::<Vec<StateChange>>(&decrypted)
                .map_err(|e| format!("Failed to deserialize decrypted changes: {e}"))?
        } else {
            changes.clone()
        };

        let mut applied = 0;
        let mut conflicts = 0u64;

        for change in &resolved_changes {
            // Conflict detection: same agent+field exists locally with a different value.
            let has_conflict = self
                .current_state
                .agent_states
                .get(&change.agent_id)
                .and_then(|s| s.get(&change.field))
                .is_some_and(|existing| *existing != change.value);

            if has_conflict {
                conflicts += 1;
                match self.conflict_resolution {
                    ConflictResolution::LastWriterWins => {
                        // Accept the incoming change (remote is "last writer").
                    }
                    ConflictResolution::HigherVersionWins => {
                        if *to_version <= self.current_state.version {
                            continue; // Local version is higher — skip.
                        }
                    }
                    ConflictResolution::Manual => {
                        continue; // Skip — requires manual resolution.
                    }
                }
            }

            let agent_state = self
                .current_state
                .agent_states
                .entry(change.agent_id.clone())
                .or_insert_with(|| serde_json::json!({}));

            if let Some(obj) = agent_state.as_object_mut() {
                obj.insert(change.field.clone(), change.value.clone());
            }

            applied += 1;
        }

        self.current_state.version += 1;
        self.current_state.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.current_state.checksum = self.compute_checksum();

        self.stats.total_syncs += 1;
        self.stats.total_conflicts += conflicts;
        self.stats.total_changes_received += applied as u64;
        self.stats.last_sync_time = Some(self.current_state.timestamp);

        Ok(applied)
    }

    /// Add a sync peer.
    pub fn add_peer(&mut self, peer: SyncPeer) {
        self.peers.push(peer);
        self.stats.connected_peers = self.peers.iter().filter(|p| p.is_connected).count();
    }

    /// Remove a peer by device ID. Returns `true` if found.
    pub fn remove_peer(&mut self, device_id: &str) -> bool {
        let before = self.peers.len();
        self.peers.retain(|p| p.device_id != device_id);
        self.stats.connected_peers = self.peers.iter().filter(|p| p.is_connected).count();
        self.peers.len() < before
    }

    /// List all known peers.
    pub fn list_peers(&self) -> &[SyncPeer] {
        &self.peers
    }

    /// Current sync state.
    pub fn get_state(&self) -> &SyncState {
        &self.current_state
    }

    /// Current sync statistics.
    pub fn get_stats(&self) -> &SyncStats {
        &self.stats
    }

    /// Compute a deterministic SHA-256 checksum of the current state.
    pub fn compute_checksum(&self) -> String {
        let mut hasher = Sha256::new();
        // Sort keys for deterministic ordering.
        let mut keys: Vec<&String> = self.current_state.agent_states.keys().collect();
        keys.sort();
        for key in keys {
            hasher.update(key.as_bytes());
            if let Some(val) = self.current_state.agent_states.get(key) {
                hasher.update(val.to_string().as_bytes());
            }
        }
        hasher.update(self.current_state.version.to_le_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Current monotonic version.
    pub fn current_version(&self) -> u64 {
        self.current_state.version
    }

    /// Whether the protocol is enabled.
    pub fn enabled(&self) -> bool {
        self.config.enabled
    }

    /// Toggle the protocol on or off.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    /// The local device ID.
    pub fn device_id(&self) -> &str {
        &self.config.device_id
    }

    /// The local device name.
    pub fn device_name(&self) -> &str {
        &self.config.device_name
    }

    /// Access pending changes (for testing / inspection).
    pub fn pending_changes(&self) -> &[StateChange] {
        &self.pending_changes
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(name: &str) -> GhostConfig {
        GhostConfig {
            enabled: true,
            device_id: format!("device-{name}"),
            device_name: name.to_string(),
            sync_interval_secs: 30,
            max_state_size_bytes: 10 * 1024 * 1024,
            encryption_enabled: false,
        }
    }

    #[test]
    fn test_config_defaults() {
        let cfg = GhostConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.sync_interval_secs, 30);
        assert_eq!(cfg.max_state_size_bytes, 10 * 1024 * 1024);
        assert!(cfg.encryption_enabled);
        assert!(!cfg.device_id.is_empty());
    }

    #[test]
    fn test_record_change() {
        let mut gp = GhostProtocol::new(test_config("alice"));
        gp.record_change("agent-1", "mood", serde_json::json!("happy"));

        assert_eq!(gp.pending_changes().len(), 1);
        assert_eq!(gp.pending_changes()[0].agent_id, "agent-1");
        assert_eq!(gp.pending_changes()[0].field, "mood");
        assert_eq!(gp.pending_changes()[0].value, serde_json::json!("happy"));
        assert_eq!(gp.current_version(), 1);
    }

    #[test]
    fn test_prepare_delta() {
        let mut gp = GhostProtocol::new(test_config("alice"));
        gp.record_change("agent-1", "status", serde_json::json!("active"));
        gp.record_change("agent-2", "fuel", serde_json::json!(500));

        let delta = gp.prepare_delta(0);
        match delta {
            SyncMessage::StateDelta {
                from_version,
                to_version,
                changes,
                encrypted,
            } => {
                assert_eq!(from_version, 0);
                assert_eq!(to_version, 2);
                assert_eq!(changes.len(), 2);
                assert!(!encrypted);
            }
            _ => panic!("Expected StateDelta"),
        }
    }

    #[test]
    fn test_apply_delta() {
        let mut alice = GhostProtocol::new(test_config("alice"));
        alice.record_change("agent-1", "status", serde_json::json!("active"));
        alice.record_change("agent-1", "fuel", serde_json::json!(1000));

        let delta = alice.prepare_delta(0);

        let mut bob = GhostProtocol::new(test_config("bob"));
        let applied = bob.apply_delta(&delta).unwrap();
        assert_eq!(applied, 2);

        let state = bob.get_state();
        let agent_state = state.agent_states.get("agent-1").unwrap();
        assert_eq!(agent_state.get("status").unwrap(), "active");
        assert_eq!(agent_state.get("fuel").unwrap(), 1000);
    }

    // ── AES-256-GCM encryption tests ────────────────────────────────────

    #[test]
    fn test_encryption_roundtrip() {
        let key = derive_sync_key("my-secret-passphrase");
        let plaintext = b"Hello, Ghost Protocol!";

        let encrypted = encrypt_state(plaintext, &key).unwrap();
        // Ciphertext (after nonce) differs from plaintext.
        assert_ne!(&encrypted[12..], plaintext);

        let decrypted = decrypt_state(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encryption_wrong_key() {
        let key_a = derive_sync_key("correct-key");
        let key_b = derive_sync_key("wrong-key");
        let plaintext = b"Secret agent state";

        let encrypted = encrypt_state(plaintext, &key_a).unwrap();
        // AES-GCM authenticated decryption must fail with the wrong key.
        let result = decrypt_state(&encrypted, &key_b);
        assert!(result.is_err());
    }

    #[test]
    fn test_encryption_tampered_data() {
        let key = derive_sync_key("tamper-test-key");
        let plaintext = b"Integrity-protected payload";

        let mut encrypted = encrypt_state(plaintext, &key).unwrap();
        // Flip a byte in the ciphertext body (after the 12-byte nonce).
        let idx = 14;
        encrypted[idx] ^= 0xFF;

        let result = decrypt_state(&encrypted, &key);
        assert!(result.is_err(), "tampered data must fail authentication");
    }

    #[test]
    fn test_nonce_uniqueness() {
        let key = derive_sync_key("nonce-uniqueness-key");
        let plaintext = b"same data encrypted twice";

        let enc1 = encrypt_state(plaintext, &key).unwrap();
        let enc2 = encrypt_state(plaintext, &key).unwrap();

        // Different nonces → different ciphertext.
        assert_ne!(enc1, enc2);

        // But both decrypt to the same plaintext.
        assert_eq!(
            decrypt_state(&enc1, &key).unwrap(),
            decrypt_state(&enc2, &key).unwrap()
        );
    }

    #[test]
    fn test_encryption_empty_data() {
        let key = derive_sync_key("empty-test");
        let encrypted = encrypt_state(b"", &key).unwrap();
        let decrypted = decrypt_state(&encrypted, &key).unwrap();
        assert!(decrypted.is_empty());
    }

    #[test]
    fn test_decrypt_too_short() {
        let key = derive_sync_key("short-test");
        // Less than 28 bytes (12 nonce + 16 tag minimum).
        let result = decrypt_state(&[0u8; 20], &key);
        assert!(result.is_err());
    }

    // ── Encrypted sync tests ────────────────────────────────────────────

    #[test]
    fn test_encrypted_delta_sync() {
        let mut alice = GhostProtocol::new(GhostConfig {
            encryption_enabled: true,
            ..test_config("alice")
        });
        alice.set_sync_key("shared-secret-123");

        alice.record_change("agent-1", "status", serde_json::json!("active"));
        alice.record_change("agent-1", "fuel", serde_json::json!(1000));

        let delta = alice.prepare_delta(0);

        // Verify the delta is encrypted.
        match &delta {
            SyncMessage::StateDelta {
                encrypted, changes, ..
            } => {
                assert!(encrypted);
                assert_eq!(changes.len(), 1); // Single encrypted blob.
                assert_eq!(changes[0].agent_id, "__encrypted__");
            }
            _ => panic!("Expected StateDelta"),
        }

        // Bob decrypts with the same key.
        let mut bob = GhostProtocol::new(test_config("bob"));
        bob.set_sync_key("shared-secret-123");

        let applied = bob.apply_delta(&delta).unwrap();
        assert_eq!(applied, 2);

        let state = bob.get_state();
        let agent = state.agent_states.get("agent-1").unwrap();
        assert_eq!(agent.get("status").unwrap(), "active");
        assert_eq!(agent.get("fuel").unwrap(), 1000);
    }

    #[test]
    fn test_encrypted_delta_wrong_key_fails() {
        let mut alice = GhostProtocol::new(GhostConfig {
            encryption_enabled: true,
            ..test_config("alice")
        });
        alice.set_sync_key("alice-secret");
        alice.record_change("agent-1", "data", serde_json::json!("classified"));
        let delta = alice.prepare_delta(0);

        let mut eve = GhostProtocol::new(test_config("eve"));
        eve.set_sync_key("eve-wrong-key");

        let result = eve.apply_delta(&delta);
        assert!(result.is_err());
    }

    // ── Other protocol tests ────────────────────────────────────────────

    #[test]
    fn test_conflict_last_writer_wins() {
        let mut alice = GhostProtocol::new(test_config("alice"));
        let mut bob = GhostProtocol::new(test_config("bob"));

        // Bob sets a local value first.
        bob.record_change("agent-1", "mood", serde_json::json!("sad"));

        // Alice sets a different value and sends a delta.
        alice.record_change("agent-1", "mood", serde_json::json!("happy"));
        let delta = alice.prepare_delta(0);

        // Bob applies Alice's delta — last writer wins.
        let applied = bob.apply_delta(&delta).unwrap();
        assert_eq!(applied, 1);

        let mood = bob
            .get_state()
            .agent_states
            .get("agent-1")
            .unwrap()
            .get("mood")
            .unwrap()
            .clone();
        assert_eq!(mood, serde_json::json!("happy"));
        assert_eq!(bob.get_stats().total_conflicts, 1);
    }

    #[test]
    fn test_version_increments() {
        let mut gp = GhostProtocol::new(test_config("alice"));
        assert_eq!(gp.current_version(), 0);

        gp.record_change("a", "x", serde_json::json!(1));
        assert_eq!(gp.current_version(), 1);

        gp.record_change("a", "y", serde_json::json!(2));
        assert_eq!(gp.current_version(), 2);

        // apply_delta also increments.
        let mut bob = GhostProtocol::new(test_config("bob"));
        let delta = gp.prepare_delta(0);
        bob.apply_delta(&delta).unwrap();
        assert_eq!(bob.current_version(), 1);
    }

    #[test]
    fn test_checksum_deterministic() {
        let mut a = GhostProtocol::new(test_config("a"));
        let mut b = GhostProtocol::new(test_config("b"));

        // Record identical changes in both.
        a.record_change("agent-1", "status", serde_json::json!("ok"));
        b.record_change("agent-1", "status", serde_json::json!("ok"));

        // Both are at version 1 with the same data.
        assert_eq!(a.compute_checksum(), b.compute_checksum());
    }

    #[test]
    fn test_checksum_changes() {
        let mut gp = GhostProtocol::new(test_config("alice"));
        gp.record_change("agent-1", "status", serde_json::json!("ok"));
        let c1 = gp.compute_checksum();

        gp.record_change("agent-1", "status", serde_json::json!("error"));
        let c2 = gp.compute_checksum();

        assert_ne!(c1, c2);
    }

    #[test]
    fn test_add_remove_peer() {
        let mut gp = GhostProtocol::new(test_config("alice"));

        let peer = SyncPeer {
            device_id: "bob-device".to_string(),
            device_name: "Bob's Laptop".to_string(),
            address: "192.168.1.42:9100".to_string(),
            last_synced_version: 0,
            last_seen: 0,
            is_connected: true,
        };

        gp.add_peer(peer);
        assert_eq!(gp.list_peers().len(), 1);
        assert_eq!(gp.get_stats().connected_peers, 1);

        let removed = gp.remove_peer("bob-device");
        assert!(removed);
        assert!(gp.list_peers().is_empty());
        assert_eq!(gp.get_stats().connected_peers, 0);

        // Removing again returns false.
        assert!(!gp.remove_peer("bob-device"));
    }

    #[test]
    fn test_stats_tracking() {
        let mut alice = GhostProtocol::new(test_config("alice"));
        let mut bob = GhostProtocol::new(test_config("bob"));

        alice.record_change("agent-1", "x", serde_json::json!(1));
        alice.record_change("agent-1", "y", serde_json::json!(2));
        let delta = alice.prepare_delta(0);

        bob.apply_delta(&delta).unwrap();

        let stats = bob.get_stats();
        assert_eq!(stats.total_syncs, 1);
        assert_eq!(stats.total_changes_received, 2);
        assert!(stats.last_sync_time.is_some());
    }
}
