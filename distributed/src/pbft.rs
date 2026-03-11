//! Simplified Practical Byzantine Fault Tolerance (PBFT) consensus for audit log agreement.
//!
//! Implements the core PBFT phases — PrePrepare, Prepare, Commit — plus view-change
//! for leader failover. Nodes reach agreement on `AuditBlock` ordering even when up
//! to `f = (n-1)/3` nodes are Byzantine (arbitrary faults).

use crate::immutable_audit::AuditBlock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Current phase of the PBFT state machine for a given sequence slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PbftState {
    /// No active proposal.
    Idle,
    /// Leader broadcast a PrePrepare; waiting for Prepare messages.
    PrePrepared,
    /// Collected 2f+1 Prepare messages; waiting for Commit messages.
    Prepared,
    /// Collected 2f+1 Commit messages; block is committed.
    Committed,
    /// View-change in progress — current leader suspected faulty.
    ViewChange,
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// PBFT protocol messages exchanged between nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PbftMessage {
    /// Phase 1: Leader proposes a block for a given view and sequence.
    PrePrepare {
        view: u64,
        sequence: u64,
        digest: String,
        block: AuditBlock,
        leader_id: String,
    },
    /// Phase 2: Replica confirms it has seen the PrePrepare.
    Prepare {
        view: u64,
        sequence: u64,
        digest: String,
        node_id: String,
        signature: Vec<u8>,
    },
    /// Phase 3: Replica confirms readiness to commit.
    Commit {
        view: u64,
        sequence: u64,
        digest: String,
        node_id: String,
        signature: Vec<u8>,
    },
    /// Sent when a replica suspects the leader is faulty.
    ViewChange {
        new_view: u64,
        node_id: String,
        last_committed_seq: u64,
        proof: Vec<PbftMessage>,
        signature: Vec<u8>,
    },
    /// New leader announces the next view after collecting ViewChange messages.
    NewView {
        new_view: u64,
        leader_id: String,
        view_change_proofs: Vec<PbftMessage>,
        signature: Vec<u8>,
    },
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors returned by the PBFT protocol handlers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PbftError {
    /// Only the current-view leader may propose.
    NotLeader,
    /// Message's view does not match the engine's current view.
    InvalidView,
    /// Message's sequence number is not the expected next sequence.
    InvalidSequence,
    /// Block content hash does not match the digest in the message.
    InvalidDigest,
    /// Signature verification failed.
    InvalidSignature(String),
    /// This node already voted for this (view, sequence) slot.
    DuplicateMessage,
    /// Not enough active nodes for BFT quorum.
    InsufficientNodes,
    /// Message from a node not in the cluster's public-key set.
    UnknownNode(String),
}

impl fmt::Display for PbftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotLeader => write!(f, "this node is not the leader for the current view"),
            Self::InvalidView => write!(f, "message view does not match current view"),
            Self::InvalidSequence => write!(f, "unexpected sequence number"),
            Self::InvalidDigest => write!(f, "digest does not match block content hash"),
            Self::InvalidSignature(detail) => write!(f, "invalid signature: {detail}"),
            Self::DuplicateMessage => write!(f, "duplicate message from same node"),
            Self::InsufficientNodes => write!(f, "insufficient nodes for BFT quorum"),
            Self::UnknownNode(id) => write!(f, "unknown node: {id}"),
        }
    }
}

impl std::error::Error for PbftError {}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

/// Result of handling a PBFT message — tells the caller what to do next.
#[derive(Debug, Clone)]
pub enum PbftOutput {
    /// Broadcast this message to all peers.
    Broadcast(PbftMessage),
    /// A block has been committed and can be applied.
    BlockCommitted(AuditBlock),
    /// No action needed (e.g. vote collected but quorum not yet reached).
    None,
}

// ---------------------------------------------------------------------------
// Consensus engine
// ---------------------------------------------------------------------------

/// PBFT consensus engine for a single node.
///
/// Tracks the current view, sequence counter, per-slot vote collection, and
/// the committed audit-block log.
#[derive(Debug)]
pub struct PbftConsensus {
    /// This node's identifier.
    pub node_id: String,
    /// Current view number (incremented on leader failover).
    pub view: u64,
    /// Next sequence number to assign.
    pub sequence: u64,
    /// Current PBFT phase.
    pub state: PbftState,
    /// Total number of nodes in the cluster.
    pub cluster_size: usize,
    /// Prepare votes keyed by `(view, sequence)`.
    pub prepare_messages: HashMap<(u64, u64), Vec<PbftMessage>>,
    /// Commit votes keyed by `(view, sequence)`.
    pub commit_messages: HashMap<(u64, u64), Vec<PbftMessage>>,
    /// Blocks that reached the Committed phase in sequence order.
    pub committed_log: Vec<AuditBlock>,
    /// Optional raw signing-key bytes for this node.
    pub signing_key: Option<Vec<u8>>,
    /// Public keys of all cluster members keyed by node-id.
    pub node_public_keys: HashMap<String, Vec<u8>>,
    /// Ordered log of every protocol message processed.
    pub message_log: Vec<PbftMessage>,
    /// Block currently being agreed upon (set on PrePrepare, cleared on commit).
    pending_block: Option<AuditBlock>,
    /// ViewChange votes keyed by proposed new view number.
    pub view_change_messages: HashMap<u64, Vec<PbftMessage>>,
    /// Milliseconds elapsed since last leader activity (proposal or heartbeat).
    pub leader_idle_ms: u64,
}

impl PbftConsensus {
    /// Create a new PBFT engine for `node_id` in a cluster of `cluster_size` nodes.
    pub fn new(
        node_id: String,
        cluster_size: usize,
        node_public_keys: HashMap<String, Vec<u8>>,
    ) -> Self {
        Self {
            node_id,
            view: 0,
            sequence: 0,
            state: PbftState::Idle,
            cluster_size,
            prepare_messages: HashMap::new(),
            commit_messages: HashMap::new(),
            committed_log: Vec::new(),
            signing_key: None,
            node_public_keys,
            message_log: Vec::new(),
            pending_block: None,
            view_change_messages: HashMap::new(),
            leader_idle_ms: 0,
        }
    }

    /// Maximum number of faulty nodes tolerated: `f = (n - 1) / 3`.
    pub fn fault_tolerance(&self) -> usize {
        (self.cluster_size.saturating_sub(1)) / 3
    }

    /// Minimum votes (including self) needed for quorum: `2f + 1`.
    pub fn quorum_size(&self) -> usize {
        2 * self.fault_tolerance() + 1
    }

    /// Whether `node_id` is the leader for the current view.
    pub fn is_leader(&self, node_id: &str) -> bool {
        self.leader_for_view(self.view) == node_id
    }

    /// Deterministic leader selection: sort known node-ids, pick index `view % n`.
    pub fn leader_for_view(&self, view: u64) -> String {
        let mut ids: Vec<&String> = self.node_public_keys.keys().collect();
        ids.sort();
        if ids.is_empty() {
            return self.node_id.clone();
        }
        let idx = (view as usize) % ids.len();
        ids[idx].clone()
    }

    // -------------------------------------------------------------------
    // Protocol phase handlers
    // -------------------------------------------------------------------

    /// Phase 1 (leader only): propose a block for consensus.
    ///
    /// Creates a `PrePrepare` message with the block's `content_hash` as the
    /// digest, increments the sequence counter, and transitions to `PrePrepared`.
    pub fn propose(&mut self, block: AuditBlock) -> Result<PbftMessage, PbftError> {
        if !self.is_leader(&self.node_id.clone()) {
            return Err(PbftError::NotLeader);
        }
        if self.cluster_size < 3 * self.fault_tolerance() + 1 {
            return Err(PbftError::InsufficientNodes);
        }

        let seq = self.sequence;
        self.sequence += 1;

        let digest = block.content_hash.clone();
        let msg = PbftMessage::PrePrepare {
            view: self.view,
            sequence: seq,
            digest,
            block: block.clone(),
            leader_id: self.node_id.clone(),
        };

        self.pending_block = Some(block);
        self.state = PbftState::PrePrepared;
        self.message_log.push(msg.clone());
        Ok(msg)
    }

