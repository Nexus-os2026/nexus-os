//! Bridge connecting PBFT consensus to the existing gossip and audit systems.
//!
//! `BftAuditBridge` wraps a `PbftConsensus` engine and translates between the
//! PBFT protocol and the gossip layer. Clusters with fewer than 4 nodes fall
//! back to gossip-only sync (no BFT overhead).

use crate::immutable_audit::AuditBlock;
use crate::pbft::{PbftConsensus, PbftError, PbftMessage, PbftOutput};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the BFT consensus layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BftConfig {
    /// Whether BFT consensus is enabled (can be disabled to use gossip-only).
    pub enabled: bool,
    /// Milliseconds before a leader is considered timed-out.
    pub leader_timeout_ms: u64,
    /// Maximum number of audit events batched into a single proposed block.
    pub max_batch_size: usize,
}

impl Default for BftConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            leader_timeout_ms: 5000,
            max_batch_size: 50,
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors from the BFT bridge layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BftError {
    /// The underlying PBFT engine returned an error.
    Pbft(PbftError),
    /// Failed to serialize or deserialize a PBFT message.
    Serialization(String),
    /// BFT consensus is not active (cluster too small or disabled).
    NotActive,
}

impl fmt::Display for BftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pbft(e) => write!(f, "pbft: {e}"),
            Self::Serialization(detail) => write!(f, "serialization: {detail}"),
            Self::NotActive => write!(f, "BFT consensus is not active"),
        }
    }
}

impl std::error::Error for BftError {}

impl From<PbftError> for BftError {
    fn from(e: PbftError) -> Self {
        Self::Pbft(e)
    }
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// Instructions returned by the bridge for the caller to execute.
#[derive(Debug, Clone)]
pub enum BridgeAction {
    /// Serialize and broadcast this payload to all peers via gossip.
    Broadcast(Vec<u8>),
    /// This node is not the leader — forward the block to the leader.
    ForwardToLeader(AuditBlock),
    /// A block has been committed by BFT consensus — apply it locally.
    CommitBlock(AuditBlock),
    /// Leader timeout detected — initiate a view change.
    InitiateViewChange,
    /// No action required.
    None,
}

// ---------------------------------------------------------------------------
// Bridge
// ---------------------------------------------------------------------------

/// Connects the PBFT consensus engine to the gossip transport and audit chain.
///
/// When `should_use_bft()` returns `false` (cluster < 4 nodes or BFT disabled),
/// the bridge does nothing and the existing gossip-only path remains active.
#[derive(Debug)]
pub struct BftAuditBridge {
    /// The underlying PBFT consensus engine.
    pub pbft: PbftConsensus,
    /// This node's identifier.
    pub node_id: String,
    /// Bridge configuration.
    pub config: BftConfig,
}

impl BftAuditBridge {
    /// Create a new bridge wrapping a PBFT engine.
    pub fn new(node_id: String, cluster_size: usize, config: BftConfig) -> Self {
        let node_public_keys = HashMap::new();
        let pbft = PbftConsensus::new(node_id.clone(), cluster_size, node_public_keys);
        Self {
            pbft,
            node_id,
            config,
        }
    }

    /// Create a bridge with pre-populated public keys for all cluster members.
    pub fn with_keys(
        node_id: String,
        cluster_size: usize,
        node_public_keys: HashMap<String, Vec<u8>>,
        config: BftConfig,
    ) -> Self {
        let pbft = PbftConsensus::new(node_id.clone(), cluster_size, node_public_keys);
        Self {
            pbft,
            node_id,
            config,
        }
    }

    /// Returns `true` if BFT consensus should be used.
    ///
    /// BFT requires at least 4 nodes (`3f + 1` where `f >= 1`) and must be
    /// enabled in config. Below 4 nodes the system falls back to gossip-only.
    pub fn should_use_bft(&self) -> bool {
        self.config.enabled && self.pbft.cluster_size >= 4
    }

    /// Propose an audit block for BFT consensus.
    ///
    /// If this node is the current PBFT leader, creates a `PrePrepare` and
    /// returns `Broadcast` so the caller can distribute it via gossip.
    /// Otherwise returns `ForwardToLeader` so the caller can relay the block.
    pub fn propose_block(&mut self, block: AuditBlock) -> Result<BridgeAction, BftError> {
        if !self.should_use_bft() {
            return Err(BftError::NotActive);
        }

        if self.pbft.is_leader(&self.node_id) {
            let msg = self.pbft.propose(block)?;
            let bytes = serialize_message(&msg)?;
            Ok(BridgeAction::Broadcast(bytes))
        } else {
            Ok(BridgeAction::ForwardToLeader(block))
        }
    }

