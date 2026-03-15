//! Content-addressable immutable audit storage.
//!
//! `AuditBlock` batches kernel `AuditEvent`s into signed, hash-chained blocks.
//! Each block's `content_hash` is the SHA-256 of its canonical contents, used as
//! a content-addressable key for lookup and tamper detection.
//!
//! Two storage backends are provided:
//! - `ContentAddressedStore` — in-memory `HashMap<String, AuditBlock>` for fast lookup
//! - `FileAuditStore` — persists blocks as individual JSON files in a directory

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use nexus_kernel::audit::AuditEvent;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

// ---------------------------------------------------------------------------
// AuditBlock
// ---------------------------------------------------------------------------

/// A batch of audit events forming one link in the immutable chain.
///
/// Events are preserved exactly as produced by the kernel — original UUIDs,
/// timestamps, and hashes are never regenerated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditBlock {
    /// SHA-256 content hash used as content-addressable key.
    pub content_hash: String,
    /// Content hash of the preceding block (genesis = all zeros).
    pub previous_hash: String,
    /// Kernel audit events preserved with original UUIDs and hashes.
    pub events: Vec<AuditEvent>,
    /// Identity of the node that created this block.
    pub node_id: Uuid,
    /// Unix timestamp (seconds) when the block was created.
    pub timestamp: u64,
    /// Monotonically increasing sequence number within this chain.
    pub sequence_number: u64,
    /// Ed25519 signature over the content hash bytes.
    pub signature: Vec<u8>,
}

impl AuditBlock {
    /// Compute the content hash for a block from its canonical fields.
    ///
    /// Follows the same SHA-256 pattern as `kernel/src/audit/mod.rs`:
    /// serialize a canonical struct to JSON, then hash
    /// `previous_hash_bytes || canonical_json_bytes`.
    pub fn compute_hash(
        events: &[AuditEvent],
        previous_hash: &str,
        node_id: Uuid,
        timestamp: u64,
        sequence_number: u64,
    ) -> String {
        #[derive(Serialize)]
        struct CanonicalBlockData<'a> {
            event_hashes: Vec<&'a str>,
            node_id: &'a str,
            timestamp: u64,
            sequence_number: u64,
        }

        let node_id_string = node_id.to_string();
        let event_hashes: Vec<&str> = events.iter().map(|e| e.hash.as_str()).collect();

        let canonical = CanonicalBlockData {
            event_hashes,
            node_id: &node_id_string,
            timestamp,
            sequence_number,
        };

        let serialized = match serde_json::to_vec(&canonical) {
            Ok(bytes) => bytes,
            Err(_) => {
                format!("{node_id}:{timestamp}:{sequence_number}:{}", events.len()).into_bytes()
            }
        };

        let mut hasher = Sha256::new();
        hasher.update(previous_hash.as_bytes());
        hasher.update(serialized);
        format!("{:x}", hasher.finalize())
    }

    /// Verify the block's content hash matches its fields.
    pub fn verify_hash(&self) -> bool {
        let expected = Self::compute_hash(
            &self.events,
            &self.previous_hash,
            self.node_id,
            self.timestamp,
            self.sequence_number,
        );
        self.content_hash == expected
    }

    /// Verify the Ed25519 signature on this block.
    pub fn verify_signature(&self, verifying_key: &VerifyingKey) -> bool {
        let Ok(signature) = Signature::from_slice(&self.signature) else {
            return false;
        };
        verifying_key
            .verify(self.content_hash.as_bytes(), &signature)
            .is_ok()
    }
}

// ---------------------------------------------------------------------------
// ContentAddressedStore — in-memory
// ---------------------------------------------------------------------------

/// In-memory content-addressable store keyed by block content hash.
#[derive(Debug, Clone, Default)]
pub struct ContentAddressedStore {
    blocks: HashMap<String, AuditBlock>,
}

impl ContentAddressedStore {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
        }
    }

    /// Insert a block. Returns `false` if a block with the same hash already exists.
    pub fn insert(&mut self, block: AuditBlock) -> bool {
        if self.blocks.contains_key(&block.content_hash) {
            return false;
        }
        self.blocks.insert(block.content_hash.clone(), block);
        true
    }

    /// Look up a block by its content hash.
    pub fn get(&self, content_hash: &str) -> Option<&AuditBlock> {
        self.blocks.get(content_hash)
    }

    /// Whether a block with the given hash exists.
    pub fn contains(&self, content_hash: &str) -> bool {
        self.blocks.contains_key(content_hash)
    }

    /// Number of blocks stored.
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Iterate over all stored blocks.
    pub fn blocks(&self) -> impl Iterator<Item = &AuditBlock> {
        self.blocks.values()
    }
}

