//! Cross-device verification engine for distributed immutable audit.
//!
//! Verifies that audit events exist across multiple devices, cross-checks
//! chain integrity with peers, and generates SOC2-ready compliance reports.

use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::gossip::TamperAlertPayload;
use crate::immutable_audit::{AuditChain, TamperResult};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Per-device verification result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceVerification {
    /// The node ID of the device.
    pub node_id: Uuid,
    /// Whether the device has the block containing the event.
    pub has_block: bool,
    /// Whether the block's content_hash matches the expected hash.
    pub hash_matches: bool,
}

/// Cross-device proof that a specific event exists across multiple devices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossDeviceProof {
    /// The event being verified.
    pub event_id: Uuid,
    /// Content hash of the block containing the event.
    pub block_hash: String,
    /// Whether the local chain verified as clean.
    pub chain_valid: bool,
    /// Number of devices that verified the event.
    pub devices_verified: usize,
    /// Total number of devices queried.
    pub devices_total: usize,
    /// Per-device verification details.
    pub device_details: Vec<DeviceVerification>,
}

/// Result of cross-device chain integrity check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainIntegrityResult {
    /// Local chain tamper result.
    pub local_result: String,
    /// Whether the local chain is clean.
    pub local_clean: bool,
    /// Number of peers with matching latest hash.
    pub peers_matching: usize,
    /// Total peers checked.
    pub peers_total: usize,
    /// Peers with mismatched latest hash.
    pub mismatches: Vec<PeerMismatch>,
}

/// A peer whose latest hash differs from local.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerMismatch {
    pub node_id: Uuid,
    pub local_hash: String,
    pub peer_hash: String,
}

/// SOC2-ready compliance report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    /// Total blocks in the local chain.
    pub chain_length: u64,
    /// Total audit events across all blocks.
    pub event_count: u64,
    /// Number of distinct devices contributing to the chain.
    pub device_count: usize,
    /// Timestamp of the latest block (0 if empty).
    pub last_block_time: u64,
    /// Number of tamper incidents detected.
    pub tamper_incidents: usize,
    /// Fraction of chain verified across devices (0.0-1.0).
    pub verification_coverage: f64,
    /// Whether the local chain is fully valid.
    pub chain_integrity: bool,
    /// Structured JSON report for audit export.
    pub report_json: Value,
}

// ---------------------------------------------------------------------------
// VerificationEngine
// ---------------------------------------------------------------------------

/// Engine for cross-device audit verification and compliance reporting.
///
/// Operates on a local `AuditChain` and a set of peer chains (representing
/// paired devices). In production, peer chains are queried via gossip; in
/// tests, they are provided directly.
pub struct VerificationEngine {
    local_node_id: Uuid,
    local_chain: AuditChain,
    verifying_key: VerifyingKey,
    /// Peer chains indexed by node_id (populated via gossip or test injection).
    peer_chains: Vec<(Uuid, AuditChain)>,
    /// Tamper alerts accumulated from gossip.
    tamper_alerts: Vec<TamperAlertPayload>,
}

impl VerificationEngine {
    /// Create a new VerificationEngine.
    pub fn new(
        local_node_id: Uuid,
        local_chain: AuditChain,
        verifying_key: VerifyingKey,
    ) -> Self {
        Self {
            local_node_id,
            local_chain,
            verifying_key,
            peer_chains: Vec::new(),
            tamper_alerts: Vec::new(),
        }
    }

    /// Register a peer chain for cross-device verification.
    pub fn add_peer_chain(&mut self, node_id: Uuid, chain: AuditChain) {
        self.peer_chains.push((node_id, chain));
    }

    /// Record a tamper alert (e.g. from gossip).
    pub fn add_tamper_alert(&mut self, alert: TamperAlertPayload) {
        self.tamper_alerts.push(alert);
    }

    /// Access the local chain.
    pub fn local_chain(&self) -> &AuditChain {
        &self.local_chain
    }

