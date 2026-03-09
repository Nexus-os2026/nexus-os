use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::distributed::consensus::{
    ConsensusProtocol, Proposal, QuorumStatus, SingleNodeConsensus, Vote,
};
use nexus_kernel::distributed::discovery::{DiscoveryProtocol, NoOpDiscovery};
use nexus_kernel::distributed::identity::{
    LocalOnlyRegistry, NodeId, NodeIdentity, NodeRegistry, PublicKeyBytes,
};
use nexus_kernel::distributed::replication::{EventReplicator, NoOpReplicator};
use serde_json::json;
use uuid::Uuid;

fn sample_identity() -> NodeIdentity {
    NodeIdentity {
        id: NodeId("node-local-1".to_string()),
        public_key: PublicKeyBytes(vec![1, 2, 3, 4]),
        capabilities: vec!["audit.append".to_string(), "agent.run".to_string()],
    }
}

#[test]
fn test_local_registry() {
    let mut registry = LocalOnlyRegistry::new();
    let identity = sample_identity();
    registry
        .register_self(identity.clone())
        .expect("register_self should succeed");

    let peers = registry
        .discover_peers()
        .expect("discover_peers should succeed");
    assert_eq!(peers, vec![identity.clone()]);

    let verified = registry
        .verify_peer(&identity.id)
        .expect("verify_peer should succeed");
    assert!(verified);
}

#[test]
fn test_noop_replicator() {
    let mut trail = AuditTrail::new();
    let agent_id = Uuid::new_v4();
    trail
        .append_event(
            agent_id,
            EventType::StateChange,
            json!({ "event_kind": "test.replicate" }),
        )
        .expect("audit append");
    let event = trail
        .events()
        .first()
        .cloned()
        .expect("event should exist for replication test");

    let mut replicator = NoOpReplicator::new();
    let ack = replicator
        .replicate_event(&event)
        .expect("replicate_event should succeed");
    assert!(ack.accepted);
}

#[test]
fn test_single_node_consensus() {
    let mut consensus = SingleNodeConsensus::new();
    let proposal = Proposal {
        kind: "policy.update".to_string(),
        payload_hash: "abc123".to_string(),
    };

    let proposal_id = consensus.propose(proposal).expect("propose should succeed");
    let quorum = consensus
        .check_quorum(&proposal_id)
        .expect("check_quorum should succeed");
    assert_eq!(quorum, QuorumStatus::Reached);
}

#[test]
fn test_all_trait_methods_callable_without_panic() {
    let identity = sample_identity();
    let mut registry = LocalOnlyRegistry::new();
    registry
        .register_self(identity.clone())
        .expect("register should not panic");
    let _ = registry
        .discover_peers()
        .expect("discover should not panic");
    let _ = registry
        .verify_peer(&identity.id)
        .expect("verify should not panic");

    let mut trail = AuditTrail::new();
    let agent_id = Uuid::new_v4();
    trail
        .append_event(
            agent_id,
            EventType::ToolCall,
            json!({ "event_kind": "distributed.interfaces" }),
        )
        .expect("audit append");
    let event = trail.events().first().cloned().expect("event should exist");

    let mut replicator = NoOpReplicator::new();
    let _ = replicator
        .replicate_event(&event)
        .expect("replicate should not panic");
    replicator
        .receive_event(&identity.id, event)
        .expect("receive should not panic");
    let _ = replicator
        .sync_state(&identity.id)
        .expect("sync should not panic");

    let mut consensus = SingleNodeConsensus::new();
    let proposal = Proposal {
        kind: "distributed.stub".to_string(),
        payload_hash: "deadbeef".to_string(),
    };
    let proposal_id = consensus.propose(proposal).expect("propose should succeed");
    consensus
        .vote(&proposal_id, Vote::Approve)
        .expect("vote should not panic");
    let _ = consensus
        .check_quorum(&proposal_id)
        .expect("check_quorum should not panic");

    let mut discovery = NoOpDiscovery::new();
    discovery
        .announce(&identity)
        .expect("announce should not panic");
    let listed = discovery.listen().expect("listen should not panic");
    assert!(listed.is_empty());
}