// ---------------------------------------------------------------------------
// FileAuditStore — file-backed persistence
// ---------------------------------------------------------------------------

/// Persists audit blocks as individual JSON files in a directory.
///
/// Each block is stored as `<content_hash>.json`. On load, all `.json` files
/// in the directory are read back into memory.
#[derive(Debug)]
pub struct FileAuditStore {
    dir: PathBuf,
    store: ContentAddressedStore,
}

impl FileAuditStore {
    /// Create a new file-backed store at the given directory.
    ///
    /// If the directory exists, all `.json` files are loaded. If it does not
    /// exist, it is created.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, String> {
        let dir = dir.as_ref().to_path_buf();
        if !dir.exists() {
            fs::create_dir_all(&dir)
                .map_err(|e| format!("failed to create audit store dir: {e}"))?;
        }

        let mut store = ContentAddressedStore::new();

        if dir.is_dir() {
            let entries =
                fs::read_dir(&dir).map_err(|e| format!("failed to read audit store dir: {e}"))?;
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    let content = fs::read_to_string(&path).map_err(|e| {
                        format!("failed to read block file '{}': {e}", path.display())
                    })?;
                    let block: AuditBlock = serde_json::from_str(&content).map_err(|e| {
                        format!("failed to parse block file '{}': {e}", path.display())
                    })?;
                    store.insert(block);
                }
            }
        }

        Ok(Self { dir, store })
    }

    /// Persist a block to disk and insert into the in-memory store.
    pub fn insert(&mut self, block: AuditBlock) -> Result<bool, String> {
        let hash = block.content_hash.clone();
        if !self.store.insert(block) {
            return Ok(false);
        }
        self.persist_block(&hash)?;
        Ok(true)
    }

    /// Look up a block by content hash.
    pub fn get(&self, content_hash: &str) -> Option<&AuditBlock> {
        self.store.get(content_hash)
    }

    /// Number of blocks stored.
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    fn persist_block(&self, content_hash: &str) -> Result<(), String> {
        let block = self
            .store
            .get(content_hash)
            .ok_or_else(|| format!("block {content_hash} not in store"))?;
        let encoded = serde_json::to_string_pretty(block)
            .map_err(|e| format!("failed to serialize block: {e}"))?;
        let path = self.dir.join(format!("{content_hash}.json"));
        fs::write(&path, encoded)
            .map_err(|e| format!("failed to write block file '{}': {e}", path.display()))
    }
}

// ---------------------------------------------------------------------------
// AuditChain — ordered chain of blocks
// ---------------------------------------------------------------------------

/// An ordered, hash-linked chain of `AuditBlock`s with content-addressable lookup.
#[derive(Debug, Clone)]
pub struct AuditChain {
    node_id: Uuid,
    /// Blocks in chain order (index == sequence_number).
    pub(crate) chain: Vec<AuditBlock>,
    /// Content-addressable index.
    store: ContentAddressedStore,
}

impl AuditChain {
    /// Create a new empty chain for the given node.
    pub fn new(node_id: Uuid) -> Self {
        Self {
            node_id,
            chain: Vec::new(),
            store: ContentAddressedStore::new(),
        }
    }

    /// Append a new block containing the given events, signed with the given key.
    ///
    /// The block's `previous_hash` is set to the latest block's content hash,
    /// or the genesis hash if the chain is empty.
    pub fn append_block(
        &mut self,
        events: Vec<AuditEvent>,
        signing_key: &SigningKey,
    ) -> &AuditBlock {
        let previous_hash = self
            .chain
            .last()
            .map(|b| b.content_hash.clone())
            .unwrap_or_else(|| GENESIS_HASH.to_string());

        let sequence_number = self.chain.len() as u64;
        let timestamp = current_unix_timestamp();

        let content_hash = AuditBlock::compute_hash(
            &events,
            &previous_hash,
            self.node_id,
            timestamp,
            sequence_number,
        );

        let signature = signing_key.sign(content_hash.as_bytes());

        let block = AuditBlock {
            content_hash,
            previous_hash,
            events,
            node_id: self.node_id,
            timestamp,
            sequence_number,
            signature: signature.to_bytes().to_vec(),
        };

        self.store.insert(block.clone());
        self.chain.push(block);
        self.chain.last().unwrap()
    }