    /// Verify that a specific event exists across all known devices.
    ///
    /// Finds the block containing the event in the local chain, then checks
    /// each peer chain for the same block with matching content_hash.
    pub fn verify_event_across_devices(
        &self,
        event_id: Uuid,
    ) -> Result<CrossDeviceProof, String> {
        // Find event in local chain
        let local_block = self
            .local_chain
            .chain
            .iter()
            .find(|b| b.events.iter().any(|e| e.event_id == event_id))
            .ok_or_else(|| format!("event {event_id} not found in local chain"))?;

        let block_hash = local_block.content_hash.clone();
        let block_seq = local_block.sequence_number;

        // Verify local chain
        let local_tamper = self.local_chain.verify_chain(&self.verifying_key);
        let chain_valid = local_tamper == TamperResult::Clean;

        // Check each peer
        let mut device_details = Vec::new();

        // Local device always counts
        device_details.push(DeviceVerification {
            node_id: self.local_node_id,
            has_block: true,
            hash_matches: true,
        });

        for (peer_id, peer_chain) in &self.peer_chains {
            let (has_block, hash_matches) =
                match peer_chain.get_block_by_sequence(block_seq) {
                    Some(peer_block) => (true, peer_block.content_hash == block_hash),
                    None => (false, false),
                };

            device_details.push(DeviceVerification {
                node_id: *peer_id,
                has_block,
                hash_matches,
            });
        }

        let devices_verified = device_details
            .iter()
            .filter(|d| d.has_block && d.hash_matches)
            .count();
        let devices_total = device_details.len();

        Ok(CrossDeviceProof {
            event_id,
            block_hash,
            chain_valid,
            devices_verified,
            devices_total,
            device_details,
        })
    }

    /// Verify local chain integrity and cross-check latest hash with peers.
    pub fn verify_full_chain_integrity(&self) -> ChainIntegrityResult {
        let local_tamper = self.local_chain.verify_chain(&self.verifying_key);
        let local_clean = local_tamper == TamperResult::Clean;

        let local_result = match &local_tamper {
            TamperResult::Clean => "Clean".to_string(),
            TamperResult::ChainBroken { sequence, .. } => {
                format!("ChainBroken at sequence {sequence}")
            }
            TamperResult::SignatureInvalid { sequence, .. } => {
                format!("SignatureInvalid at sequence {sequence}")
            }
            TamperResult::SequenceGap { missing_sequences } => {
                format!("SequenceGap: missing {missing_sequences:?}")
            }
            TamperResult::HashMismatch { sequence } => {
                format!("HashMismatch at sequence {sequence}")
            }
        };

        let local_latest = self
            .local_chain
            .latest_block()
            .map(|b| b.content_hash.clone())
            .unwrap_or_default();

        let local_seq = self
            .local_chain
            .latest_block()
            .map(|b| b.sequence_number)
            .unwrap_or(0);

        let mut peers_matching = 0;
        let mut mismatches = Vec::new();

        for (peer_id, peer_chain) in &self.peer_chains {
            // Compare at the local chain's latest sequence
            match peer_chain.get_block_by_sequence(local_seq) {
                Some(peer_block) if peer_block.content_hash == local_latest => {
                    peers_matching += 1;
                }
                Some(peer_block) => {
                    mismatches.push(PeerMismatch {
                        node_id: *peer_id,
                        local_hash: local_latest.clone(),
                        peer_hash: peer_block.content_hash.clone(),
                    });
                }
                None => {
                    // Peer doesn't have this block yet — not a mismatch, just behind
                    // Only count as mismatch if peer has *some* blocks at that sequence
                }
            }
        }

        ChainIntegrityResult {
            local_result,
            local_clean,
            peers_matching,
            peers_total: self.peer_chains.len(),
            mismatches,
        }
    }