    /// Phase 1 (replica): validate a PrePrepare and return a Prepare vote.
    pub fn handle_pre_prepare(
        &mut self,
        msg: PbftMessage,
    ) -> Result<Option<PbftMessage>, PbftError> {
        let (view, sequence, digest, block) = match &msg {
            PbftMessage::PrePrepare {
                view,
                sequence,
                digest,
                block,
                leader_id,
            } => {
                if *view != self.view {
                    return Err(PbftError::InvalidView);
                }
                if *leader_id != self.leader_for_view(*view) {
                    return Err(PbftError::NotLeader);
                }
                if !self.node_public_keys.contains_key(leader_id) {
                    return Err(PbftError::UnknownNode(leader_id.clone()));
                }
                if *digest != block.content_hash {
                    return Err(PbftError::InvalidDigest);
                }
                if *sequence != self.sequence {
                    return Err(PbftError::InvalidSequence);
                }
                (*view, *sequence, digest.clone(), block.clone())
            }
            _ => return Ok(None),
        };

        self.sequence = sequence + 1;
        self.pending_block = Some(block);
        self.state = PbftState::PrePrepared;
        self.message_log.push(msg);

        let prepare = PbftMessage::Prepare {
            view,
            sequence,
            digest: digest.clone(),
            node_id: self.node_id.clone(),
            signature: self.sign_digest(&digest),
        };
        self.message_log.push(prepare.clone());

        // Also count our own Prepare
        self.prepare_messages
            .entry((view, sequence))
            .or_default()
            .push(prepare.clone());

        Ok(Some(prepare))
    }

    /// Phase 2: collect Prepare messages. Returns a Commit when quorum reached.
    pub fn handle_prepare(&mut self, msg: PbftMessage) -> Result<Option<PbftMessage>, PbftError> {
        let (view, sequence, digest, sender_id) = match &msg {
            PbftMessage::Prepare {
                view,
                sequence,
                digest,
                node_id,
                signature,
            } => {
                if *view != self.view {
                    return Err(PbftError::InvalidView);
                }
                let pub_key = self
                    .node_public_keys
                    .get(node_id)
                    .ok_or_else(|| PbftError::UnknownNode(node_id.clone()))?;
                if !Self::verify_digest_signature(digest, signature, pub_key) {
                    return Err(PbftError::InvalidSignature(node_id.clone()));
                }
                (*view, *sequence, digest.clone(), node_id.clone())
            }
            _ => return Ok(None),
        };

        // Check for duplicate from same node
        let slot = self.prepare_messages.entry((view, sequence)).or_default();
        if slot
            .iter()
            .any(|m| matches!(m, PbftMessage::Prepare { node_id: nid, .. } if *nid == sender_id))
        {
            return Err(PbftError::DuplicateMessage);
        }

        self.message_log.push(msg.clone());
        slot.push(msg);

        // Check if we reached quorum
        let count = self
            .prepare_messages
            .get(&(view, sequence))
            .map_or(0, |v| v.len());

        if count >= self.quorum_size() && self.state == PbftState::PrePrepared {
            self.state = PbftState::Prepared;
            let commit = PbftMessage::Commit {
                view: self.view,
                sequence,
                digest: digest.clone(),
                node_id: self.node_id.clone(),
                signature: self.sign_digest(&digest),
            };
            self.message_log.push(commit.clone());

            // Count our own Commit
            self.commit_messages
                .entry((self.view, sequence))
                .or_default()
                .push(commit.clone());

            return Ok(Some(commit));
        }

        Ok(None)
    }

    /// Phase 3: collect Commit messages. Returns committed block when quorum reached.
    pub fn handle_commit(&mut self, msg: PbftMessage) -> Result<Option<AuditBlock>, PbftError> {
        let (view, sequence, sender_id) = match &msg {
            PbftMessage::Commit {
                view,
                sequence,
                digest,
                node_id,
                signature,
            } => {
                if *view != self.view {
                    return Err(PbftError::InvalidView);
                }
                let pub_key = self
                    .node_public_keys
                    .get(node_id)
                    .ok_or_else(|| PbftError::UnknownNode(node_id.clone()))?;
                if !Self::verify_digest_signature(digest, signature, pub_key) {
                    return Err(PbftError::InvalidSignature(node_id.clone()));
                }
                (*view, *sequence, node_id.clone())
            }
            _ => return Ok(None),
        };

        // Check for duplicate
        let slot = self.commit_messages.entry((view, sequence)).or_default();
        if slot
            .iter()
            .any(|m| matches!(m, PbftMessage::Commit { node_id: nid, .. } if *nid == sender_id))
        {
            return Err(PbftError::DuplicateMessage);
        }

        self.message_log.push(msg.clone());
        slot.push(msg);

        // Check if we reached quorum
        let count = self
            .commit_messages
            .get(&(view, sequence))
            .map_or(0, |v| v.len());

        if count >= self.quorum_size() && self.state == PbftState::Prepared {
            self.state = PbftState::Committed;
            if let Some(block) = self.pending_block.take() {
                self.committed_log.push(block.clone());
                return Ok(Some(block));
            }
        }

        Ok(None)
    }

    /// Top-level message router: dispatches to the appropriate phase handler.
    pub fn handle_message(&mut self, msg: PbftMessage) -> Result<PbftOutput, PbftError> {
        match &msg {
            PbftMessage::PrePrepare { .. } => match self.handle_pre_prepare(msg)? {
                Some(prepare) => Ok(PbftOutput::Broadcast(prepare)),
                None => Ok(PbftOutput::None),
            },
            PbftMessage::Prepare { .. } => match self.handle_prepare(msg)? {
                Some(commit) => Ok(PbftOutput::Broadcast(commit)),
                None => Ok(PbftOutput::None),
            },
            PbftMessage::Commit { .. } => match self.handle_commit(msg)? {
                Some(block) => Ok(PbftOutput::BlockCommitted(block)),
                None => Ok(PbftOutput::None),
            },
            PbftMessage::ViewChange { .. } => match self.handle_view_change(msg)? {
                Some(new_view) => Ok(PbftOutput::Broadcast(new_view)),
                None => Ok(PbftOutput::None),
            },
            PbftMessage::NewView { .. } => {
                self.handle_new_view(msg)?;
                Ok(PbftOutput::None)
            }
        }
    }

    // -------------------------------------------------------------------
    // View change
    // -------------------------------------------------------------------

    /// Initiate a view change when the current leader is suspected faulty.
    ///
    /// Creates a `ViewChange` message targeting `view + 1`, including the last
    /// committed sequence number and commit proofs for that sequence.
    pub fn request_view_change(&mut self) -> Result<PbftMessage, PbftError> {
        let new_view = self.view + 1;
        let last_committed_seq = if self.committed_log.is_empty() {
            0
        } else {
            self.committed_log.len() as u64 - 1
        };

        // Gather commit proofs for the last committed sequence (if any)
        let proof: Vec<PbftMessage> = self
            .commit_messages
            .get(&(self.view, last_committed_seq))
            .cloned()
            .unwrap_or_default();

        let digest = format!("view-change:{new_view}:{last_committed_seq}");
        let msg = PbftMessage::ViewChange {
            new_view,
            node_id: self.node_id.clone(),
            last_committed_seq,
            proof,
            signature: self.sign_digest(&digest),
        };

        self.state = PbftState::ViewChange;
        self.message_log.push(msg.clone());

        // Count our own ViewChange vote
        self.view_change_messages
            .entry(new_view)
            .or_default()
            .push(msg.clone());

        Ok(msg)
    }