    /// Append a pre-built, pre-signed block (e.g. received from a peer via gossip).
    ///
    /// The caller is responsible for verifying the block's hash and signature
    /// before calling this method.
    pub fn append_verified_block(&mut self, block: AuditBlock) {
        self.store.insert(block.clone());
        self.chain.push(block);
    }

    /// Look up a block by its content hash.
    pub fn get_block_by_hash(&self, content_hash: &str) -> Option<&AuditBlock> {
        self.store.get(content_hash)
    }

    /// Look up a block by its sequence number.
    pub fn get_block_by_sequence(&self, sequence_number: u64) -> Option<&AuditBlock> {
        self.chain.get(sequence_number as usize)
    }

    /// The most recently appended block, if any.
    pub fn latest_block(&self) -> Option<&AuditBlock> {
        self.chain.last()
    }

    /// Number of blocks in the chain.
    pub fn chain_length(&self) -> usize {
        self.chain.len()
    }

    /// The node ID this chain belongs to.
    pub fn node_id(&self) -> Uuid {
        self.node_id
    }

    /// Verify the entire chain: hash linkage, content hashes, and signatures.
    pub fn verify_integrity(&self, verifying_key: &VerifyingKey) -> bool {
        let mut expected_previous = GENESIS_HASH.to_string();

        for (i, block) in self.chain.iter().enumerate() {
            if block.sequence_number != i as u64 {
                return false;
            }
            if block.previous_hash != expected_previous {
                return false;
            }
            if !block.verify_hash() {
                return false;
            }
            if !block.verify_signature(verifying_key) {
                return false;
            }
            expected_previous = block.content_hash.clone();
        }

        true
    }
}

// ---------------------------------------------------------------------------
// TamperResult — detailed verification outcome
// ---------------------------------------------------------------------------

/// Result of verifying an entire audit chain for tampering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TamperResult {
    /// Chain is fully valid — all hashes, signatures, and linkage check out.
    Clean,
    /// A block's `previous_hash` does not match the preceding block's `content_hash`.
    ChainBroken {
        sequence: u64,
        expected_hash: String,
        found_hash: String,
    },
    /// A block's Ed25519 signature is invalid.
    SignatureInvalid { sequence: u64, node_id: Uuid },
    /// Sequence numbers have gaps (missing block(s)).
    SequenceGap { missing_sequences: Vec<u64> },
    /// A block's stored `content_hash` does not match recomputed SHA-256.
    HashMismatch { sequence: u64 },
}

/// Proof that a specific event exists within a verified chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationProof {
    /// Content hash of the block containing the event.
    pub block_hash: String,
    /// Whether the full chain verified as Clean.
    pub chain_valid: bool,
    /// Number of distinct node IDs in the chain (device count).
    pub device_count: usize,
}

// ---------------------------------------------------------------------------
// AuditChain — verification methods
// ---------------------------------------------------------------------------

impl AuditChain {
    /// Walk the entire chain and return a detailed tamper result.
    ///
    /// Checks (in order for each block):
    /// 1. Sequence numbers are contiguous with no gaps
    /// 2. Recomputed SHA-256 matches stored `content_hash`
    /// 3. `previous_hash` links to the prior block's `content_hash`
    /// 4. Ed25519 signature is valid
    pub fn verify_chain(&self, verifying_key: &VerifyingKey) -> TamperResult {
        // Check for sequence gaps first (scan all blocks).
        let mut missing = Vec::new();
        for expected_seq in 0..self.chain.len() as u64 {
            match self.chain.get(expected_seq as usize) {
                Some(block) if block.sequence_number != expected_seq => {
                    missing.push(expected_seq);
                }
                None => {
                    missing.push(expected_seq);
                }
                _ => {}
            }
        }
        if !missing.is_empty() {
            return TamperResult::SequenceGap {
                missing_sequences: missing,
            };
        }

        let mut expected_previous = GENESIS_HASH.to_string();

        for block in &self.chain {
            // Hash integrity
            if !block.verify_hash() {
                return TamperResult::HashMismatch {
                    sequence: block.sequence_number,
                };
            }

            // Chain linkage
            if block.previous_hash != expected_previous {
                return TamperResult::ChainBroken {
                    sequence: block.sequence_number,
                    expected_hash: expected_previous,
                    found_hash: block.previous_hash.clone(),
                };
            }

            // Signature
            if !block.verify_signature(verifying_key) {
                return TamperResult::SignatureInvalid {
                    sequence: block.sequence_number,
                    node_id: block.node_id,
                };
            }

            expected_previous = block.content_hash.clone();
        }

        TamperResult::Clean
    }

