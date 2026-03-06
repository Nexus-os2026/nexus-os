//! Quorum-backed execution for high-risk distributed actions.
//!
//! When agents at L2+ attempt actions in a distributed context, the kernel
//! can require quorum approval before execution proceeds.

use crate::node::{ClusterView, NodeState};
use nexus_kernel::audit::{AuditTrail, EventType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuorumRequest {
    pub request_id: Uuid,
    pub agent_id: Uuid,
    pub action_description: String,
    pub required_votes: usize,
    pub timeout_secs: u64,
    pub policy_hash: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuorumVote {
    Approve { node_id: Uuid, signature: Vec<u8> },
    Reject { node_id: Uuid, reason: String },
    Abstain { node_id: Uuid },
}

impl QuorumVote {
    pub fn node_id(&self) -> Uuid {
        match self {
            QuorumVote::Approve { node_id, .. } => *node_id,
            QuorumVote::Reject { node_id, .. } => *node_id,
            QuorumVote::Abstain { node_id } => *node_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuorumOutcome {
    Approved {
        votes: Vec<QuorumVote>,
        decided_at: u64,
    },
    Rejected {
        votes: Vec<QuorumVote>,
        reason: String,
    },
    TimedOut {
        votes_received: Vec<QuorumVote>,
    },
    InsufficientNodes,
}

#[derive(Debug)]
pub struct QuorumEngine {
    pending: HashMap<Uuid, (QuorumRequest, Vec<QuorumVote>)>,
    decided: HashMap<Uuid, QuorumOutcome>,
    cluster: ClusterView,
}

impl QuorumEngine {
    pub fn new(cluster: ClusterView) -> Self {
        Self {
            pending: HashMap::new(),
            decided: HashMap::new(),
            cluster,
        }
    }

    pub fn cluster_view(&self) -> &ClusterView {
        &self.cluster
    }

    pub fn cluster_view_mut(&mut self) -> &mut ClusterView {
        &mut self.cluster
    }

    /// Propose a new quorum request. Returns the request_id on success,
    /// or InsufficientNodes if the cluster doesn't have enough active nodes.
    pub fn propose(
        &mut self,
        agent_id: Uuid,
        action_description: String,
        required_votes: usize,
        timeout_secs: u64,
        policy_hash: String,
        audit_trail: &mut AuditTrail,
    ) -> Result<Uuid, QuorumOutcome> {
        let active = self.cluster.active_count();
        if active < required_votes {
            let outcome = QuorumOutcome::InsufficientNodes;
            audit_trail.append_event(
                agent_id,
                EventType::StateChange,
                serde_json::json!({
                    "event": "quorum.propose_failed",
                    "reason": "insufficient_nodes",
                    "active_nodes": active,
                    "required_votes": required_votes,
                    "action": &action_description,
                }),
            );
            return Err(outcome);
        }

        let request_id = Uuid::new_v4();
        let request = QuorumRequest {
            request_id,
            agent_id,
            action_description: action_description.clone(),
            required_votes,
            timeout_secs,
            policy_hash: policy_hash.clone(),
            created_at: unix_now(),
        };

        audit_trail.append_event(
            agent_id,
            EventType::StateChange,
            serde_json::json!({
                "event": "quorum.proposed",
                "request_id": request_id.to_string(),
                "action": &action_description,
                "required_votes": required_votes,
                "policy_hash": &policy_hash,
            }),
        );

        self.pending.insert(request_id, (request, Vec::new()));
        Ok(request_id)
    }

    /// Record a vote for a pending request. Returns Some(QuorumOutcome) when
    /// the request reaches a decision (enough approvals or rejections).
    pub fn vote(
        &mut self,
        request_id: Uuid,
        vote: QuorumVote,
        audit_trail: &mut AuditTrail,
    ) -> Option<QuorumOutcome> {
        let (request, votes) = self.pending.get_mut(&request_id)?;
        let agent_id = request.agent_id;
        let required = request.required_votes;

        // Prevent duplicate votes from the same node
        let voter = vote.node_id();
        if votes.iter().any(|v| v.node_id() == voter) {
            return None;
        }

        let vote_label = match &vote {
            QuorumVote::Approve { .. } => "approve",
            QuorumVote::Reject { reason, .. } => reason.as_str(),
            QuorumVote::Abstain { .. } => "abstain",
        };

        audit_trail.append_event(
            agent_id,
            EventType::UserAction,
            serde_json::json!({
                "event": "quorum.vote",
                "request_id": request_id.to_string(),
                "node_id": voter.to_string(),
                "vote": vote_label,
            }),
        );

        votes.push(vote);

        let approvals = votes
            .iter()
            .filter(|v| matches!(v, QuorumVote::Approve { .. }))
            .count();
        let rejections = votes
            .iter()
            .filter(|v| matches!(v, QuorumVote::Reject { .. }))
            .count();
        let total_active = self.cluster.active_count();

        // Check if we have enough approvals
        if approvals >= required {
            let (_, final_votes) = self.pending.remove(&request_id).unwrap();
            let outcome = QuorumOutcome::Approved {
                votes: final_votes,
                decided_at: unix_now(),
            };
            audit_trail.append_event(
                agent_id,
                EventType::StateChange,
                serde_json::json!({
                    "event": "quorum.outcome",
                    "request_id": request_id.to_string(),
                    "result": "approved",
                    "approvals": approvals,
                }),
            );
            self.decided.insert(request_id, outcome.clone());
            return Some(outcome);
        }

        // Check if enough rejections make approval impossible
        let remaining = total_active.saturating_sub(votes.len());
        if approvals + remaining < required {
            let (_, final_votes) = self.pending.remove(&request_id).unwrap();
            let outcome = QuorumOutcome::Rejected {
                votes: final_votes,
                reason: format!(
                    "insufficient approvals possible: {} approvals, {} rejections, {} remaining",
                    approvals, rejections, remaining
                ),
            };
            audit_trail.append_event(
                agent_id,
                EventType::StateChange,
                serde_json::json!({
                    "event": "quorum.outcome",
                    "request_id": request_id.to_string(),
                    "result": "rejected",
                    "rejections": rejections,
                }),
            );
            self.decided.insert(request_id, outcome.clone());
            return Some(outcome);
        }

        None
    }

    /// Scan pending requests and return outcomes for any that have timed out.
    pub fn check_timeouts(&mut self, audit_trail: &mut AuditTrail) -> Vec<(Uuid, QuorumOutcome)> {
        let now = unix_now();
        let timed_out: Vec<Uuid> = self
            .pending
            .iter()
            .filter(|(_, (req, _))| now.saturating_sub(req.created_at) >= req.timeout_secs)
            .map(|(id, _)| *id)
            .collect();

        let mut results = Vec::new();
        for request_id in timed_out {
            let (request, votes) = self.pending.remove(&request_id).unwrap();
            let outcome = QuorumOutcome::TimedOut {
                votes_received: votes,
            };
            audit_trail.append_event(
                request.agent_id,
                EventType::StateChange,
                serde_json::json!({
                    "event": "quorum.outcome",
                    "request_id": request_id.to_string(),
                    "result": "timed_out",
                }),
            );
            self.decided.insert(request_id, outcome.clone());
            results.push((request_id, outcome));
        }
        results
    }

    /// Get the outcome for a decided request.
    pub fn outcome(&self, request_id: Uuid) -> Option<&QuorumOutcome> {
        self.decided.get(&request_id)
    }

    /// For testing: propose with a manually specified created_at timestamp.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn propose_with_timestamp(
        &mut self,
        agent_id: Uuid,
        action_description: String,
        required_votes: usize,
        timeout_secs: u64,
        policy_hash: String,
        created_at: u64,
        audit_trail: &mut AuditTrail,
    ) -> Result<Uuid, QuorumOutcome> {
        let active = self.cluster.active_count();
        if active < required_votes {
            let outcome = QuorumOutcome::InsufficientNodes;
            audit_trail.append_event(
                agent_id,
                EventType::StateChange,
                serde_json::json!({
                    "event": "quorum.propose_failed",
                    "reason": "insufficient_nodes",
                    "active_nodes": active,
                    "required_votes": required_votes,
                    "action": &action_description,
                }),
            );
            return Err(outcome);
        }

        let request_id = Uuid::new_v4();
        let request = QuorumRequest {
            request_id,
            agent_id,
            action_description: action_description.clone(),
            required_votes,
            timeout_secs,
            policy_hash: policy_hash.clone(),
            created_at,
        };

        audit_trail.append_event(
            agent_id,
            EventType::StateChange,
            serde_json::json!({
                "event": "quorum.proposed",
                "request_id": request_id.to_string(),
                "action": &action_description,
                "required_votes": required_votes,
                "policy_hash": &policy_hash,
            }),
        );

        self.pending.insert(request_id, (request, Vec::new()));
        Ok(request_id)
    }
}

/// Check whether a quorum approval is required for the given autonomy level
/// in a distributed context. Returns true if the agent is L2+ and the cluster
/// has more than one active node.
pub fn requires_quorum(
    autonomy_level: nexus_kernel::autonomy::AutonomyLevel,
    cluster: &ClusterView,
) -> bool {
    use nexus_kernel::autonomy::AutonomyLevel;
    autonomy_level >= AutonomyLevel::L2
        && cluster.active_count() > 1
        && cluster.members.iter().any(|(_, s)| *s == NodeState::Active)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{NodeIdentity, NodeState};
    use nexus_kernel::audit::AuditTrail;

    fn make_cluster(n: usize) -> ClusterView {
        let mut view = ClusterView::new(n);
        for i in 0..n {
            view.add_node(
                NodeIdentity {
                    id: Uuid::new_v4(),
                    name: format!("node-{i}"),
                    addr: format!("127.0.0.1:900{i}").parse().unwrap(),
                    public_key: vec![0; 32],
                    capabilities: vec!["audit".to_string()],
                    joined_at: 1000,
                },
                NodeState::Active,
            );
        }
        view
    }

    fn node_ids(view: &ClusterView) -> Vec<Uuid> {
        view.members.iter().map(|(n, _)| n.id).collect()
    }

    #[test]
    fn three_node_quorum_two_approvals() {
        let cluster = make_cluster(3);
        let nodes = node_ids(&cluster);
        let mut engine = QuorumEngine::new(cluster);
        let mut audit = AuditTrail::new();
        let agent = Uuid::new_v4();

        let req_id = engine
            .propose(
                agent,
                "deploy model".to_string(),
                2,
                60,
                "policy-abc".to_string(),
                &mut audit,
            )
            .unwrap();

        // First approval — not yet decided
        let result = engine.vote(
            req_id,
            QuorumVote::Approve {
                node_id: nodes[0],
                signature: vec![1],
            },
            &mut audit,
        );
        assert!(result.is_none());

        // Second approval — quorum reached
        let result = engine.vote(
            req_id,
            QuorumVote::Approve {
                node_id: nodes[1],
                signature: vec![2],
            },
            &mut audit,
        );
        assert!(matches!(result, Some(QuorumOutcome::Approved { .. })));

        // Verify outcome is stored
        let outcome = engine.outcome(req_id);
        assert!(matches!(outcome, Some(QuorumOutcome::Approved { .. })));
    }

    #[test]
    fn three_node_quorum_two_rejections() {
        let cluster = make_cluster(3);
        let nodes = node_ids(&cluster);
        let mut engine = QuorumEngine::new(cluster);
        let mut audit = AuditTrail::new();
        let agent = Uuid::new_v4();

        let req_id = engine
            .propose(
                agent,
                "risky action".to_string(),
                2,
                60,
                "policy-xyz".to_string(),
                &mut audit,
            )
            .unwrap();

        // Two rejections — can't reach 2 approvals with only 1 node left
        let result = engine.vote(
            req_id,
            QuorumVote::Reject {
                node_id: nodes[0],
                reason: "too risky".to_string(),
            },
            &mut audit,
        );
        assert!(result.is_none());

        let result = engine.vote(
            req_id,
            QuorumVote::Reject {
                node_id: nodes[1],
                reason: "disagree".to_string(),
            },
            &mut audit,
        );
        assert!(matches!(result, Some(QuorumOutcome::Rejected { .. })));
    }

    #[test]
    fn timeout_with_insufficient_votes() {
        let cluster = make_cluster(3);
        let nodes = node_ids(&cluster);
        let mut engine = QuorumEngine::new(cluster);
        let mut audit = AuditTrail::new();
        let agent = Uuid::new_v4();

        // Use a created_at in the past so it's already timed out
        let req_id = engine
            .propose_with_timestamp(
                agent,
                "slow action".to_string(),
                2,
                1, // 1 second timeout
                "policy-timeout".to_string(),
                0, // created_at = epoch (well past timeout)
                &mut audit,
            )
            .unwrap();

        // Cast one vote but not enough
        engine.vote(
            req_id,
            QuorumVote::Approve {
                node_id: nodes[0],
                signature: vec![1],
            },
            &mut audit,
        );

        let timeouts = engine.check_timeouts(&mut audit);
        assert_eq!(timeouts.len(), 1);
        assert_eq!(timeouts[0].0, req_id);
        assert!(matches!(timeouts[0].1, QuorumOutcome::TimedOut { .. }));

        if let QuorumOutcome::TimedOut { votes_received } = &timeouts[0].1 {
            assert_eq!(votes_received.len(), 1);
        }
    }

    #[test]
    fn propose_fails_with_insufficient_nodes() {
        let cluster = make_cluster(1); // only 1 active node
        let mut engine = QuorumEngine::new(cluster);
        let mut audit = AuditTrail::new();
        let agent = Uuid::new_v4();

        let result = engine.propose(
            agent,
            "need 3 but have 1".to_string(),
            3, // require 3 votes
            60,
            "policy-fail".to_string(),
            &mut audit,
        );

        assert!(matches!(result, Err(QuorumOutcome::InsufficientNodes)));
    }

    #[test]
    fn every_outcome_gets_audit_event() {
        let cluster = make_cluster(3);
        let nodes = node_ids(&cluster);
        let mut engine = QuorumEngine::new(cluster);
        let mut audit = AuditTrail::new();
        let agent = Uuid::new_v4();

        // Propose
        let req_id = engine
            .propose(
                agent,
                "audited action".to_string(),
                2,
                60,
                "policy-audit".to_string(),
                &mut audit,
            )
            .unwrap();

        let propose_event = audit
            .events()
            .iter()
            .find(|e| e.payload.get("event").and_then(|v| v.as_str()) == Some("quorum.proposed"))
            .expect("propose must create audit event");
        assert_eq!(
            propose_event
                .payload
                .get("request_id")
                .and_then(|v| v.as_str()),
            Some(req_id.to_string().as_str())
        );

        // Vote 1
        engine.vote(
            req_id,
            QuorumVote::Approve {
                node_id: nodes[0],
                signature: vec![1],
            },
            &mut audit,
        );

        let vote_events: Vec<_> = audit
            .events()
            .iter()
            .filter(|e| e.payload.get("event").and_then(|v| v.as_str()) == Some("quorum.vote"))
            .collect();
        assert_eq!(vote_events.len(), 1);

        // Vote 2 — triggers outcome
        engine.vote(
            req_id,
            QuorumVote::Approve {
                node_id: nodes[1],
                signature: vec![2],
            },
            &mut audit,
        );

        let outcome_event = audit
            .events()
            .iter()
            .find(|e| e.payload.get("event").and_then(|v| v.as_str()) == Some("quorum.outcome"))
            .expect("outcome must create audit event");
        assert_eq!(
            outcome_event.payload.get("result").and_then(|v| v.as_str()),
            Some("approved")
        );

        // Total audit events: 1 propose + 2 votes + 1 outcome = 4
        assert_eq!(audit.events().len(), 4);
    }

    #[test]
    fn duplicate_vote_ignored() {
        let cluster = make_cluster(3);
        let nodes = node_ids(&cluster);
        let mut engine = QuorumEngine::new(cluster);
        let mut audit = AuditTrail::new();
        let agent = Uuid::new_v4();

        let req_id = engine
            .propose(
                agent,
                "dup test".to_string(),
                2,
                60,
                "policy".to_string(),
                &mut audit,
            )
            .unwrap();

        engine.vote(
            req_id,
            QuorumVote::Approve {
                node_id: nodes[0],
                signature: vec![1],
            },
            &mut audit,
        );

        // Same node voting again should be ignored
        let result = engine.vote(
            req_id,
            QuorumVote::Approve {
                node_id: nodes[0],
                signature: vec![1],
            },
            &mut audit,
        );
        assert!(result.is_none());

        // Still need one more distinct vote to reach quorum
        let result = engine.vote(
            req_id,
            QuorumVote::Approve {
                node_id: nodes[1],
                signature: vec![2],
            },
            &mut audit,
        );
        assert!(matches!(result, Some(QuorumOutcome::Approved { .. })));
    }

    #[test]
    fn requires_quorum_for_l2_plus() {
        use nexus_kernel::autonomy::AutonomyLevel;

        let cluster = make_cluster(3);
        assert!(!requires_quorum(AutonomyLevel::L0, &cluster));
        assert!(!requires_quorum(AutonomyLevel::L1, &cluster));
        assert!(requires_quorum(AutonomyLevel::L2, &cluster));
        assert!(requires_quorum(AutonomyLevel::L3, &cluster));
        assert!(requires_quorum(AutonomyLevel::L5, &cluster));

        // Single node cluster — no quorum needed
        let single = make_cluster(1);
        assert!(!requires_quorum(AutonomyLevel::L3, &single));
    }
}