    /// Collect ViewChange messages. When quorum is reached for a new view,
    /// the new leader creates and returns a `NewView` message.
    pub fn handle_view_change(
        &mut self,
        msg: PbftMessage,
    ) -> Result<Option<PbftMessage>, PbftError> {
        let (new_view, sender_id) = match &msg {
            PbftMessage::ViewChange {
                new_view, node_id, ..
            } => {
                if *new_view <= self.view {
                    return Err(PbftError::InvalidView);
                }
                if !self.node_public_keys.contains_key(node_id) {
                    return Err(PbftError::UnknownNode(node_id.clone()));
                }
                (*new_view, node_id.clone())
            }
            _ => return Ok(None),
        };

        // Check for duplicate from same node
        let slot = self.view_change_messages.entry(new_view).or_default();
        if slot
            .iter()
            .any(|m| matches!(m, PbftMessage::ViewChange { node_id: nid, .. } if *nid == sender_id))
        {
            return Err(PbftError::DuplicateMessage);
        }

        self.state = PbftState::ViewChange;
        self.message_log.push(msg.clone());
        slot.push(msg);

        // Check quorum
        let count = self
            .view_change_messages
            .get(&new_view)
            .map_or(0, |v| v.len());

        if count >= self.quorum_size() {
            let new_leader = self.leader_for_view(new_view);
            if new_leader == self.node_id {
                // We are the new leader — create NewView
                let proofs = self
                    .view_change_messages
                    .get(&new_view)
                    .cloned()
                    .unwrap_or_default();
                let digest = format!("new-view:{new_view}");
                let new_view_msg = PbftMessage::NewView {
                    new_view,
                    leader_id: self.node_id.clone(),
                    view_change_proofs: proofs,
                    signature: self.sign_digest(&digest),
                };

                // Apply the view change locally
                self.view = new_view;
                self.state = PbftState::Idle;
                self.pending_block = None;
                self.leader_idle_ms = 0;
                self.message_log.push(new_view_msg.clone());

                return Ok(Some(new_view_msg));
            }
        }

        Ok(None)
    }

    /// Validate and apply a `NewView` message from the new leader.
    ///
    /// Checks that the sender is the correct leader for the new view and that
    /// the message contains at least `quorum_size` ViewChange proofs all
    /// targeting the same view.
    pub fn handle_new_view(&mut self, msg: PbftMessage) -> Result<(), PbftError> {
        let (new_view, leader_id, view_change_proofs) = match &msg {
            PbftMessage::NewView {
                new_view,
                leader_id,
                view_change_proofs,
                ..
            } => (*new_view, leader_id.clone(), view_change_proofs.clone()),
            _ => return Ok(()),
        };

        // Must advance the view
        if new_view <= self.view {
            return Err(PbftError::InvalidView);
        }

        // Sender must be the correct leader for the new view
        if leader_id != self.leader_for_view(new_view) {
            return Err(PbftError::NotLeader);
        }

        // Must contain quorum_size ViewChange proofs
        if view_change_proofs.len() < self.quorum_size() {
            return Err(PbftError::InsufficientNodes);
        }

        // All proofs must be ViewChange messages targeting the same new_view
        for proof in &view_change_proofs {
            match proof {
                PbftMessage::ViewChange {
                    new_view: pv,
                    node_id,
                    ..
                } => {
                    if *pv != new_view {
                        return Err(PbftError::InvalidView);
                    }
                    if !self.node_public_keys.contains_key(node_id) {
                        return Err(PbftError::UnknownNode(node_id.clone()));
                    }
                }
                _ => return Err(PbftError::InvalidDigest),
            }
        }

        // Apply the view change
        self.view = new_view;
        self.state = PbftState::Idle;
        self.pending_block = None;
        self.leader_idle_ms = 0;
        self.message_log.push(msg);

        Ok(())
    }

    /// Returns `true` if the leader has been idle longer than `timeout_ms`,
    /// indicating a view change should be initiated.
    pub fn check_leader_timeout(&mut self, elapsed_ms: u64, timeout_ms: u64) -> bool {
        self.leader_idle_ms += elapsed_ms;
        self.leader_idle_ms >= timeout_ms && self.state != PbftState::ViewChange
    }

    /// Detect equivocation: same node sent conflicting messages for the same
    /// `(view, sequence)` slot. This is proof of Byzantine misbehavior.
    pub fn detect_equivocation(&self, msg1: &PbftMessage, msg2: &PbftMessage) -> bool {
        match (msg1, msg2) {
            (
                PbftMessage::PrePrepare {
                    view: v1,
                    sequence: s1,
                    digest: d1,
                    leader_id: l1,
                    ..
                },
                PbftMessage::PrePrepare {
                    view: v2,
                    sequence: s2,
                    digest: d2,
                    leader_id: l2,
                    ..
                },
            ) => v1 == v2 && s1 == s2 && l1 == l2 && d1 != d2,
            (
                PbftMessage::Prepare {
                    view: v1,
                    sequence: s1,
                    digest: d1,
                    node_id: n1,
                    ..
                },
                PbftMessage::Prepare {
                    view: v2,
                    sequence: s2,
                    digest: d2,
                    node_id: n2,
                    ..
                },
            ) => v1 == v2 && s1 == s2 && n1 == n2 && d1 != d2,
            (
                PbftMessage::Commit {
                    view: v1,
                    sequence: s1,
                    digest: d1,
                    node_id: n1,
                    ..
                },
                PbftMessage::Commit {
                    view: v2,
                    sequence: s2,
                    digest: d2,
                    node_id: n2,
                    ..
                },
            ) => v1 == v2 && s1 == s2 && n1 == n2 && d1 != d2,
            _ => false,
        }
    }

    // -------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------

    /// Produce a simple signature over a digest using the node's signing key.
    ///
    /// Uses SHA-256(signing_key_bytes || digest_bytes) as a deterministic
    /// signature stand-in when real Ed25519 is not wired up.
    fn sign_digest(&self, digest: &str) -> Vec<u8> {
        use sha2::{Digest as _, Sha256};
        let key_bytes = self.signing_key.as_deref().unwrap_or(b"default-key");
        let mut hasher = Sha256::new();
        hasher.update(key_bytes);
        hasher.update(digest.as_bytes());
        hasher.finalize().to_vec()
    }