    /// Find a specific event by its `event_id` and verify the chain surrounding it.
    ///
    /// Returns a `VerificationProof` if the event exists, or an error message.
    pub fn verify_event(
        &self,
        event_id: Uuid,
        verifying_key: &VerifyingKey,
    ) -> Result<VerificationProof, String> {
        // Scan blocks for the event
        let block = self
            .chain
            .iter()
            .find(|b| b.events.iter().any(|e| e.event_id == event_id))
            .ok_or_else(|| format!("event {event_id} not found in chain"))?;

        let chain_result = self.verify_chain(verifying_key);
        let chain_valid = chain_result == TamperResult::Clean;

        // Count distinct node IDs
        let mut node_ids = std::collections::HashSet::new();
        for b in &self.chain {
            node_ids.insert(b.node_id);
        }

        Ok(VerificationProof {
            block_hash: block.content_hash.clone(),
            chain_valid,
            device_count: node_ids.len(),
        })
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
    use nexus_kernel::audit::{AuditTrail, EventType};
    use serde_json::json;

    fn test_keypair() -> (SigningKey, VerifyingKey) {
        let seed = Sha256::digest(b"nexus-immutable-audit-test-key");
        let mut seed_bytes = [0u8; 32];
        seed_bytes.copy_from_slice(&seed);
        let signing_key = SigningKey::from_bytes(&seed_bytes);
        let verifying_key = signing_key.verifying_key();
        (signing_key, verifying_key)
    }

    fn make_events(count: usize) -> Vec<AuditEvent> {
        let mut trail = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        for i in 0..count {
            trail
                .append_event(agent_id, EventType::StateChange, json!({"seq": i}))
                .expect("audit: fail-closed");
        }
        trail.events().to_vec()
    }

    // -----------------------------------------------------------------------
    // AuditBlock hash computation
    // -----------------------------------------------------------------------

    #[test]
    fn compute_hash_deterministic() {
        let events = make_events(2);
        let node_id = Uuid::new_v4();
        let h1 = AuditBlock::compute_hash(&events, GENESIS_HASH, node_id, 1000, 0);
        let h2 = AuditBlock::compute_hash(&events, GENESIS_HASH, node_id, 1000, 0);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn compute_hash_changes_with_different_events() {
        let events_a = make_events(1);
        let events_b = make_events(2);
        let node_id = Uuid::new_v4();
        let h1 = AuditBlock::compute_hash(&events_a, GENESIS_HASH, node_id, 1000, 0);
        let h2 = AuditBlock::compute_hash(&events_b, GENESIS_HASH, node_id, 1000, 0);
        assert_ne!(h1, h2);
    }

    #[test]
    fn compute_hash_changes_with_different_previous() {
        let events = make_events(1);
        let node_id = Uuid::new_v4();
        let h1 = AuditBlock::compute_hash(&events, GENESIS_HASH, node_id, 1000, 0);
        let h2 = AuditBlock::compute_hash(&events, "abcd1234", node_id, 1000, 0);
        assert_ne!(h1, h2);
    }

    #[test]
    fn compute_hash_changes_with_different_sequence() {
        let events = make_events(1);
        let node_id = Uuid::new_v4();
        let h1 = AuditBlock::compute_hash(&events, GENESIS_HASH, node_id, 1000, 0);
        let h2 = AuditBlock::compute_hash(&events, GENESIS_HASH, node_id, 1000, 1);
        assert_ne!(h1, h2);
    }

    #[test]
    fn block_verify_hash_valid() {
        let (sk, _vk) = test_keypair();
        let events = make_events(3);
        let node_id = Uuid::new_v4();
        let content_hash = AuditBlock::compute_hash(&events, GENESIS_HASH, node_id, 1000, 0);
        let signature = sk.sign(content_hash.as_bytes());

        let block = AuditBlock {
            content_hash,
            previous_hash: GENESIS_HASH.to_string(),
            events,
            node_id,
            timestamp: 1000,
            sequence_number: 0,
            signature: signature.to_bytes().to_vec(),
        };

        assert!(block.verify_hash());
    }

    #[test]
    fn block_verify_hash_detects_tamper() {
        let (sk, _vk) = test_keypair();
        let events = make_events(2);
        let node_id = Uuid::new_v4();
        let content_hash = AuditBlock::compute_hash(&events, GENESIS_HASH, node_id, 1000, 0);
        let signature = sk.sign(content_hash.as_bytes());

        let mut block = AuditBlock {
            content_hash,
            previous_hash: GENESIS_HASH.to_string(),
            events,
            node_id,
            timestamp: 1000,
            sequence_number: 0,
            signature: signature.to_bytes().to_vec(),
        };

        // Tamper with the timestamp
        block.timestamp = 9999;
        assert!(!block.verify_hash());
    }

    #[test]
    fn block_verify_signature_valid() {
        let (sk, vk) = test_keypair();
        let events = make_events(1);
        let node_id = Uuid::new_v4();
        let content_hash = AuditBlock::compute_hash(&events, GENESIS_HASH, node_id, 1000, 0);
        let signature = sk.sign(content_hash.as_bytes());

        let block = AuditBlock {
            content_hash,
            previous_hash: GENESIS_HASH.to_string(),
            events,
            node_id,
            timestamp: 1000,
            sequence_number: 0,
            signature: signature.to_bytes().to_vec(),
        };

        assert!(block.verify_signature(&vk));
    }

    #[test]
    fn block_verify_signature_wrong_key_rejected() {
        let (sk, _vk) = test_keypair();
        let events = make_events(1);
        let node_id = Uuid::new_v4();
        let content_hash = AuditBlock::compute_hash(&events, GENESIS_HASH, node_id, 1000, 0);
        let signature = sk.sign(content_hash.as_bytes());

        let block = AuditBlock {
            content_hash,
            previous_hash: GENESIS_HASH.to_string(),
            events,
            node_id,
            timestamp: 1000,
            sequence_number: 0,
            signature: signature.to_bytes().to_vec(),
        };

        // Different key
        let other_seed = Sha256::digest(b"other-key");
        let mut other_bytes = [0u8; 32];
        other_bytes.copy_from_slice(&other_seed);
        let other_vk = SigningKey::from_bytes(&other_bytes).verifying_key();

        assert!(!block.verify_signature(&other_vk));
    }

    // -----------------------------------------------------------------------
    // ContentAddressedStore
    // -----------------------------------------------------------------------

    #[test]
    fn content_addressed_store_insert_and_lookup() {
        let mut store = ContentAddressedStore::new();
        assert!(store.is_empty());

        let (sk, _vk) = test_keypair();
        let events = make_events(2);
        let node_id = Uuid::new_v4();
        let content_hash = AuditBlock::compute_hash(&events, GENESIS_HASH, node_id, 1000, 0);
        let signature = sk.sign(content_hash.as_bytes());

        let block = AuditBlock {
            content_hash: content_hash.clone(),
            previous_hash: GENESIS_HASH.to_string(),
            events,
            node_id,
            timestamp: 1000,
            sequence_number: 0,
            signature: signature.to_bytes().to_vec(),
        };

        assert!(store.insert(block));
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());
        assert!(store.contains(&content_hash));

        let retrieved = store.get(&content_hash).unwrap();
        assert_eq!(retrieved.sequence_number, 0);
        assert_eq!(retrieved.node_id, node_id);
    }

    #[test]
    fn content_addressed_store_rejects_duplicate() {
        let mut store = ContentAddressedStore::new();
        let (sk, _vk) = test_keypair();
        let events = make_events(1);
        let node_id = Uuid::new_v4();
        let content_hash = AuditBlock::compute_hash(&events, GENESIS_HASH, node_id, 1000, 0);
        let signature = sk.sign(content_hash.as_bytes());

        let block = AuditBlock {
            content_hash,
            previous_hash: GENESIS_HASH.to_string(),
            events,
            node_id,
            timestamp: 1000,
            sequence_number: 0,
            signature: signature.to_bytes().to_vec(),
        };

        assert!(store.insert(block.clone()));
        assert!(!store.insert(block));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn content_addressed_store_lookup_missing() {
        let store = ContentAddressedStore::new();
        assert!(store.get("nonexistent").is_none());
        assert!(!store.contains("nonexistent"));
    }

    // -----------------------------------------------------------------------
    // AuditChain
    // -----------------------------------------------------------------------

    #[test]
    fn chain_append_and_lookup() {
        let (sk, vk) = test_keypair();
        let node_id = Uuid::new_v4();
        let mut chain = AuditChain::new(node_id);

        assert_eq!(chain.chain_length(), 0);
        assert!(chain.latest_block().is_none());

        let events1 = make_events(2);
        chain.append_block(events1, &sk);

        assert_eq!(chain.chain_length(), 1);
        let b1 = chain.get_block_by_sequence(0).unwrap();
        let block1_hash = b1.content_hash.clone();
        assert_eq!(b1.sequence_number, 0);
        assert_eq!(b1.previous_hash, GENESIS_HASH);
        assert_eq!(b1.events.len(), 2);

        let events2 = make_events(3);
        chain.append_block(events2, &sk);

        assert_eq!(chain.chain_length(), 2);
        let b2 = chain.get_block_by_sequence(1).unwrap();
        let block2_hash = b2.content_hash.clone();
        assert_eq!(b2.sequence_number, 1);
        assert_eq!(b2.previous_hash, block1_hash);
        assert_eq!(b2.events.len(), 3);

        // Lookup by hash
        assert!(chain.get_block_by_hash(&block1_hash).is_some());
        assert!(chain.get_block_by_hash(&block2_hash).is_some());
        assert!(chain.get_block_by_hash("nonexistent").is_none());

        // Lookup by sequence
        assert_eq!(
            chain.get_block_by_sequence(0).unwrap().content_hash,
            block1_hash
        );
        assert_eq!(
            chain.get_block_by_sequence(1).unwrap().content_hash,
            block2_hash
        );
        assert!(chain.get_block_by_sequence(2).is_none());

        // Latest
        assert_eq!(chain.latest_block().unwrap().content_hash, block2_hash);

        // Integrity
        assert!(chain.verify_integrity(&vk));
    }

    #[test]
    fn chain_preserves_original_event_uuids() {
        let (sk, _vk) = test_keypair();
        let node_id = Uuid::new_v4();
        let mut chain = AuditChain::new(node_id);

        let events = make_events(3);
        let original_ids: Vec<Uuid> = events.iter().map(|e| e.event_id).collect();

        chain.append_block(events, &sk);

        let block = chain.get_block_by_sequence(0).unwrap();
        let stored_ids: Vec<Uuid> = block.events.iter().map(|e| e.event_id).collect();
        assert_eq!(original_ids, stored_ids);
    }

    #[test]
    fn chain_integrity_detects_tampered_block() {
        let (sk, vk) = test_keypair();
        let node_id = Uuid::new_v4();
        let mut chain = AuditChain::new(node_id);

        chain.append_block(make_events(1), &sk);
        chain.append_block(make_events(1), &sk);

        assert!(chain.verify_integrity(&vk));

        // Tamper with block 0's timestamp
        chain.chain[0].timestamp = 9999;
        assert!(!chain.verify_integrity(&vk));
    }

    #[test]
    fn chain_integrity_detects_wrong_key() {
        let (sk, _vk) = test_keypair();
        let node_id = Uuid::new_v4();
        let mut chain = AuditChain::new(node_id);

        chain.append_block(make_events(1), &sk);

        let other_seed = Sha256::digest(b"wrong-key");
        let mut other_bytes = [0u8; 32];
        other_bytes.copy_from_slice(&other_seed);
        let wrong_vk = SigningKey::from_bytes(&other_bytes).verifying_key();

        assert!(!chain.verify_integrity(&wrong_vk));
    }

    #[test]
    fn chain_node_id() {
        let node_id = Uuid::new_v4();
        let chain = AuditChain::new(node_id);
        assert_eq!(chain.node_id(), node_id);
    }

    // -----------------------------------------------------------------------
    // FileAuditStore — persistence
    // -----------------------------------------------------------------------

    #[test]
    fn file_store_save_and_reload() {
        let dir = std::env::temp_dir()
            .join(format!(
                "nexus_immutable_audit_tests_{}",
                std::process::id()
            ))
            .join("save_reload");
        let _ = fs::remove_dir_all(&dir);

        let (sk, _vk) = test_keypair();
        let events = make_events(3);
        let node_id = Uuid::new_v4();
        let content_hash = AuditBlock::compute_hash(&events, GENESIS_HASH, node_id, 1000, 0);
        let signature = sk.sign(content_hash.as_bytes());

        let block = AuditBlock {
            content_hash: content_hash.clone(),
            previous_hash: GENESIS_HASH.to_string(),
            events,
            node_id,
            timestamp: 1000,
            sequence_number: 0,
            signature: signature.to_bytes().to_vec(),
        };

        // Save
        {
            let mut store = FileAuditStore::open(&dir).unwrap();
            assert!(store.insert(block).unwrap());
            assert_eq!(store.len(), 1);
        }

        // Reload from disk
        {
            let store = FileAuditStore::open(&dir).unwrap();
            assert_eq!(store.len(), 1);
            let loaded = store.get(&content_hash).unwrap();
            assert_eq!(loaded.node_id, node_id);
            assert_eq!(loaded.sequence_number, 0);
            assert_eq!(loaded.events.len(), 3);
            assert!(loaded.verify_hash());
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_store_multiple_blocks() {
        let dir = std::env::temp_dir()
            .join(format!(
                "nexus_immutable_audit_tests_{}",
                std::process::id()
            ))
            .join("multi_blocks");
        let _ = fs::remove_dir_all(&dir);

        let (sk, _vk) = test_keypair();
        let node_id = Uuid::new_v4();

        let mut hashes = Vec::new();

        {
            let mut store = FileAuditStore::open(&dir).unwrap();
            for seq in 0..3u64 {
                let prev = hashes
                    .last()
                    .cloned()
                    .unwrap_or_else(|| GENESIS_HASH.to_string());
                let events = make_events(1);
                let content_hash =
                    AuditBlock::compute_hash(&events, &prev, node_id, 1000 + seq, seq);
                let signature = sk.sign(content_hash.as_bytes());

                let block = AuditBlock {
                    content_hash: content_hash.clone(),
                    previous_hash: prev,
                    events,
                    node_id,
                    timestamp: 1000 + seq,
                    sequence_number: seq,
                    signature: signature.to_bytes().to_vec(),
                };

                store.insert(block).unwrap();
                hashes.push(content_hash);
            }
            assert_eq!(store.len(), 3);
        }

        // Reload
        let store = FileAuditStore::open(&dir).unwrap();
        assert_eq!(store.len(), 3);
        for hash in &hashes {
            assert!(store.get(hash).is_some());
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_store_rejects_duplicate() {
        let dir = std::env::temp_dir()
            .join(format!(
                "nexus_immutable_audit_tests_{}",
                std::process::id()
            ))
            .join("dup");
        let _ = fs::remove_dir_all(&dir);

        let (sk, _vk) = test_keypair();
        let events = make_events(1);
        let node_id = Uuid::new_v4();
        let content_hash = AuditBlock::compute_hash(&events, GENESIS_HASH, node_id, 1000, 0);
        let signature = sk.sign(content_hash.as_bytes());

        let block = AuditBlock {
            content_hash,
            previous_hash: GENESIS_HASH.to_string(),
            events,
            node_id,
            timestamp: 1000,
            sequence_number: 0,
            signature: signature.to_bytes().to_vec(),
        };

        let mut store = FileAuditStore::open(&dir).unwrap();
        assert!(store.insert(block.clone()).unwrap());
        assert!(!store.insert(block).unwrap());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_store_empty_dir() {
        let dir = std::env::temp_dir()
            .join(format!(
                "nexus_immutable_audit_tests_{}",
                std::process::id()
            ))
            .join("empty");
        let _ = fs::remove_dir_all(&dir);

        let store = FileAuditStore::open(&dir).unwrap();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        let _ = fs::remove_dir_all(&dir);
    }

    // -----------------------------------------------------------------------
    // Verification & tamper detection
    // -----------------------------------------------------------------------

    #[test]
    fn verify_chain_valid_returns_clean() {
        let (sk, vk) = test_keypair();
        let node_id = Uuid::new_v4();
        let mut chain = AuditChain::new(node_id);

        chain.append_block(make_events(2), &sk);
        chain.append_block(make_events(3), &sk);
        chain.append_block(make_events(1), &sk);

        assert_eq!(chain.verify_chain(&vk), TamperResult::Clean);
    }

    #[test]
    fn verify_chain_detects_hash_mismatch() {
        let (sk, vk) = test_keypair();
        let node_id = Uuid::new_v4();
        let mut chain = AuditChain::new(node_id);

        chain.append_block(make_events(1), &sk);
        chain.append_block(make_events(1), &sk);

        // Tamper with block 0's timestamp — hash no longer matches
        chain.chain[0].timestamp = 9999;

        assert_eq!(
            chain.verify_chain(&vk),
            TamperResult::HashMismatch { sequence: 0 }
        );
    }

    #[test]
    fn verify_chain_detects_broken_link() {
        let (sk, vk) = test_keypair();
        let node_id = Uuid::new_v4();
        let mut chain = AuditChain::new(node_id);

        chain.append_block(make_events(1), &sk);
        chain.append_block(make_events(1), &sk);

        // Corrupt block 1's previous_hash but recompute its content_hash
        // so hash check passes but linkage fails.
        let fake_prev = "aaaa".repeat(16);
        let new_hash = AuditBlock::compute_hash(
            &chain.chain[1].events,
            &fake_prev,
            chain.chain[1].node_id,
            chain.chain[1].timestamp,
            chain.chain[1].sequence_number,
        );
        let new_sig = sk.sign(new_hash.as_bytes());
        chain.chain[1].previous_hash = fake_prev.clone();
        chain.chain[1].content_hash = new_hash;
        chain.chain[1].signature = new_sig.to_bytes().to_vec();

        let result = chain.verify_chain(&vk);
        match result {
            TamperResult::ChainBroken {
                sequence,
                found_hash,
                ..
            } => {
                assert_eq!(sequence, 1);
                assert_eq!(found_hash, fake_prev);
            }
            other => panic!("expected ChainBroken, got {other:?}"),
        }
    }

    #[test]
    fn verify_chain_detects_invalid_signature() {
        let (sk, vk) = test_keypair();
        let node_id = Uuid::new_v4();
        let mut chain = AuditChain::new(node_id);

        chain.append_block(make_events(1), &sk);
        chain.append_block(make_events(1), &sk);

        // Replace block 1's signature with one from a different key
        let other_seed = Sha256::digest(b"attacker-key");
        let mut other_bytes = [0u8; 32];
        other_bytes.copy_from_slice(&other_seed);
        let attacker_sk = SigningKey::from_bytes(&other_bytes);
        let bad_sig = attacker_sk.sign(chain.chain[1].content_hash.as_bytes());
        chain.chain[1].signature = bad_sig.to_bytes().to_vec();

        assert_eq!(
            chain.verify_chain(&vk),
            TamperResult::SignatureInvalid {
                sequence: 1,
                node_id,
            }
        );
    }

    #[test]
    fn verify_chain_detects_sequence_gap() {
        let (sk, vk) = test_keypair();
        let node_id = Uuid::new_v4();
        let mut chain = AuditChain::new(node_id);

        chain.append_block(make_events(1), &sk);
        chain.append_block(make_events(1), &sk);
        chain.append_block(make_events(1), &sk);

        // Manually set block 1's sequence to 5 (creating a gap)
        chain.chain[1].sequence_number = 5;

        match chain.verify_chain(&vk) {
            TamperResult::SequenceGap {
                missing_sequences, ..
            } => {
                assert!(missing_sequences.contains(&1));
            }
            other => panic!("expected SequenceGap, got {other:?}"),
        }
    }

    #[test]
    fn verify_event_succeeds_for_existing_event() {
        let (sk, vk) = test_keypair();
        let node_id = Uuid::new_v4();
        let mut chain = AuditChain::new(node_id);

        let events = make_events(3);
        let target_event_id = events[1].event_id;
        chain.append_block(events, &sk);
        chain.append_block(make_events(2), &sk);

        let proof = chain.verify_event(target_event_id, &vk).unwrap();
        assert!(proof.chain_valid);
        assert_eq!(proof.device_count, 1);
        // The block hash should match block 0 (where the event lives)
        assert_eq!(
            proof.block_hash,
            chain.get_block_by_sequence(0).unwrap().content_hash
        );
    }

    #[test]
    fn verify_event_not_found_returns_error() {
        let (sk, vk) = test_keypair();
        let node_id = Uuid::new_v4();
        let mut chain = AuditChain::new(node_id);

        chain.append_block(make_events(1), &sk);

        let missing_id = Uuid::new_v4();
        let result = chain.verify_event(missing_id, &vk);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }
}