    /// Process a PBFT message received via gossip.
    ///
    /// Deserializes the payload, feeds it to the PBFT engine, and converts
    /// the output into `BridgeAction`s. When a block is committed, an
    /// additional `Broadcast` action is returned so non-BFT peers can sync
    /// via the existing gossip announce mechanism.
    pub fn handle_gossip_message(
        &mut self,
        _from: &str,
        data: &[u8],
    ) -> Result<Vec<BridgeAction>, BftError> {
        if !self.should_use_bft() {
            return Err(BftError::NotActive);
        }

        let msg = deserialize_message(data)?;
        let output = self.pbft.handle_message(msg)?;

        let mut actions = Vec::new();
        match output {
            PbftOutput::Broadcast(reply) => {
                let bytes = serialize_message(&reply)?;
                actions.push(BridgeAction::Broadcast(bytes));
            }
            PbftOutput::BlockCommitted(block) => {
                // Return the committed block for local application
                actions.push(BridgeAction::CommitBlock(block.clone()));
                // Also broadcast a gossip announce so non-BFT peers stay synced
                let announce = serialize_message(&PbftMessage::PrePrepare {
                    view: self.pbft.view,
                    sequence: block.sequence_number,
                    digest: block.content_hash.clone(),
                    block,
                    leader_id: self.node_id.clone(),
                })?;
                actions.push(BridgeAction::Broadcast(announce));
            }
            PbftOutput::None => {
                actions.push(BridgeAction::None);
            }
        }

        Ok(actions)
    }