    /// Verify a digest signature against a public key.
    ///
    /// Mirror of `sign_digest`: SHA-256(pub_key || digest) == signature.
    fn verify_digest_signature(digest: &str, signature: &[u8], pub_key: &[u8]) -> bool {
        use sha2::{Digest as _, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(pub_key);
        hasher.update(digest.as_bytes());
        let expected = hasher.finalize().to_vec();
        expected == signature
    }

    /// Create a signature for testing: SHA-256(node_id_bytes || digest_bytes).
    ///
    /// Matches the sign/verify scheme so tests can construct valid messages
    /// without wiring up a full `PbftConsensus` instance for every node.
    #[cfg(test)]
    fn make_sig(node_id: &str, digest: &str) -> Vec<u8> {
        use sha2::{Digest as _, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(node_id.as_bytes());
        hasher.update(digest.as_bytes());
        hasher.finalize().to_vec()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_keys(ids: &[&str]) -> HashMap<String, Vec<u8>> {
        ids.iter()
            .map(|id| (id.to_string(), vec![0u8; 32]))
            .collect()
    }

    #[test]
    fn new_starts_idle() {
        let keys = make_keys(&["a", "b", "c", "d"]);
        let pbft = PbftConsensus::new("a".into(), 4, keys);
        assert_eq!(pbft.state, PbftState::Idle);
        assert_eq!(pbft.view, 0);
        assert_eq!(pbft.sequence, 0);
        assert!(pbft.committed_log.is_empty());
    }

    #[test]
    fn fault_tolerance_4_nodes() {
        let keys = make_keys(&["a", "b", "c", "d"]);
        let pbft = PbftConsensus::new("a".into(), 4, keys);
        // f = (4-1)/3 = 1
        assert_eq!(pbft.fault_tolerance(), 1);
        // quorum = 2*1+1 = 3
        assert_eq!(pbft.quorum_size(), 3);
    }

    #[test]
    fn fault_tolerance_7_nodes() {
        let keys = make_keys(&["a", "b", "c", "d", "e", "f", "g"]);
        let pbft = PbftConsensus::new("a".into(), 7, keys);
        // f = (7-1)/3 = 2
        assert_eq!(pbft.fault_tolerance(), 2);
        // quorum = 2*2+1 = 5
        assert_eq!(pbft.quorum_size(), 5);
    }

    #[test]
    fn fault_tolerance_single_node() {
        let keys = make_keys(&["a"]);
        let pbft = PbftConsensus::new("a".into(), 1, keys);
        assert_eq!(pbft.fault_tolerance(), 0);
        assert_eq!(pbft.quorum_size(), 1);
    }

    #[test]
    fn leader_rotation_by_view() {
        let keys = make_keys(&["alice", "bob", "carol"]);
        let pbft = PbftConsensus::new("alice".into(), 3, keys);
        // Sorted: ["alice", "bob", "carol"]
        assert_eq!(pbft.leader_for_view(0), "alice");
        assert_eq!(pbft.leader_for_view(1), "bob");
        assert_eq!(pbft.leader_for_view(2), "carol");
        assert_eq!(pbft.leader_for_view(3), "alice"); // wraps
    }

    #[test]
    fn is_leader_checks_current_view() {
        let keys = make_keys(&["alice", "bob", "carol"]);
        let mut pbft = PbftConsensus::new("bob".into(), 3, keys);
        // view 0 → leader is "alice"
        assert!(!pbft.is_leader("bob"));
        assert!(pbft.is_leader("alice"));
        // view 1 → leader is "bob"
        pbft.view = 1;
        assert!(pbft.is_leader("bob"));
    }

    #[test]
    fn quorum_size_for_various_cluster_sizes() {
        // n=1 → f=0, q=1
        // n=2 → f=0, q=1
        // n=3 → f=0, q=1
        // n=4 → f=1, q=3
        // n=5 → f=1, q=3
        // n=6 → f=1, q=3
        // n=7 → f=2, q=5
        // n=10 → f=3, q=7
        let expected = [
            (1, 1),
            (2, 1),
            (3, 1),
            (4, 3),
            (5, 3),
            (6, 3),
            (7, 5),
            (10, 7),
        ];
        for (n, q) in expected {
            let keys: HashMap<String, Vec<u8>> =
                (0..n).map(|i| (format!("n{i}"), vec![0u8; 32])).collect();
            let pbft = PbftConsensus::new("n0".into(), n, keys);
            assert_eq!(
                pbft.quorum_size(),
                q,
                "cluster_size={n} expected quorum={q}"
            );
        }
    }

    #[test]
    fn message_log_starts_empty() {
        let keys = make_keys(&["a", "b", "c", "d"]);
        let pbft = PbftConsensus::new("a".into(), 4, keys);
        assert!(pbft.message_log.is_empty());
        assert!(pbft.prepare_messages.is_empty());
        assert!(pbft.commit_messages.is_empty());
    }

    // ---------------------------------------------------------------
    // setup_4_node_cluster helper + named-node tests
    // ---------------------------------------------------------------

    fn setup_4_node_cluster() -> Vec<PbftConsensus> {
        let ids = &["node_0", "node_1", "node_2", "node_3"];
        let keys: HashMap<String, Vec<u8>> = ids
            .iter()
            .map(|id| (id.to_string(), id.as_bytes().to_vec()))
            .collect();
        ids.iter()
            .map(|id| {
                let mut c = PbftConsensus::new(id.to_string(), ids.len(), keys.clone());
                c.signing_key = Some(id.as_bytes().to_vec());
                c
            })
            .collect()
    }

    #[test]
    fn test_fault_tolerance_calculation() {
        let k4 = make_keys(&["a", "b", "c", "d"]);
        assert_eq!(PbftConsensus::new("a".into(), 4, k4).fault_tolerance(), 1);
        let k7: HashMap<String, Vec<u8>> =
            (0..7).map(|i| (format!("n{i}"), vec![0u8; 32])).collect();
        assert_eq!(PbftConsensus::new("n0".into(), 7, k7).fault_tolerance(), 2);
        let k10: HashMap<String, Vec<u8>> =
            (0..10).map(|i| (format!("n{i}"), vec![0u8; 32])).collect();
        assert_eq!(
            PbftConsensus::new("n0".into(), 10, k10).fault_tolerance(),
            3
        );
    }

    #[test]
    fn test_quorum_size() {
        let k4 = make_keys(&["a", "b", "c", "d"]);
        assert_eq!(PbftConsensus::new("a".into(), 4, k4).quorum_size(), 3);
        let k7: HashMap<String, Vec<u8>> =
            (0..7).map(|i| (format!("n{i}"), vec![0u8; 32])).collect();
        assert_eq!(PbftConsensus::new("n0".into(), 7, k7).quorum_size(), 5);
        let k10: HashMap<String, Vec<u8>> =
            (0..10).map(|i| (format!("n{i}"), vec![0u8; 32])).collect();
        assert_eq!(PbftConsensus::new("n0".into(), 10, k10).quorum_size(), 7);
    }

    #[test]
    fn test_leader_rotation() {
        let cluster = setup_4_node_cluster();
        // sorted: ["node_0","node_1","node_2","node_3"]
        assert_eq!(cluster[0].leader_for_view(0), "node_0");
        assert_eq!(cluster[0].leader_for_view(1), "node_1");
        assert_eq!(cluster[0].leader_for_view(2), "node_2");
        assert_eq!(cluster[0].leader_for_view(3), "node_3");
        assert_eq!(cluster[0].leader_for_view(4), "node_0"); // wraps
    }

    #[test]
    fn test_propose_only_leader() {
        let mut cluster = setup_4_node_cluster();
        // node_0 is leader at view 0
        let block = make_block("test");
        let err = cluster[1].propose(block.clone()).unwrap_err();
        assert_eq!(err, PbftError::NotLeader);
        let err2 = cluster[2].propose(block.clone()).unwrap_err();
        assert_eq!(err2, PbftError::NotLeader);
        // leader succeeds
        assert!(cluster[0].propose(block).is_ok());
    }

    #[test]
    fn test_three_phase_commit_4_nodes() {
        let mut cluster = setup_4_node_cluster();
        let block = make_block("commit-me");
        let pp = cluster[0].propose(block).unwrap();

        // Phase 1: replicas handle PrePrepare → return Prepare
        let prep_1 = cluster[1].handle_pre_prepare(pp.clone()).unwrap().unwrap();
        let prep_2 = cluster[2].handle_pre_prepare(pp.clone()).unwrap().unwrap();
        let prep_3 = cluster[3].handle_pre_prepare(pp).unwrap().unwrap();

        // Phase 2: leader collects Prepares → returns Commit at quorum (3)
        cluster[0].handle_prepare(prep_1).unwrap();
        cluster[0].handle_prepare(prep_2).unwrap();
        let _commit_0 = cluster[0].handle_prepare(prep_3).unwrap().unwrap();
        assert_eq!(cluster[0].state, PbftState::Prepared);

        // Phase 3: leader collects Commits → block committed at quorum
        // Leader auto-counted its own commit (1). Need 2 more.
        let commit_1 = PbftMessage::Commit {
            view: 0,
            sequence: 0,
            digest: "commit-me".into(),
            node_id: "node_1".into(),
            signature: PbftConsensus::make_sig("node_1", "commit-me"),
        };
        let commit_2 = PbftMessage::Commit {
            view: 0,
            sequence: 0,
            digest: "commit-me".into(),
            node_id: "node_2".into(),
            signature: PbftConsensus::make_sig("node_2", "commit-me"),
        };
        cluster[0].handle_commit(commit_1).unwrap();
        let committed = cluster[0].handle_commit(commit_2).unwrap();
        assert!(committed.is_some());
        let committed_block = committed.unwrap();
        assert_eq!(committed_block.content_hash, "commit-me");
        assert_eq!(cluster[0].committed_log.len(), 1);
    }

    #[test]
    fn test_byzantine_node_rejected() {
        // 4 nodes, node_3 sends bad signature. 3 honest nodes still reach consensus.
        let mut cluster = setup_4_node_cluster();
        let block = make_block("honest-block");
        let pp = cluster[0].propose(block).unwrap();

        let prep_1 = cluster[1].handle_pre_prepare(pp.clone()).unwrap().unwrap();
        let prep_2 = cluster[2].handle_pre_prepare(pp.clone()).unwrap().unwrap();
        let _prep_3 = cluster[3].handle_pre_prepare(pp).unwrap().unwrap();

        // Byzantine node sends Prepare with wrong signature
        let bad_prepare = PbftMessage::Prepare {
            view: 0,
            sequence: 0,
            digest: "honest-block".into(),
            node_id: "node_3".into(),
            signature: vec![0xBA, 0xD],
        };

        cluster[0].handle_prepare(prep_1).unwrap();
        cluster[0].handle_prepare(prep_2).unwrap();
        // Bad signature rejected
        let err = cluster[0].handle_prepare(bad_prepare).unwrap_err();
        assert!(matches!(err, PbftError::InvalidSignature(_)));
        // Still at PrePrepared — only 2 valid Prepares, need 3
        assert_eq!(cluster[0].state, PbftState::PrePrepared);

        // But a valid Prepare from node_3 would complete quorum
        let good_prep_3 = PbftMessage::Prepare {
            view: 0,
            sequence: 0,
            digest: "honest-block".into(),
            node_id: "node_3".into(),
            signature: PbftConsensus::make_sig("node_3", "honest-block"),
        };
        let commit = cluster[0].handle_prepare(good_prep_3).unwrap();
        assert!(commit.is_some());
        assert_eq!(cluster[0].state, PbftState::Prepared);
    }

    #[test]
    fn test_two_byzantine_blocked() {
        // 4 nodes, 2 send bad messages. Only 2 valid Prepares < quorum (3).
        let mut cluster = setup_4_node_cluster();
        let block = make_block("contested");
        let pp = cluster[0].propose(block).unwrap();

        let prep_1 = cluster[1].handle_pre_prepare(pp.clone()).unwrap().unwrap();
        cluster[2].handle_pre_prepare(pp.clone()).unwrap();
        cluster[3].handle_pre_prepare(pp).unwrap();

        // Only node_1 sends valid Prepare. node_2 and node_3 send garbage.
        cluster[0].handle_prepare(prep_1).unwrap();

        let bad2 = PbftMessage::Prepare {
            view: 0,
            sequence: 0,
            digest: "contested".into(),
            node_id: "node_2".into(),
            signature: vec![0xFF],
        };
        let bad3 = PbftMessage::Prepare {
            view: 0,
            sequence: 0,
            digest: "contested".into(),
            node_id: "node_3".into(),
            signature: vec![0xFF],
        };
        assert!(cluster[0].handle_prepare(bad2).is_err());
        assert!(cluster[0].handle_prepare(bad3).is_err());

        // Only 1 valid Prepare, quorum=3 → still PrePrepared, no Commit
        assert_eq!(cluster[0].state, PbftState::PrePrepared);
        assert!(cluster[0].committed_log.is_empty());
    }

    #[test]
    fn test_duplicate_prepare_rejected() {
        let mut cluster = setup_4_node_cluster();
        let block = make_block("dup-test");
        let pp = cluster[0].propose(block).unwrap();

        let prep_1 = cluster[1].handle_pre_prepare(pp).unwrap().unwrap();
        cluster[0].handle_prepare(prep_1.clone()).unwrap();
        let err = cluster[0].handle_prepare(prep_1).unwrap_err();
        assert_eq!(err, PbftError::DuplicateMessage);
    }

    #[test]
    fn test_wrong_view_rejected() {
        let mut cluster = setup_4_node_cluster();
        let bad = PbftMessage::Prepare {
            view: 99,
            sequence: 0,
            digest: "x".into(),
            node_id: "node_1".into(),
            signature: vec![],
        };
        let err = cluster[0].handle_prepare(bad).unwrap_err();
        assert_eq!(err, PbftError::InvalidView);
    }

    #[test]
    fn test_view_change_on_timeout() {
        let mut cluster = setup_4_node_cluster();
        // 3 of 4 nodes request view change for view 1
        let vc_0 = cluster[0].request_view_change().unwrap();
        let vc_2 = cluster[2].request_view_change().unwrap();
        let vc_3 = cluster[3].request_view_change().unwrap();

        // node_1 is leader for view 1 (sorted: node_0..node_3, index 1)
        cluster[1].handle_view_change(vc_0).unwrap();
        cluster[1].handle_view_change(vc_2).unwrap();
        let nv = cluster[1].handle_view_change(vc_3).unwrap();
        assert!(nv.is_some());
        assert_eq!(cluster[1].view, 1);
        assert!(cluster[1].is_leader("node_1"));
    }

    #[test]
    fn test_view_change_requires_quorum() {
        let mut cluster = setup_4_node_cluster();
        // Only 1 node requests view change — not enough
        let vc_0 = cluster[0].request_view_change().unwrap();
        let result = cluster[1].handle_view_change(vc_0).unwrap();
        assert!(result.is_none());
        // node_1 is in ViewChange but no NewView produced
        assert_eq!(cluster[1].state, PbftState::ViewChange);
        assert_eq!(cluster[1].view, 0); // unchanged
    }

    #[test]
    fn test_equivocation_detected() {
        let cluster = setup_4_node_cluster();
        let pp1 = PbftMessage::PrePrepare {
            view: 0,
            sequence: 0,
            digest: "block_a".into(),
            block: make_block("block_a"),
            leader_id: "node_0".into(),
        };
        let pp2 = PbftMessage::PrePrepare {
            view: 0,
            sequence: 0,
            digest: "block_b".into(),
            block: make_block("block_b"),
            leader_id: "node_0".into(),
        };
        assert!(cluster[0].detect_equivocation(&pp1, &pp2));
    }

    #[test]
    fn test_committed_block_matches_proposed() {
        let mut cluster = setup_4_node_cluster();
        let block = AuditBlock {
            content_hash: "exact-match-hash".into(),
            previous_hash: "prev".repeat(16),
            events: vec![],
            node_id: uuid::Uuid::from_u128(42),
            timestamp: 1234567890,
            sequence_number: 7,
            signature: vec![1, 2, 3, 4, 5],
        };
        let pp = cluster[0].propose(block.clone()).unwrap();

        let prep_1 = cluster[1].handle_pre_prepare(pp.clone()).unwrap().unwrap();
        let prep_2 = cluster[2].handle_pre_prepare(pp.clone()).unwrap().unwrap();
        let prep_3 = cluster[3].handle_pre_prepare(pp).unwrap().unwrap();

        cluster[0].handle_prepare(prep_1).unwrap();
        cluster[0].handle_prepare(prep_2).unwrap();
        cluster[0].handle_prepare(prep_3).unwrap();

        let commit_1 = PbftMessage::Commit {
            view: 0,
            sequence: 0,
            digest: "exact-match-hash".into(),
            node_id: "node_1".into(),
            signature: PbftConsensus::make_sig("node_1", "exact-match-hash"),
        };
        let commit_2 = PbftMessage::Commit {
            view: 0,
            sequence: 0,
            digest: "exact-match-hash".into(),
            node_id: "node_2".into(),
            signature: PbftConsensus::make_sig("node_2", "exact-match-hash"),
        };
        cluster[0].handle_commit(commit_1).unwrap();
        let committed = cluster[0].handle_commit(commit_2).unwrap().unwrap();

        assert_eq!(committed.content_hash, block.content_hash);
        assert_eq!(committed.previous_hash, block.previous_hash);
        assert_eq!(committed.node_id, block.node_id);
        assert_eq!(committed.timestamp, block.timestamp);
        assert_eq!(committed.sequence_number, block.sequence_number);
        assert_eq!(committed.signature, block.signature);
    }

    #[test]
    fn test_insufficient_nodes() {
        // 4-node cluster but only 2 participate → cannot reach quorum (3)
        let mut cluster = setup_4_node_cluster();
        let block = make_block("partial");
        let pp = cluster[0].propose(block).unwrap();

        // Only node_1 responds
        let prep_1 = cluster[1].handle_pre_prepare(pp).unwrap().unwrap();
        let result = cluster[0].handle_prepare(prep_1).unwrap();
        // 1 Prepare < quorum(3) → no Commit returned
        assert!(result.is_none());
        assert_eq!(cluster[0].state, PbftState::PrePrepared);
    }

    #[test]
    fn test_single_node_not_bft() {
        let keys: HashMap<String, Vec<u8>> =
            [("solo".to_string(), vec![0u8; 32])].into_iter().collect();
        let pbft = PbftConsensus::new("solo".into(), 1, keys);
        assert_eq!(pbft.fault_tolerance(), 0);
        assert_eq!(pbft.quorum_size(), 1);
    }

    // ---------------------------------------------------------------
    // Helpers for protocol tests
    // ---------------------------------------------------------------

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

    /// Build a 4-node cluster where each node uses its id bytes as both
    /// signing key and public key (so sign/verify are symmetric).
    fn make_cluster(ids: &[&str]) -> Vec<PbftConsensus> {
        let keys: HashMap<String, Vec<u8>> = ids
            .iter()
            .map(|id| (id.to_string(), id.as_bytes().to_vec()))
            .collect();
        ids.iter()
            .map(|id| {
                let mut c = PbftConsensus::new(id.to_string(), ids.len(), keys.clone());
                c.signing_key = Some(id.as_bytes().to_vec());
                c
            })
            .collect()
    }

    // ---------------------------------------------------------------
    // Propose tests
    // ---------------------------------------------------------------

    #[test]
    fn propose_succeeds_for_leader() {
        // sorted: ["a","b","c","d"], view 0 → leader "a"
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let block = make_block("deadbeef");
        let msg = cluster[0].propose(block).unwrap();
        match msg {
            PbftMessage::PrePrepare {
                view,
                sequence,
                ref digest,
                ref leader_id,
                ..
            } => {
                assert_eq!(view, 0);
                assert_eq!(sequence, 0);
                assert_eq!(digest, "deadbeef");
                assert_eq!(leader_id, "a");
            }
            _ => panic!("expected PrePrepare"),
        }
        assert_eq!(cluster[0].state, PbftState::PrePrepared);
        assert_eq!(cluster[0].sequence, 1);
    }

    #[test]
    fn propose_fails_for_non_leader() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let block = make_block("deadbeef");
        let err = cluster[1].propose(block).unwrap_err();
        assert_eq!(err, PbftError::NotLeader);
    }

    // ---------------------------------------------------------------
    // PrePrepare handling
    // ---------------------------------------------------------------

    #[test]
    fn handle_pre_prepare_returns_prepare() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let block = make_block("deadbeef");
        let pp = cluster[0].propose(block).unwrap();

        let prepare = cluster[1].handle_pre_prepare(pp).unwrap().unwrap();
        match prepare {
            PbftMessage::Prepare {
                view,
                sequence,
                ref node_id,
                ..
            } => {
                assert_eq!(view, 0);
                assert_eq!(sequence, 0);
                assert_eq!(node_id, "b");
            }
            _ => panic!("expected Prepare"),
        }
        assert_eq!(cluster[1].state, PbftState::PrePrepared);
    }

    #[test]
    fn handle_pre_prepare_rejects_wrong_view() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let block = make_block("deadbeef");
        let mut pp = cluster[0].propose(block).unwrap();
        if let PbftMessage::PrePrepare { ref mut view, .. } = pp {
            *view = 99;
        }
        let err = cluster[1].handle_pre_prepare(pp).unwrap_err();
        assert_eq!(err, PbftError::InvalidView);
    }

