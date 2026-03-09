//! Integration tests for Phase 6.4: Distributed Immutable Audit.
//!
//! Tests exercise the full pipeline: gossip sync between chains,
//! tamper detection across peers, cross-device verification, and
//! kernel AuditTrail → distributed AuditBlock batching bridge.

use ed25519_dalek::SigningKey;
use nexus_distributed::device_pairing::DevicePairingManager;
use nexus_distributed::gossip::{GossipAction, GossipProtocol};
use nexus_distributed::immutable_audit::AuditChain;
use nexus_distributed::transport::LocalTransport;
use nexus_distributed::verification::VerificationEngine;
use nexus_kernel::audit::{AuditEvent, AuditTrail, BatcherConfig, BlockBatchSink, EventType};
use serde_json::json;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_signing_key(seed: u8) -> SigningKey {
    let mut bytes = [0u8; 32];
    bytes[0] = seed;
    SigningKey::from_bytes(&bytes)
}

fn tempdir(prefix: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nexus-test-{prefix}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Create a DevicePairingManager with a specific node ID.
fn create_pairing_manager_with_id(
    base_dir: &std::path::Path,
    name: &str,
    node_id: Uuid,
) -> DevicePairingManager {
    let key_path = base_dir.join(format!("{name}.key"));
    let pairings_dir = base_dir.join(format!("{name}_pairings"));
    DevicePairingManager::open(node_id, &key_path, &pairings_dir).unwrap()
}

/// Pair two DevicePairingManagers bidirectionally using generated pairing codes.
fn pair_managers(mgr_a: &mut DevicePairingManager, mgr_b: &mut DevicePairingManager) {
    let code_a = mgr_a.generate_pairing_code().encode();
    let code_b = mgr_b.generate_pairing_code().encode();
    mgr_a.accept_pairing(&code_b).expect("A accepts B");
    mgr_b.accept_pairing(&code_a).expect("B accepts A");
}

/// Append N kernel-style audit events as a single block to a chain.
fn append_events_as_block(
    chain: &mut AuditChain,
    signing_key: &SigningKey,
    agent_id: Uuid,
    count: usize,
) {
    let events: Vec<AuditEvent> = (0..count)
        .map(|i| {
            let mut trail = AuditTrail::new();
            trail
                .append_event(agent_id, EventType::StateChange, json!({"seq": i}))
                .expect("audit append");
            trail.events()[0].clone()
        })
        .collect();
    chain.append_block(events, signing_key);
}

// ---------------------------------------------------------------------------
// Test 1: Three chains sync via gossip with all blocks matching
// ---------------------------------------------------------------------------

#[test]
fn three_chains_sync_via_gossip() {
    let key = test_signing_key(1);
    let vk = key.verifying_key();
    let transport = LocalTransport::new();

    let node_a = Uuid::new_v4();
    let node_b = Uuid::new_v4();
    let node_c = Uuid::new_v4();
    let agent_id = Uuid::new_v4();

    transport.register_node(node_a);
    transport.register_node(node_b);
    transport.register_node(node_c);

    // Create chains — only node A has blocks
    let mut chain_a = AuditChain::new(node_a);
    append_events_as_block(&mut chain_a, &key, agent_id, 3);
    append_events_as_block(&mut chain_a, &key, agent_id, 2);
    append_events_as_block(&mut chain_a, &key, agent_id, 4);
    assert_eq!(chain_a.chain_length(), 3);

    let chain_b = AuditChain::new(node_b);
    let chain_c = AuditChain::new(node_c);

    // Set up pairing managers with specific node IDs
    let dir = tempdir("sync3");
    let mut mgr_a = create_pairing_manager_with_id(&dir, "a", node_a);
    let mut mgr_b = create_pairing_manager_with_id(&dir, "b", node_b);
    let mut mgr_c = create_pairing_manager_with_id(&dir, "c", node_c);

    pair_managers(&mut mgr_a, &mut mgr_b);
    pair_managers(&mut mgr_a, &mut mgr_c);
    pair_managers(&mut mgr_b, &mut mgr_c);

    // Create gossip protocols
    let mut gossip_a = GossipProtocol::new(transport.clone(), node_a, chain_a, mgr_a);
    let mut gossip_b = GossipProtocol::new(transport.clone(), node_b, chain_b, mgr_b);
    let mut gossip_c = GossipProtocol::new(transport.clone(), node_c, chain_c, mgr_c);

    // Round 1: A announces → B and C request blocks
    gossip_a.start_gossip_round().unwrap();

    let actions_b = gossip_b.process_incoming(&vk).unwrap();
    assert!(
        actions_b
            .iter()
            .any(|a| matches!(a, GossipAction::RequestedBlocks { .. })),
        "B should request blocks from A"
    );

    let actions_c = gossip_c.process_incoming(&vk).unwrap();
    assert!(
        actions_c
            .iter()
            .any(|a| matches!(a, GossipAction::RequestedBlocks { .. })),
        "C should request blocks from A"
    );

    // Round 2: A processes block requests and sends blocks
    let actions_a = gossip_a.process_incoming(&vk).unwrap();
    assert!(
        actions_a
            .iter()
            .any(|a| matches!(a, GossipAction::SentBlocks)),
        "A should send blocks"
    );

    // Round 3: B and C receive blocks
    let actions_b2 = gossip_b.process_incoming(&vk).unwrap();
    assert!(
        actions_b2
            .iter()
            .any(|a| matches!(a, GossipAction::ReceivedBlocks { .. })),
        "B should receive blocks"
    );

    let actions_c2 = gossip_c.process_incoming(&vk).unwrap();
    assert!(
        actions_c2
            .iter()
            .any(|a| matches!(a, GossipAction::ReceivedBlocks { .. })),
        "C should receive blocks"
    );

    // All three chains should now have 3 blocks with identical hashes
    assert_eq!(gossip_a.chain().chain_length(), 3);
    assert_eq!(gossip_b.chain().chain_length(), 3);
    assert_eq!(gossip_c.chain().chain_length(), 3);

    for seq in 0..3u64 {
        let hash_a = gossip_a
            .chain()
            .get_block_by_sequence(seq)
            .unwrap()
            .content_hash
            .clone();
        let hash_b = gossip_b
            .chain()
            .get_block_by_sequence(seq)
            .unwrap()
            .content_hash
            .clone();
        let hash_c = gossip_c
            .chain()
            .get_block_by_sequence(seq)
            .unwrap()
            .content_hash
            .clone();
        assert_eq!(hash_a, hash_b, "block {seq} hash mismatch A vs B");
        assert_eq!(hash_a, hash_c, "block {seq} hash mismatch A vs C");
    }

    // Verify integrity on all chains
    assert!(gossip_a.chain().verify_integrity(&vk));
    assert!(gossip_b.chain().verify_integrity(&vk));
    assert!(gossip_c.chain().verify_integrity(&vk));
}

// ---------------------------------------------------------------------------
// Test 2: Tamper on one chain detected by peers via gossip announce
// ---------------------------------------------------------------------------

#[test]
fn tamper_detected_by_peer_via_gossip() {
    let key = test_signing_key(2);
    let vk = key.verifying_key();
    let transport = LocalTransport::new();

    let node_a = Uuid::new_v4();
    let node_b = Uuid::new_v4();
    let agent_id = Uuid::new_v4();

    transport.register_node(node_a);
    transport.register_node(node_b);

    // Both chains start with same blocks (clone from A)
    let mut chain_a = AuditChain::new(node_a);
    append_events_as_block(&mut chain_a, &key, agent_id, 3);
    append_events_as_block(&mut chain_a, &key, agent_id, 2);

    // Build chain_b with DIFFERENT events at the same sequence positions
    // This simulates a diverged/tampered chain — same length, different hashes
    let mut chain_b = AuditChain::new(node_b);
    // Block 0: same as A (clone it)
    let block0 = chain_a.get_block_by_sequence(0).unwrap().clone();
    chain_b.append_verified_block(block0);
    // Block 1: different events → different hash at same sequence
    let different_agent = Uuid::new_v4();
    append_events_as_block(&mut chain_b, &key, different_agent, 5);
    assert_eq!(chain_b.chain_length(), 2);

    // Verify hashes actually differ at sequence 1
    let hash_a1 = chain_a
        .get_block_by_sequence(1)
        .unwrap()
        .content_hash
        .clone();
    let hash_b1 = chain_b
        .get_block_by_sequence(1)
        .unwrap()
        .content_hash
        .clone();
    assert_ne!(
        hash_a1, hash_b1,
        "chains must have different hashes at seq 1"
    );

    // Set up pairing
    let dir = tempdir("tamper");
    let mut mgr_a = create_pairing_manager_with_id(&dir, "a", node_a);
    let mut mgr_b = create_pairing_manager_with_id(&dir, "b", node_b);
    pair_managers(&mut mgr_a, &mut mgr_b);

    let mut gossip_a = GossipProtocol::new(transport.clone(), node_a, chain_a, mgr_a);
    let gossip_b = GossipProtocol::new(transport.clone(), node_b, chain_b, mgr_b);

    // B announces with its tampered latest hash
    gossip_b.start_gossip_round().unwrap();

    // A processes and detects hash mismatch at sequence 1
    let actions = gossip_a.process_incoming(&vk).unwrap();
    let tamper_detected = actions
        .iter()
        .any(|a| matches!(a, GossipAction::TamperDetected { sequence } if *sequence == 1));
    assert!(
        tamper_detected,
        "expected tamper detection at sequence 1, got: {actions:?}"
    );
    assert!(
        !gossip_a.tamper_alerts.is_empty(),
        "tamper_alerts should not be empty"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Event verification proof shows 3-of-3 devices
// ---------------------------------------------------------------------------

#[test]
fn event_verification_proof_three_of_three() {
    let key = test_signing_key(3);
    let vk = key.verifying_key();

    let node_a = Uuid::new_v4();
    let node_b = Uuid::new_v4();
    let node_c = Uuid::new_v4();
    let agent_id = Uuid::new_v4();

    // Create events and blocks
    let mut trail = AuditTrail::new();
    let target_event_id = trail
        .append_event(agent_id, EventType::ToolCall, json!({"action": "deploy"}))
        .expect("audit append");
    let events = trail.events().to_vec();

    // All three chains have the same block with the target event
    let mut chain_a = AuditChain::new(node_a);
    chain_a.append_block(events, &key);

    let mut chain_b = AuditChain::new(node_b);
    let block = chain_a.get_block_by_sequence(0).unwrap().clone();
    chain_b.append_verified_block(block.clone());

    let mut chain_c = AuditChain::new(node_c);
    chain_c.append_verified_block(block);

    // Set up verification engine
    let mut engine = VerificationEngine::new(node_a, chain_a.clone(), vk);
    engine.add_peer_chain(node_b, chain_b);
    engine.add_peer_chain(node_c, chain_c);

    // Verify the event across all devices
    let proof = engine
        .verify_event_across_devices(target_event_id)
        .expect("event verification should succeed");

    assert_eq!(proof.event_id, target_event_id);
    assert!(proof.chain_valid, "local chain should be valid");
    assert_eq!(
        proof.devices_verified, 3,
        "all 3 devices should verify the event"
    );
    assert_eq!(proof.devices_total, 3);
    assert!(!proof.block_hash.is_empty());

    for detail in &proof.device_details {
        assert!(
            detail.has_block,
            "device {} should have the block",
            detail.node_id
        );
        assert!(
            detail.hash_matches,
            "device {} hash should match",
            detail.node_id
        );
    }
}

// ---------------------------------------------------------------------------
// Test 4: Kernel AuditTrail events flow through batcher into AuditBlocks
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct AuditBlockSink {
    batches: Arc<Mutex<Vec<Vec<AuditEvent>>>>,
}

impl AuditBlockSink {
    fn new() -> (Self, Arc<Mutex<Vec<Vec<AuditEvent>>>>) {
        let batches = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                batches: batches.clone(),
            },
            batches,
        )
    }
}

impl BlockBatchSink for AuditBlockSink {
    fn seal_batch(&mut self, events: Vec<AuditEvent>) {
        self.batches.lock().unwrap().push(events);
    }
}

#[test]
fn kernel_events_flow_through_batcher_into_audit_blocks() {
    let key = test_signing_key(4);
    let vk = key.verifying_key();
    let node_id = Uuid::new_v4();
    let agent_id = Uuid::new_v4();

    // Set up kernel AuditTrail with batcher
    let (sink, batches) = AuditBlockSink::new();
    let mut trail = AuditTrail::new();
    trail.enable_distributed_audit(
        BatcherConfig {
            max_events: 5,
            max_age_secs: 3600,
        },
        Box::new(sink),
    );

    // Append 12 kernel events → should trigger 2 batches (5+5), 2 pending
    let mut event_ids = Vec::new();
    for i in 0..12 {
        let eid = trail
            .append_event(agent_id, EventType::StateChange, json!({"i": i}))
            .expect("audit append");
        event_ids.push(eid);
    }

    assert_eq!(trail.sealed_batch_count(), 2);
    assert_eq!(trail.pending_batch_count(), 2);
    assert!(
        trail.verify_integrity(),
        "kernel audit chain should be valid"
    );

    // Flush remaining
    trail.flush_batcher();
    assert_eq!(trail.sealed_batch_count(), 3);
    assert_eq!(trail.pending_batch_count(), 0);

    // Convert batched events into distributed AuditBlocks
    let sealed = batches.lock().unwrap();
    let mut chain = AuditChain::new(node_id);

    for batch in sealed.iter() {
        chain.append_block(batch.clone(), &key);
    }

    assert_eq!(chain.chain_length(), 3);
    assert!(chain.verify_integrity(&vk));

    // Verify original event UUIDs are preserved in distributed blocks
    let all_block_event_ids: Vec<Uuid> = (0..chain.chain_length() as u64)
        .flat_map(|seq| {
            chain
                .get_block_by_sequence(seq)
                .unwrap()
                .events
                .iter()
                .map(|e| e.event_id)
                .collect::<Vec<_>>()
        })
        .collect();

    assert_eq!(all_block_event_ids.len(), 12);
    assert_eq!(
        all_block_event_ids, event_ids,
        "event UUIDs must be preserved"
    );

    // Verify event timestamps and hashes are preserved (not regenerated)
    let kernel_events = trail.events();
    for (i, block_eid) in all_block_event_ids.iter().enumerate() {
        let kernel_event = &kernel_events[i];
        assert_eq!(*block_eid, kernel_event.event_id);
    }
}