    /// Check whether the leader has timed out and a view change is needed.
    pub fn tick(&mut self, elapsed_ms: u64) -> BridgeAction {
        if !self.should_use_bft() {
            return BridgeAction::None;
        }

        if self
            .pbft
            .check_leader_timeout(elapsed_ms, self.config.leader_timeout_ms)
        {
            BridgeAction::InitiateViewChange
        } else {
            BridgeAction::None
        }
    }
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

fn serialize_message(msg: &PbftMessage) -> Result<Vec<u8>, BftError> {
    serde_json::to_vec(msg).map_err(|e| BftError::Serialization(e.to_string()))
}

fn deserialize_message(data: &[u8]) -> Result<PbftMessage, BftError> {
    serde_json::from_slice(data).map_err(|e| BftError::Serialization(e.to_string()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> BftConfig {
        BftConfig::default()
    }

    fn make_keys(ids: &[&str]) -> HashMap<String, Vec<u8>> {
        ids.iter()
            .map(|id| (id.to_string(), id.as_bytes().to_vec()))
            .collect()
    }

    fn make_bridge(node_id: &str, ids: &[&str]) -> BftAuditBridge {
        let keys = make_keys(ids);
        let mut bridge = BftAuditBridge::with_keys(node_id.into(), ids.len(), keys, make_config());
        bridge.pbft.signing_key = Some(node_id.as_bytes().to_vec());
        bridge
    }

    fn make_block(hash: &str) -> AuditBlock {
        AuditBlock {
            content_hash: hash.to_string(),
            previous_hash: "0".repeat(64),
            events: vec![],
            node_id: uuid::Uuid::nil(),
            timestamp: 1000,
            sequence_number: 0,
            signature: vec![],
        }
    }

    // ---------------------------------------------------------------
    // should_use_bft
    // ---------------------------------------------------------------

    #[test]
    fn bft_active_with_4_nodes() {
        let bridge = make_bridge("a", &["a", "b", "c", "d"]);
        assert!(bridge.should_use_bft());
    }

    #[test]
    fn bft_inactive_with_3_nodes() {
        let bridge = make_bridge("a", &["a", "b", "c"]);
        assert!(!bridge.should_use_bft());
    }

    #[test]
    fn bft_inactive_when_disabled() {
        let mut config = make_config();
        config.enabled = false;
        let keys = make_keys(&["a", "b", "c", "d"]);
        let bridge = BftAuditBridge::with_keys("a".into(), 4, keys, config);
        assert!(!bridge.should_use_bft());
    }

    // ---------------------------------------------------------------
    // propose_block
    // ---------------------------------------------------------------

    #[test]
    fn leader_propose_returns_broadcast() {
        // sorted: ["a","b","c","d"], view 0 → leader "a"
        let mut bridge = make_bridge("a", &["a", "b", "c", "d"]);
        let block = make_block("deadbeef");
        let action = bridge.propose_block(block).unwrap();
        assert!(matches!(action, BridgeAction::Broadcast(_)));
    }

    #[test]
    fn non_leader_propose_returns_forward() {
        let mut bridge = make_bridge("b", &["a", "b", "c", "d"]);
        let block = make_block("deadbeef");
        let action = bridge.propose_block(block).unwrap();
        assert!(matches!(action, BridgeAction::ForwardToLeader(_)));
    }

    #[test]
    fn propose_errors_when_bft_inactive() {
        let mut bridge = make_bridge("a", &["a", "b", "c"]);
        let block = make_block("deadbeef");
        let err = bridge.propose_block(block).unwrap_err();
        assert_eq!(err, BftError::NotActive);
    }

    // ---------------------------------------------------------------
    // handle_gossip_message
    // ---------------------------------------------------------------

    #[test]
    fn gossip_message_roundtrip() {
        let mut leader = make_bridge("a", &["a", "b", "c", "d"]);
        let mut replica = make_bridge("b", &["a", "b", "c", "d"]);

        // Leader proposes
        let block = make_block("deadbeef");
        let action = leader.propose_block(block).unwrap();
        let broadcast_bytes = match action {
            BridgeAction::Broadcast(bytes) => bytes,
            _ => panic!("expected Broadcast"),
        };

        // Replica receives via gossip → should get Broadcast(Prepare)
        let actions = replica
            .handle_gossip_message("a", &broadcast_bytes)
            .unwrap();
        assert!(!actions.is_empty());
        assert!(matches!(actions[0], BridgeAction::Broadcast(_)));
    }

    #[test]
    fn gossip_message_errors_on_bad_data() {
        let mut bridge = make_bridge("a", &["a", "b", "c", "d"]);
        let err = bridge
            .handle_gossip_message("b", b"not-valid-json")
            .unwrap_err();
        assert!(matches!(err, BftError::Serialization(_)));
    }

    #[test]
    fn gossip_message_errors_when_bft_inactive() {
        let mut bridge = make_bridge("a", &["a", "b", "c"]);
        let err = bridge.handle_gossip_message("b", b"{}").unwrap_err();
        assert_eq!(err, BftError::NotActive);
    }

    // ---------------------------------------------------------------
    // tick / timeout
    // ---------------------------------------------------------------

    #[test]
    fn tick_returns_none_before_timeout() {
        let mut bridge = make_bridge("b", &["a", "b", "c", "d"]);
        let action = bridge.tick(1000);
        assert!(matches!(action, BridgeAction::None));
    }

    #[test]
    fn tick_returns_view_change_after_timeout() {
        let mut bridge = make_bridge("b", &["a", "b", "c", "d"]);
        bridge.tick(3000);
        let action = bridge.tick(3000); // total 6000 > 5000
        assert!(matches!(action, BridgeAction::InitiateViewChange));
    }

    #[test]
    fn tick_returns_none_when_bft_inactive() {
        let mut bridge = make_bridge("a", &["a", "b", "c"]);
        let action = bridge.tick(999_999);
        assert!(matches!(action, BridgeAction::None));
    }

    // ---------------------------------------------------------------
    // BftConfig defaults
    // ---------------------------------------------------------------

    #[test]
    fn config_defaults() {
        let config = BftConfig::default();
        assert!(config.enabled);
        assert_eq!(config.leader_timeout_ms, 5000);
        assert_eq!(config.max_batch_size, 50);
    }

    // ---------------------------------------------------------------
    // Requested named tests (16-19)
    // ---------------------------------------------------------------

    #[test]
    fn test_bft_disabled_under_4_nodes() {
        let bridge = make_bridge("a", &["a", "b", "c"]);
        assert!(!bridge.should_use_bft());
    }

    #[test]
    fn test_bft_enabled_at_4_nodes() {
        let bridge = make_bridge("a", &["a", "b", "c", "d"]);
        assert!(bridge.should_use_bft());
    }

    #[test]
    fn test_propose_non_leader_forwards() {
        let mut bridge = make_bridge("b", &["a", "b", "c", "d"]);
        let block = make_block("fwd");
        let action = bridge.propose_block(block).unwrap();
        assert!(matches!(action, BridgeAction::ForwardToLeader(_)));
    }

    #[test]
    fn test_config_defaults() {
        let cfg = BftConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.leader_timeout_ms, 5000);
        assert_eq!(cfg.max_batch_size, 50);
    }

    // ---------------------------------------------------------------
    // Serialization roundtrip
    // ---------------------------------------------------------------

    #[test]
    fn message_serialize_deserialize() {
        let msg = PbftMessage::Prepare {
            view: 1,
            sequence: 2,
            digest: "abc".into(),
            node_id: "n1".into(),
            signature: vec![1, 2, 3],
        };
        let bytes = serialize_message(&msg).unwrap();
        let decoded = deserialize_message(&bytes).unwrap();
        match decoded {
            PbftMessage::Prepare {
                view,
                sequence,
                digest,
                node_id,
                signature,
            } => {
                assert_eq!(view, 1);
                assert_eq!(sequence, 2);
                assert_eq!(digest, "abc");
                assert_eq!(node_id, "n1");
                assert_eq!(signature, vec![1, 2, 3]);
            }
            _ => panic!("expected Prepare"),
        }
    }
}
