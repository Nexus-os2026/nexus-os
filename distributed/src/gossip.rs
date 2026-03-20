//! Gossip protocol for syncing immutable audit blocks between paired devices.
//!
//! Uses the existing `Transport` trait and `MessageKind` gossip variants.
//! Only communicates with paired devices — unpaired nodes are rejected.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::device_pairing::DevicePairingManager;
use crate::immutable_audit::{AuditBlock, AuditChain};
use crate::transport::{Message, MessageKind, Transport};

// ---------------------------------------------------------------------------
// Gossip payload types (serialized into Message.payload)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnouncePayload {
    pub latest_hash: String,
    pub sequence_number: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestBlocksPayload {
    pub from_sequence: u64,
    pub to_sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendBlocksPayload {
    pub blocks: Vec<AuditBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamperAlertPayload {
    pub sequence: u64,
    pub expected_hash: String,
    pub actual_hash: String,
}

// ---------------------------------------------------------------------------
// GossipProtocol
// ---------------------------------------------------------------------------

/// Gossip-based audit block synchronization between paired devices.
///
/// Wraps a `Transport`, an `AuditChain`, and a `DevicePairingManager`.
/// Only exchanges blocks with actively paired peers.
pub struct GossipProtocol<T: Transport> {
    transport: T,
    local_node_id: Uuid,
    chain: AuditChain,
    pairing: DevicePairingManager,
    /// Tamper alerts received during gossip rounds.
    pub tamper_alerts: Vec<TamperAlertPayload>,
}

impl<T: Transport> GossipProtocol<T> {
    pub fn new(
        transport: T,
        local_node_id: Uuid,
        chain: AuditChain,
        pairing: DevicePairingManager,
    ) -> Self {
        Self {
            transport,
            local_node_id,
            chain,
            pairing,
            tamper_alerts: Vec::new(),
        }
    }

    /// The local node ID.
    pub fn local_node_id(&self) -> Uuid {
        self.local_node_id
    }

    /// Access the local audit chain.
    pub fn chain(&self) -> &AuditChain {
        &self.chain
    }

    /// Mutable access to the local audit chain.
    pub fn chain_mut(&mut self) -> &mut AuditChain {
        &mut self.chain
    }

    /// Start a gossip round: announce our latest state to all paired peers.
    pub fn start_gossip_round(&self) -> Result<usize, String> {
        let paired = self.pairing.list_paired_devices();
        if paired.is_empty() {
            return Ok(0);
        }

        let (latest_hash, sequence_number) = match self.chain.latest_block() {
            Some(block) => (block.content_hash.clone(), block.sequence_number),
            None => (String::new(), 0),
        };

        let payload = AnnouncePayload {
            latest_hash,
            sequence_number,
        };
        let payload_bytes =
            serde_json::to_vec(&payload).map_err(|e| format!("serialize announce: {e}"))?;

        let mut sent = 0;
        for pairing in &paired {
            let msg = Message {
                from: self.local_node_id,
                to: pairing.remote_node,
                kind: MessageKind::GossipAnnounce,
                payload: payload_bytes.clone(),
            };
            if self.transport.send(msg).is_ok() {
                sent += 1;
            }
        }

        Ok(sent)
    }

    /// Handle an incoming announce from a peer.
    ///
    /// If the peer is ahead, request missing blocks.
    /// If the peer has a different hash at the same sequence, detect tamper.
    pub fn handle_announce(
        &mut self,
        from: Uuid,
        announce: &AnnouncePayload,
    ) -> Result<GossipAction, String> {
        // Reject unpaired
        if !self.pairing.is_paired(from) {
            return Err(format!("rejected unpaired node {from}"));
        }

        let local_len = self.chain.chain_length() as u64;

        // Peer is ahead — request missing blocks
        if announce.sequence_number >= local_len
            && (local_len == 0 || announce.sequence_number > local_len - 1)
            && !(announce.latest_hash.is_empty() && local_len == 0)
        {
            let req = RequestBlocksPayload {
                from_sequence: local_len,
                to_sequence: announce.sequence_number,
            };
            let payload_bytes =
                serde_json::to_vec(&req).map_err(|e| format!("serialize request: {e}"))?;

            self.transport
                .send(Message {
                    from: self.local_node_id,
                    to: from,
                    kind: MessageKind::GossipRequestBlocks,
                    payload: payload_bytes,
                })
                .map_err(|e| format!("send request: {e}"))?;

            return Ok(GossipAction::RequestedBlocks {
                from_sequence: local_len,
                to_sequence: announce.sequence_number,
            });
        }

        // Same sequence — check for hash mismatch (tamper detection)
        if announce.sequence_number < local_len && !announce.latest_hash.is_empty() {
            if let Some(local_block) = self.chain.get_block_by_sequence(announce.sequence_number) {
                if local_block.content_hash != announce.latest_hash {
                    let alert = TamperAlertPayload {
                        sequence: announce.sequence_number,
                        expected_hash: local_block.content_hash.clone(),
                        actual_hash: announce.latest_hash.clone(),
                    };

                    let payload_bytes =
                        serde_json::to_vec(&alert).map_err(|e| format!("serialize alert: {e}"))?;

                    self.transport
                        .send(Message {
                            from: self.local_node_id,
                            to: from,
                            kind: MessageKind::GossipTamperAlert,
                            payload: payload_bytes,
                        })
                        .map_err(|e| format!("send tamper alert: {e}"))?;

                    self.tamper_alerts.push(alert);
                    return Ok(GossipAction::TamperDetected {
                        sequence: announce.sequence_number,
                    });
                }
            }
        }

        Ok(GossipAction::InSync)
    }

    /// Handle a request for blocks from a peer.
    pub fn handle_request_blocks(
        &self,
        from: Uuid,
        request: &RequestBlocksPayload,
    ) -> Result<usize, String> {
        // Reject unpaired
        if !self.pairing.is_paired(from) {
            return Err(format!("rejected unpaired node {from}"));
        }

        let mut blocks = Vec::new();
        for seq in request.from_sequence..=request.to_sequence {
            if let Some(block) = self.chain.get_block_by_sequence(seq) {
                blocks.push(block.clone());
            }
        }

        let count = blocks.len();
        let payload = SendBlocksPayload { blocks };
        let payload_bytes =
            serde_json::to_vec(&payload).map_err(|e| format!("serialize blocks: {e}"))?;

        self.transport
            .send(Message {
                from: self.local_node_id,
                to: from,
                kind: MessageKind::GossipSendBlocks,
                payload: payload_bytes,
            })
            .map_err(|e| format!("send blocks: {e}"))?;

        Ok(count)
    }

    /// Handle received blocks from a peer — validate and append to local chain.
    pub fn handle_send_blocks(
        &mut self,
        from: Uuid,
        send: &SendBlocksPayload,
        verifying_key: &ed25519_dalek::VerifyingKey,
    ) -> Result<usize, String> {
        // Reject unpaired
        if !self.pairing.is_paired(from) {
            return Err(format!("rejected unpaired node {from}"));
        }

        let mut appended = 0;

        for block in &send.blocks {
            // Verify block hash integrity
            if !block.verify_hash() {
                return Err(format!(
                    "block {} failed hash verification",
                    block.sequence_number
                ));
            }

            // Verify signature
            if !block.verify_signature(verifying_key) {
                return Err(format!(
                    "block {} failed signature verification",
                    block.sequence_number
                ));
            }

            // Verify linkage: block's previous_hash must match our latest
            let expected_previous = self
                .chain
                .latest_block()
                .map(|b| b.content_hash.clone())
                .unwrap_or_else(|| {
                    "0000000000000000000000000000000000000000000000000000000000000000".to_string()
                });

            if block.previous_hash != expected_previous {
                return Err(format!(
                    "block {} linkage broken: expected previous {}, got {}",
                    block.sequence_number, expected_previous, block.previous_hash
                ));
            }

            // Append directly (block is pre-built and signed by the remote)
            self.chain.append_verified_block(block.clone());
            appended += 1;
        }

        Ok(appended)
    }

    /// Process all pending incoming messages for this node.
    pub fn process_incoming(
        &mut self,
        verifying_key: &ed25519_dalek::VerifyingKey,
    ) -> Result<Vec<GossipAction>, String> {
        let messages = self
            .transport
            .recv(self.local_node_id)
            .map_err(|e| format!("recv: {e}"))?;

        let mut actions = Vec::new();

        for msg in messages {
            match msg.kind {
                MessageKind::GossipAnnounce => {
                    let announce: AnnouncePayload = serde_json::from_slice(&msg.payload)
                        .map_err(|e| format!("parse announce: {e}"))?;
                    let action = self.handle_announce(msg.from, &announce)?;
                    actions.push(action);
                }
                MessageKind::GossipRequestBlocks => {
                    let request: RequestBlocksPayload = serde_json::from_slice(&msg.payload)
                        .map_err(|e| format!("parse request: {e}"))?;
                    self.handle_request_blocks(msg.from, &request)?;
                    actions.push(GossipAction::SentBlocks);
                }
                MessageKind::GossipSendBlocks => {
                    let send: SendBlocksPayload = serde_json::from_slice(&msg.payload)
                        .map_err(|e| format!("parse blocks: {e}"))?;
                    let count = self.handle_send_blocks(msg.from, &send, verifying_key)?;
                    actions.push(GossipAction::ReceivedBlocks { count });
                }
                MessageKind::GossipTamperAlert => {
                    let alert: TamperAlertPayload = serde_json::from_slice(&msg.payload)
                        .map_err(|e| format!("parse alert: {e}"))?;
                    self.tamper_alerts.push(alert.clone());
                    actions.push(GossipAction::TamperDetected {
                        sequence: alert.sequence,
                    });
                }
                _ => {
                    // Not a gossip message — ignore
                }
            }
        }

        Ok(actions)
    }
}

/// Result of handling a gossip message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GossipAction {
    /// Chains are in sync, nothing to do.
    InSync,
    /// Requested missing blocks from peer.
    RequestedBlocks {
        from_sequence: u64,
        to_sequence: u64,
    },
    /// Sent blocks to a peer.
    SentBlocks,
    /// Received and appended blocks from a peer.
    ReceivedBlocks { count: usize },
    /// Hash mismatch detected at a given sequence.
    TamperDetected { sequence: u64 },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_pairing::DevicePairingManager;
    use crate::immutable_audit::AuditChain;
    use crate::transport::LocalTransport;
    use ed25519_dalek::SigningKey;
    use nexus_kernel::audit::{AuditTrail, EventType};
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::path::PathBuf;

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("nexus_gossip_tests_{}", std::process::id()))
            .join(name)
    }

    fn clean_dir(dir: &PathBuf) {
        let _ = fs::remove_dir_all(dir);
    }

    fn test_keypair() -> (SigningKey, ed25519_dalek::VerifyingKey) {
        let seed = Sha256::digest(b"nexus-gossip-test-key");
        let mut seed_bytes = [0u8; 32];
        seed_bytes.copy_from_slice(&seed);
        let sk = SigningKey::from_bytes(&seed_bytes);
        let vk = sk.verifying_key();
        (sk, vk)
    }

    fn make_events(count: usize) -> Vec<nexus_kernel::audit::AuditEvent> {
        let mut trail = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        for i in 0..count {
            if let Err(e) = trail.append_event(agent_id, EventType::StateChange, json!({"seq": i})) {
                eprintln!("[WARN] audit write failed: {e}");
            }
        }
        trail.events().to_vec()
    }

    /// Create a paired setup: two gossip protocols connected via LocalTransport.
    fn make_paired_gossip(
        test_name: &str,
    ) -> (
        GossipProtocol<LocalTransport>,
        GossipProtocol<LocalTransport>,
        SigningKey,
        ed25519_dalek::VerifyingKey,
        Vec<PathBuf>,
    ) {
        let dir_a = test_dir(&format!("{test_name}_a"));
        let dir_b = test_dir(&format!("{test_name}_b"));
        clean_dir(&dir_a);
        clean_dir(&dir_b);

        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();

        let transport = LocalTransport::new();
        transport.register_node(node_a);
        transport.register_node(node_b);

        let (sk, vk) = test_keypair();

        // Set up pairing managers and pair them
        let mgr_a =
            DevicePairingManager::open(node_a, dir_a.join("device.key"), dir_a.join("pairings"))
                .unwrap();
        let mut mgr_b =
            DevicePairingManager::open(node_b, dir_b.join("device.key"), dir_b.join("pairings"))
                .unwrap();

        // B generates code, A accepts — but we need bidirectional pairing
        // A generates code for B to accept
        let code_a = mgr_a.generate_pairing_code();
        mgr_b.accept_pairing(&code_a.encode()).unwrap();

        // For simplicity: create a second manager for A that accepts B's code
        let code_b = mgr_b.generate_pairing_code();
        let mut mgr_a_mut =
            DevicePairingManager::open(node_a, dir_a.join("device.key"), dir_a.join("pairings"))
                .unwrap();
        mgr_a_mut.accept_pairing(&code_b.encode()).unwrap();

        let chain_a = AuditChain::new(node_a);
        let chain_b = AuditChain::new(node_b);

        let gossip_a = GossipProtocol::new(transport.clone(), node_a, chain_a, mgr_a_mut);
        let gossip_b = GossipProtocol::new(transport, node_b, chain_b, mgr_b);

        (gossip_a, gossip_b, sk, vk, vec![dir_a, dir_b])
    }

    #[test]
    fn two_chains_sync_correctly() {
        let (mut gossip_a, mut gossip_b, sk, vk, dirs) = make_paired_gossip("sync");

        // A has 3 blocks, B has 0
        gossip_a.chain_mut().append_block(make_events(2), &sk);
        gossip_a.chain_mut().append_block(make_events(1), &sk);
        gossip_a.chain_mut().append_block(make_events(3), &sk);
        assert_eq!(gossip_a.chain().chain_length(), 3);
        assert_eq!(gossip_b.chain().chain_length(), 0);

        // A announces to B
        gossip_a.start_gossip_round().unwrap();

        // B processes: sees announce, requests blocks 0..2
        let actions = gossip_b.process_incoming(&vk).unwrap();
        assert!(actions
            .iter()
            .any(|a| matches!(a, GossipAction::RequestedBlocks { .. })));

        // A processes: sees request, sends blocks
        let actions = gossip_a.process_incoming(&vk).unwrap();
        assert!(actions
            .iter()
            .any(|a| matches!(a, GossipAction::SentBlocks)));

        // B processes: receives blocks
        let actions = gossip_b.process_incoming(&vk).unwrap();
        assert!(actions
            .iter()
            .any(|a| matches!(a, GossipAction::ReceivedBlocks { count: 3 })));

        // B now has all 3 blocks
        assert_eq!(gossip_b.chain().chain_length(), 3);

        // Verify chains match
        assert_eq!(
            gossip_a.chain().latest_block().unwrap().content_hash,
            gossip_b.chain().latest_block().unwrap().content_hash
        );

        for dir in &dirs {
            clean_dir(dir);
        }
    }

    #[test]
    fn missing_blocks_transferred() {
        let (mut gossip_a, mut gossip_b, sk, vk, dirs) = make_paired_gossip("missing");

        // A builds 2 blocks
        gossip_a.chain_mut().append_block(make_events(1), &sk);
        gossip_a.chain_mut().append_block(make_events(1), &sk);

        // B gets the same blocks via cloning (simulating a previous sync)
        for i in 0..2u64 {
            let block = gossip_a.chain().get_block_by_sequence(i).unwrap().clone();
            gossip_b.chain_mut().append_verified_block(block);
        }

        assert_eq!(gossip_a.chain().chain_length(), 2);
        assert_eq!(gossip_b.chain().chain_length(), 2);

        // A gets a 3rd block
        gossip_a.chain_mut().append_block(make_events(2), &sk);
        assert_eq!(gossip_a.chain().chain_length(), 3);

        // A announces
        gossip_a.start_gossip_round().unwrap();

        // B sees A is ahead, requests block 2
        let actions = gossip_b.process_incoming(&vk).unwrap();
        assert!(actions.iter().any(|a| matches!(
            a,
            GossipAction::RequestedBlocks {
                from_sequence: 2,
                to_sequence: 2
            }
        )));

        // A sends the missing block
        gossip_a.process_incoming(&vk).unwrap();

        // B receives it
        let actions = gossip_b.process_incoming(&vk).unwrap();
        assert!(actions
            .iter()
            .any(|a| matches!(a, GossipAction::ReceivedBlocks { count: 1 })));

        assert_eq!(gossip_b.chain().chain_length(), 3);

        for dir in &dirs {
            clean_dir(dir);
        }
    }

    #[test]
    fn hash_mismatch_triggers_tamper_alert() {
        let (mut gossip_a, mut gossip_b, sk, _vk, dirs) = make_paired_gossip("tamper");

        // A builds 2 blocks
        gossip_a.chain_mut().append_block(make_events(1), &sk);
        gossip_a.chain_mut().append_block(make_events(1), &sk);

        // B gets the same blocks via cloning (previous sync)
        for i in 0..2u64 {
            let block = gossip_a.chain().get_block_by_sequence(i).unwrap().clone();
            gossip_b.chain_mut().append_verified_block(block);
        }

        let node_a_id = gossip_a.local_node_id();

        // A announces with a tampered hash at sequence 1
        let announce = AnnouncePayload {
            latest_hash: "tampered_hash".to_string(),
            sequence_number: 1,
        };

        let action = gossip_b.handle_announce(node_a_id, &announce).unwrap();
        assert!(matches!(
            action,
            GossipAction::TamperDetected { sequence: 1 }
        ));
        assert_eq!(gossip_b.tamper_alerts.len(), 1);
        assert_eq!(gossip_b.tamper_alerts[0].actual_hash, "tampered_hash");

        for dir in &dirs {
            clean_dir(dir);
        }
    }

    #[test]
    fn unpaired_device_rejected() {
        let dir = test_dir("unpaired");
        clean_dir(&dir);

        let node_a = Uuid::new_v4();
        let node_unknown = Uuid::new_v4();

        let transport = LocalTransport::new();
        transport.register_node(node_a);
        transport.register_node(node_unknown);

        let (_sk, _vk) = test_keypair();

        let mgr_a =
            DevicePairingManager::open(node_a, dir.join("device.key"), dir.join("pairings"))
                .unwrap();

        let chain_a = AuditChain::new(node_a);
        let mut gossip_a = GossipProtocol::new(transport, node_a, chain_a, mgr_a);

        // Unknown node tries to announce
        let announce = AnnouncePayload {
            latest_hash: "somehash".to_string(),
            sequence_number: 5,
        };
        let result = gossip_a.handle_announce(node_unknown, &announce);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("rejected unpaired"));

        // Unknown node tries to request blocks
        let request = RequestBlocksPayload {
            from_sequence: 0,
            to_sequence: 5,
        };
        let result = gossip_a.handle_request_blocks(node_unknown, &request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("rejected unpaired"));

        clean_dir(&dir);
    }
}