    #[test]
    fn handle_pre_prepare_rejects_wrong_leader() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let block = make_block("deadbeef");
        let mut pp = cluster[0].propose(block).unwrap();
        if let PbftMessage::PrePrepare {
            ref mut leader_id, ..
        } = pp
        {
            *leader_id = "b".to_string(); // not the real leader
        }
        let err = cluster[1].handle_pre_prepare(pp).unwrap_err();
        assert_eq!(err, PbftError::NotLeader);
    }

    #[test]
    fn handle_pre_prepare_rejects_bad_digest() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let block = make_block("deadbeef");
        let mut pp = cluster[0].propose(block).unwrap();
        if let PbftMessage::PrePrepare { ref mut digest, .. } = pp {
            *digest = "wrong-digest".to_string();
        }
        let err = cluster[1].handle_pre_prepare(pp).unwrap_err();
        assert_eq!(err, PbftError::InvalidDigest);
    }

    // ---------------------------------------------------------------
    // Prepare handling
    // ---------------------------------------------------------------

    #[test]
    fn prepare_quorum_transitions_to_prepared() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let block = make_block("deadbeef");
        let pp = cluster[0].propose(block).unwrap();

        // Replicas b, c, d handle PrePrepare → each returns Prepare
        let prep_b = cluster[1].handle_pre_prepare(pp.clone()).unwrap().unwrap();
        let prep_c = cluster[2].handle_pre_prepare(pp.clone()).unwrap().unwrap();
        let prep_d = cluster[3].handle_pre_prepare(pp).unwrap().unwrap();

        // Leader "a" collects Prepares. quorum = 3 (f=1, 2f+1=3).
        // "a" already has its own PrePrepare, so we feed it 3 Prepare messages.
        // After quorum_size (3) Prepares, it should transition to Prepared.
        assert!(cluster[0].handle_prepare(prep_b).unwrap().is_none());
        assert!(cluster[0].handle_prepare(prep_c).unwrap().is_none());
        let commit = cluster[0].handle_prepare(prep_d).unwrap();
        assert!(commit.is_some());
        assert_eq!(cluster[0].state, PbftState::Prepared);
    }

    #[test]
    fn prepare_rejects_duplicate_from_same_node() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let block = make_block("deadbeef");
        let pp = cluster[0].propose(block).unwrap();

        let prep_b = cluster[1].handle_pre_prepare(pp).unwrap().unwrap();
        cluster[0].handle_prepare(prep_b.clone()).unwrap();
        let err = cluster[0].handle_prepare(prep_b).unwrap_err();
        assert_eq!(err, PbftError::DuplicateMessage);
    }

    #[test]
    fn prepare_rejects_unknown_node() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let block = make_block("deadbeef");
        let _pp = cluster[0].propose(block).unwrap();

        let fake = PbftMessage::Prepare {
            view: 0,
            sequence: 0,
            digest: "deadbeef".into(),
            node_id: "unknown".into(),
            signature: vec![],
        };
        let err = cluster[0].handle_prepare(fake).unwrap_err();
        assert_eq!(err, PbftError::UnknownNode("unknown".into()));
    }

    // ---------------------------------------------------------------
    // Commit handling
    // ---------------------------------------------------------------

    #[test]
    fn full_consensus_round_commits_block() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let block = make_block("deadbeef");
        let pp = cluster[0].propose(block.clone()).unwrap();

        // Replicas handle PrePrepare
        let prep_b = cluster[1].handle_pre_prepare(pp.clone()).unwrap().unwrap();
        let prep_c = cluster[2].handle_pre_prepare(pp.clone()).unwrap().unwrap();
        let prep_d = cluster[3].handle_pre_prepare(pp).unwrap().unwrap();

        // Leader collects Prepares until quorum → gets Commit
        cluster[0].handle_prepare(prep_b).unwrap();
        cluster[0].handle_prepare(prep_c).unwrap();
        let _commit_from_a = cluster[0].handle_prepare(prep_d).unwrap().unwrap();

        // Distribute leader's commit to replicas and collect their commits
        // Replica b: collect commits from a, c, d
        // But first, each replica also needs to reach Prepared state.
        // Let's do this for replica b:
        // b already has its own Prepare from handle_pre_prepare.
        // Feed b the Prepares from c and d so b transitions to Prepared.
        let prep_c_for_b = PbftMessage::Prepare {
            view: 0,
            sequence: 0,
            digest: "deadbeef".into(),
            node_id: "c".into(),
            signature: PbftConsensus::make_sig("c", "deadbeef"),
        };
        let prep_d_for_b = PbftMessage::Prepare {
            view: 0,
            sequence: 0,
            digest: "deadbeef".into(),
            node_id: "d".into(),
            signature: PbftConsensus::make_sig("d", "deadbeef"),
        };
        cluster[1].handle_prepare(prep_c_for_b).unwrap();
        let commit_from_b = cluster[1].handle_prepare(prep_d_for_b).unwrap().unwrap();

        // Now leader collects commit messages.
        // Leader already has its own commit from the prepare-quorum step (count=1).
        // commit_from_b → count=2, commit_c → count=3 = quorum → committed.
        cluster[0].handle_commit(commit_from_b).unwrap();

        let commit_c = PbftMessage::Commit {
            view: 0,
            sequence: 0,
            digest: "deadbeef".into(),
            node_id: "c".into(),
            signature: PbftConsensus::make_sig("c", "deadbeef"),
        };
        let committed = cluster[0].handle_commit(commit_c).unwrap();

        assert!(committed.is_some());
        assert_eq!(cluster[0].state, PbftState::Committed);
        assert_eq!(cluster[0].committed_log.len(), 1);
        assert_eq!(cluster[0].committed_log[0].content_hash, "deadbeef");
    }

    #[test]
    fn commit_rejects_duplicate() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let block = make_block("deadbeef");
        let pp = cluster[0].propose(block).unwrap();
        let prep_b = cluster[1].handle_pre_prepare(pp.clone()).unwrap().unwrap();
        let prep_c = cluster[2].handle_pre_prepare(pp.clone()).unwrap().unwrap();
        let prep_d = cluster[3].handle_pre_prepare(pp).unwrap().unwrap();
        cluster[0].handle_prepare(prep_b).unwrap();
        cluster[0].handle_prepare(prep_c).unwrap();
        cluster[0].handle_prepare(prep_d).unwrap();

        let commit_b = PbftMessage::Commit {
            view: 0,
            sequence: 0,
            digest: "deadbeef".into(),
            node_id: "b".into(),
            signature: PbftConsensus::make_sig("b", "deadbeef"),
        };
        cluster[0].handle_commit(commit_b.clone()).unwrap();
        let err = cluster[0].handle_commit(commit_b).unwrap_err();
        assert_eq!(err, PbftError::DuplicateMessage);
    }

    // ---------------------------------------------------------------
    // handle_message router
    // ---------------------------------------------------------------

    #[test]
    fn handle_message_routes_pre_prepare() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let block = make_block("deadbeef");
        let pp = cluster[0].propose(block).unwrap();

        let output = cluster[1].handle_message(pp).unwrap();
        assert!(matches!(
            output,
            PbftOutput::Broadcast(PbftMessage::Prepare { .. })
        ));
    }

    #[test]
    fn handle_message_routes_commit_to_block_committed() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let block = make_block("deadbeef");
        let pp = cluster[0].propose(block).unwrap();

        let prep_b = cluster[1].handle_pre_prepare(pp.clone()).unwrap().unwrap();
        let prep_c = cluster[2].handle_pre_prepare(pp.clone()).unwrap().unwrap();
        let prep_d = cluster[3].handle_pre_prepare(pp).unwrap().unwrap();

        cluster[0].handle_prepare(prep_b).unwrap();
        cluster[0].handle_prepare(prep_c).unwrap();
        cluster[0].handle_prepare(prep_d).unwrap();

        // Feed commits until quorum.
        // Leader already auto-counted its own commit (count=1).
        // After commit_b (count=2) and commit_c (count=3=quorum) → committed.
        let commit_b = PbftMessage::Commit {
            view: 0,
            sequence: 0,
            digest: "deadbeef".into(),
            node_id: "b".into(),
            signature: PbftConsensus::make_sig("b", "deadbeef"),
        };
        let commit_c = PbftMessage::Commit {
            view: 0,
            sequence: 0,
            digest: "deadbeef".into(),
            node_id: "c".into(),
            signature: PbftConsensus::make_sig("c", "deadbeef"),
        };
        cluster[0].handle_message(commit_b).unwrap();
        let output = cluster[0].handle_message(commit_c).unwrap();
        assert!(matches!(output, PbftOutput::BlockCommitted(_)));
    }

    // ---------------------------------------------------------------
    // View change tests
    // ---------------------------------------------------------------

    #[test]
    fn request_view_change_creates_message() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let msg = cluster[1].request_view_change().unwrap();
        match &msg {
            PbftMessage::ViewChange {
                new_view,
                node_id,
                last_committed_seq,
                ..
            } => {
                assert_eq!(*new_view, 1);
                assert_eq!(node_id, "b");
                assert_eq!(*last_committed_seq, 0);
            }
            _ => panic!("expected ViewChange"),
        }
        assert_eq!(cluster[1].state, PbftState::ViewChange);
    }

    #[test]
    fn view_change_quorum_produces_new_view() {
        // sorted: ["a","b","c","d"], view 1 → leader "b"
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);

        // Nodes a, c, d request view change (3 = quorum for 4 nodes)
        let vc_a = cluster[0].request_view_change().unwrap();
        let vc_c = cluster[2].request_view_change().unwrap();
        let vc_d = cluster[3].request_view_change().unwrap();

        // Node b (new leader for view 1) collects ViewChange messages
        cluster[1].handle_view_change(vc_a).unwrap();
        cluster[1].handle_view_change(vc_c).unwrap();
        // After 3rd ViewChange (quorum=3), b should produce NewView
        let new_view_msg = cluster[1].handle_view_change(vc_d).unwrap();
        assert!(new_view_msg.is_some());

        let nv = new_view_msg.unwrap();
        match &nv {
            PbftMessage::NewView {
                new_view,
                leader_id,
                view_change_proofs,
                ..
            } => {
                assert_eq!(*new_view, 1);
                assert_eq!(leader_id, "b");
                assert_eq!(view_change_proofs.len(), 3);
            }
            _ => panic!("expected NewView"),
        }
        assert_eq!(cluster[1].view, 1);
        assert_eq!(cluster[1].state, PbftState::Idle);
    }

    #[test]
    fn handle_new_view_advances_replica() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);

        // Build a valid NewView with quorum ViewChange proofs
        let vc_a = cluster[0].request_view_change().unwrap();
        let vc_c = cluster[2].request_view_change().unwrap();
        let vc_d = cluster[3].request_view_change().unwrap();

        // b collects and produces NewView
        cluster[1].handle_view_change(vc_a).unwrap();
        cluster[1].handle_view_change(vc_c).unwrap();
        let nv = cluster[1].handle_view_change(vc_d).unwrap().unwrap();

        // Replica c applies the NewView
        cluster[2].handle_new_view(nv).unwrap();
        assert_eq!(cluster[2].view, 1);
        assert_eq!(cluster[2].state, PbftState::Idle);
    }

    #[test]
    fn handle_new_view_rejects_wrong_leader() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);

        // Forge a NewView with wrong leader
        let fake_nv = PbftMessage::NewView {
            new_view: 1,
            leader_id: "a".into(), // should be "b" for view 1
            view_change_proofs: vec![],
            signature: vec![],
        };
        let err = cluster[0].handle_new_view(fake_nv).unwrap_err();
        assert_eq!(err, PbftError::NotLeader);
    }

    #[test]
    fn handle_new_view_rejects_insufficient_proofs() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let vc_a = cluster[0].request_view_change().unwrap();

        // Only 1 proof, need 3
        let fake_nv = PbftMessage::NewView {
            new_view: 1,
            leader_id: "b".into(),
            view_change_proofs: vec![vc_a],
            signature: vec![],
        };
        let err = cluster[2].handle_new_view(fake_nv).unwrap_err();
        assert_eq!(err, PbftError::InsufficientNodes);
    }

    #[test]
    fn view_change_rejects_stale_view() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        cluster[0].view = 5;
        let stale = PbftMessage::ViewChange {
            new_view: 3,
            node_id: "b".into(),
            last_committed_seq: 0,
            proof: vec![],
            signature: vec![],
        };
        let err = cluster[0].handle_view_change(stale).unwrap_err();
        assert_eq!(err, PbftError::InvalidView);
    }

    #[test]
    fn view_change_rejects_duplicate() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        let vc_a = cluster[0].request_view_change().unwrap();
        cluster[1].handle_view_change(vc_a.clone()).unwrap();
        let err = cluster[1].handle_view_change(vc_a).unwrap_err();
        assert_eq!(err, PbftError::DuplicateMessage);
    }

    // ---------------------------------------------------------------
    // Leader timeout
    // ---------------------------------------------------------------

    #[test]
    fn check_leader_timeout_accumulates() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        assert!(!cluster[1].check_leader_timeout(500, 1000));
        assert_eq!(cluster[1].leader_idle_ms, 500);
        assert!(!cluster[1].check_leader_timeout(400, 1000));
        assert_eq!(cluster[1].leader_idle_ms, 900);
        assert!(cluster[1].check_leader_timeout(200, 1000));
        assert_eq!(cluster[1].leader_idle_ms, 1100);
    }

    #[test]
    fn check_leader_timeout_not_triggered_during_view_change() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);
        cluster[1].state = PbftState::ViewChange;
        // Even though time exceeds timeout, state is ViewChange → false
        assert!(!cluster[1].check_leader_timeout(2000, 1000));
    }

    // ---------------------------------------------------------------
    // Equivocation detection
    // ---------------------------------------------------------------

    #[test]
    fn detect_equivocation_on_pre_prepare() {
        let cluster = make_cluster(&["a", "b", "c", "d"]);
        let pp1 = PbftMessage::PrePrepare {
            view: 0,
            sequence: 0,
            digest: "block1".into(),
            block: make_block("block1"),
            leader_id: "a".into(),
        };
        let pp2 = PbftMessage::PrePrepare {
            view: 0,
            sequence: 0,
            digest: "block2".into(),
            block: make_block("block2"),
            leader_id: "a".into(),
        };
        assert!(cluster[0].detect_equivocation(&pp1, &pp2));
    }

    #[test]
    fn no_equivocation_on_same_digest() {
        let cluster = make_cluster(&["a", "b", "c", "d"]);
        let pp1 = PbftMessage::PrePrepare {
            view: 0,
            sequence: 0,
            digest: "same".into(),
            block: make_block("same"),
            leader_id: "a".into(),
        };
        let pp2 = pp1.clone();
        assert!(!cluster[0].detect_equivocation(&pp1, &pp2));
    }

    #[test]
    fn detect_equivocation_on_prepare() {
        let cluster = make_cluster(&["a", "b", "c", "d"]);
        let p1 = PbftMessage::Prepare {
            view: 0,
            sequence: 0,
            digest: "d1".into(),
            node_id: "b".into(),
            signature: vec![],
        };
        let p2 = PbftMessage::Prepare {
            view: 0,
            sequence: 0,
            digest: "d2".into(),
            node_id: "b".into(),
            signature: vec![],
        };
        assert!(cluster[0].detect_equivocation(&p1, &p2));
    }

    #[test]
    fn detect_equivocation_on_commit() {
        let cluster = make_cluster(&["a", "b", "c", "d"]);
        let c1 = PbftMessage::Commit {
            view: 0,
            sequence: 0,
            digest: "d1".into(),
            node_id: "c".into(),
            signature: vec![],
        };
        let c2 = PbftMessage::Commit {
            view: 0,
            sequence: 0,
            digest: "d2".into(),
            node_id: "c".into(),
            signature: vec![],
        };
        assert!(cluster[0].detect_equivocation(&c1, &c2));
    }

    #[test]
    fn no_equivocation_across_different_types() {
        let cluster = make_cluster(&["a", "b", "c", "d"]);
        let p = PbftMessage::Prepare {
            view: 0,
            sequence: 0,
            digest: "d1".into(),
            node_id: "b".into(),
            signature: vec![],
        };
        let c = PbftMessage::Commit {
            view: 0,
            sequence: 0,
            digest: "d2".into(),
            node_id: "b".into(),
            signature: vec![],
        };
        // Different message types → not equivocation
        assert!(!cluster[0].detect_equivocation(&p, &c));
    }

    // ---------------------------------------------------------------
    // Full view change + propose on new view
    // ---------------------------------------------------------------

    #[test]
    fn new_leader_can_propose_after_view_change() {
        let mut cluster = make_cluster(&["a", "b", "c", "d"]);

        // Trigger view change: view 0 → view 1, new leader = "b"
        let vc_a = cluster[0].request_view_change().unwrap();
        let vc_c = cluster[2].request_view_change().unwrap();
        let vc_d = cluster[3].request_view_change().unwrap();

        cluster[1].handle_view_change(vc_a).unwrap();
        cluster[1].handle_view_change(vc_c).unwrap();
        let nv = cluster[1].handle_view_change(vc_d).unwrap().unwrap();

        // Apply to replicas
        cluster[0].handle_new_view(nv.clone()).unwrap();
        cluster[2].handle_new_view(nv.clone()).unwrap();
        cluster[3].handle_new_view(nv).unwrap();

        // Now "b" is leader for view 1 — can propose
        let block = make_block("new-block");
        let pp = cluster[1].propose(block).unwrap();
        match pp {
            PbftMessage::PrePrepare {
                view, leader_id, ..
            } => {
                assert_eq!(view, 1);
                assert_eq!(leader_id, "b");
            }
            _ => panic!("expected PrePrepare"),
        }

        // Old leader "a" cannot propose
        let err = cluster[0].propose(make_block("x")).unwrap_err();
        assert_eq!(err, PbftError::NotLeader);
    }
}