    /// Generate a SOC2-ready compliance report.
    pub fn generate_compliance_report(&self) -> ComplianceReport {
        let chain_length = self.local_chain.chain_length() as u64;

        let event_count: u64 = self
            .local_chain
            .chain
            .iter()
            .map(|b| b.events.len() as u64)
            .sum();

        // Count distinct node IDs across local + peer chains
        let mut node_ids = std::collections::HashSet::new();
        node_ids.insert(self.local_node_id);
        for (peer_id, _) in &self.peer_chains {
            node_ids.insert(*peer_id);
        }
        let device_count = node_ids.len();

        let last_block_time = self
            .local_chain
            .latest_block()
            .map(|b| b.timestamp)
            .unwrap_or(0);

        let tamper_incidents = self.tamper_alerts.len();

        // Verification coverage: fraction of peers that have matching latest block
        let integrity = self.verify_full_chain_integrity();
        let verification_coverage = if self.peer_chains.is_empty() {
            if integrity.local_clean {
                1.0
            } else {
                0.0
            }
        } else {
            integrity.peers_matching as f64 / self.peer_chains.len() as f64
        };

        let chain_integrity = integrity.local_clean;

        let report_json = serde_json::json!({
            "report_type": "SOC2_audit_evidence",
            "chain_length": chain_length,
            "event_count": event_count,
            "device_count": device_count,
            "last_block_time": last_block_time,
            "tamper_incidents": tamper_incidents,
            "verification_coverage": verification_coverage,
            "chain_integrity": chain_integrity,
            "peers_matching": integrity.peers_matching,
            "peers_total": integrity.peers_total,
            "mismatches": integrity.mismatches.len(),
        });

        ComplianceReport {
            chain_length,
            event_count,
            device_count,
            last_block_time,
            tamper_incidents,
            verification_coverage,
            chain_integrity,
            report_json,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::immutable_audit::AuditChain;
    use ed25519_dalek::SigningKey;
    use nexus_kernel::audit::{AuditTrail, EventType};
    use serde_json::json;
    use sha2::{Digest, Sha256};

    fn test_keypair() -> (SigningKey, VerifyingKey) {
        let seed = Sha256::digest(b"nexus-verification-test-key");
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
            trail.append_event(agent_id, EventType::StateChange, json!({"seq": i}));
        }
        trail.events().to_vec()
    }

    /// Build a synced set of chains: one primary with blocks, N peers with
    /// the same blocks cloned via append_verified_block.
    fn build_synced_chains(
        num_peers: usize,
        blocks: &[Vec<nexus_kernel::audit::AuditEvent>],
        sk: &SigningKey,
    ) -> (Uuid, AuditChain, Vec<(Uuid, AuditChain)>) {
        let local_id = Uuid::new_v4();
        let mut local_chain = AuditChain::new(local_id);

        for events in blocks {
            local_chain.append_block(events.clone(), sk);
        }

        let mut peers = Vec::new();
        for _ in 0..num_peers {
            let peer_id = Uuid::new_v4();
            let mut peer_chain = AuditChain::new(peer_id);
            for seq in 0..local_chain.chain_length() as u64 {
                let block = local_chain.get_block_by_sequence(seq).unwrap().clone();
                peer_chain.append_verified_block(block);
            }
            peers.push((peer_id, peer_chain));
        }

        (local_id, local_chain, peers)
    }

    #[test]
    fn event_verified_across_3_devices() {
        let (sk, vk) = test_keypair();
        let events = make_events(5);
        let target_event_id = events[2].event_id;

        let (local_id, local_chain, peers) =
            build_synced_chains(2, &[events, make_events(3)], &sk);

        let mut engine = VerificationEngine::new(local_id, local_chain, vk);
        for (peer_id, peer_chain) in peers {
            engine.add_peer_chain(peer_id, peer_chain);
        }

        let proof = engine.verify_event_across_devices(target_event_id).unwrap();

        assert_eq!(proof.event_id, target_event_id);
        assert!(proof.chain_valid);
        assert_eq!(proof.devices_verified, 3); // local + 2 peers
        assert_eq!(proof.devices_total, 3);
        assert!(!proof.block_hash.is_empty());

        // All devices verified
        for detail in &proof.device_details {
            assert!(detail.has_block);
            assert!(detail.hash_matches);
        }
    }

    #[test]
    fn event_missing_on_one_device_shows_partial() {
        let (sk, vk) = test_keypair();
        let events = make_events(3);
        let target_event_id = events[1].event_id;

        // Build local + 2 peers, all synced with block 0
        let (local_id, local_chain, mut peers) =
            build_synced_chains(2, &[events, make_events(2)], &sk);

        // Remove block 1 from the second peer (simulate it being behind)
        // We rebuild peer 2 with only block 0
        let peer2_id = peers[1].0;
        let mut short_chain = AuditChain::new(peer2_id);
        let block0 = local_chain.get_block_by_sequence(0).unwrap().clone();
        short_chain.append_verified_block(block0);
        peers[1] = (peer2_id, short_chain);

        let mut engine = VerificationEngine::new(local_id, local_chain.clone(), vk);
        for (peer_id, peer_chain) in peers {
            engine.add_peer_chain(peer_id, peer_chain);
        }

        let proof = engine.verify_event_across_devices(target_event_id).unwrap();

        // Event is in block 0 — all 3 devices have block 0
        assert_eq!(proof.devices_verified, 3);
        assert_eq!(proof.devices_total, 3);

        // Now test with an event in block 1 (only local + peer1 have it)
        // Rebuild engine with fresh chains
        let (local_id2, local_chain2, mut peers2) =
            build_synced_chains(2, &[make_events(3), make_events(2)], &sk);

        // Second peer only has block 0
        let peer2_id2 = peers2[1].0;
        let mut short_chain2 = AuditChain::new(peer2_id2);
        let b0 = local_chain2.get_block_by_sequence(0).unwrap().clone();
        short_chain2.append_verified_block(b0);
        peers2[1] = (peer2_id2, short_chain2);

        let block1_event = local_chain2.get_block_by_sequence(1).unwrap().events[0].event_id;

        let mut engine2 = VerificationEngine::new(local_id2, local_chain2, vk);
        for (pid, pc) in peers2 {
            engine2.add_peer_chain(pid, pc);
        }

        let proof2 = engine2.verify_event_across_devices(block1_event).unwrap();

        // Only local + peer1 have block 1 = 2 verified out of 3
        assert_eq!(proof2.devices_verified, 2);
        assert_eq!(proof2.devices_total, 3);

        // Check the detail: peer2 should show has_block=false
        let missing_device = proof2
            .device_details
            .iter()
            .find(|d| !d.has_block)
            .expect("should have one device without the block");
        assert!(!missing_device.hash_matches);
    }

    #[test]
    fn full_chain_integrity_cross_device() {
        let (sk, vk) = test_keypair();

        let (local_id, local_chain, peers) =
            build_synced_chains(2, &[make_events(2), make_events(3), make_events(1)], &sk);

        let mut engine = VerificationEngine::new(local_id, local_chain, vk);
        for (peer_id, peer_chain) in peers {
            engine.add_peer_chain(peer_id, peer_chain);
        }

        let result = engine.verify_full_chain_integrity();

        assert!(result.local_clean);
        assert_eq!(result.local_result, "Clean");
        assert_eq!(result.peers_matching, 2);
        assert_eq!(result.peers_total, 2);
        assert!(result.mismatches.is_empty());
    }

    #[test]
    fn compliance_report_has_all_required_fields() {
        let (sk, vk) = test_keypair();

        let (local_id, local_chain, peers) =
            build_synced_chains(2, &[make_events(5), make_events(3)], &sk);

        let mut engine = VerificationEngine::new(local_id, local_chain, vk);
        for (peer_id, peer_chain) in peers {
            engine.add_peer_chain(peer_id, peer_chain);
        }

        // Add a tamper alert for coverage
        engine.add_tamper_alert(TamperAlertPayload {
            sequence: 0,
            expected_hash: "aaa".to_string(),
            actual_hash: "bbb".to_string(),
        });

        let report = engine.generate_compliance_report();

        assert_eq!(report.chain_length, 2);
        assert_eq!(report.event_count, 8); // 5 + 3
        assert_eq!(report.device_count, 3); // local + 2 peers
        assert!(report.last_block_time > 0);
        assert_eq!(report.tamper_incidents, 1);
        assert!(report.chain_integrity);
        // All peers synced, so coverage = 1.0
        assert!((report.verification_coverage - 1.0).abs() < f64::EPSILON);

        // Verify JSON report has required keys
        let json = &report.report_json;
        assert_eq!(json["report_type"], "SOC2_audit_evidence");
        assert_eq!(json["chain_length"], 2);
        assert_eq!(json["event_count"], 8);
        assert_eq!(json["device_count"], 3);
        assert_eq!(json["tamper_incidents"], 1);
        assert_eq!(json["chain_integrity"], true);
        assert!(json["verification_coverage"].as_f64().unwrap() > 0.99);
        assert_eq!(json["peers_matching"], 2);
        assert_eq!(json["peers_total"], 2);
        assert_eq!(json["mismatches"], 0);
    }
}
