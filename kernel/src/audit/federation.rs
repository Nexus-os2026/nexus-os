//! Federated audit chains with cross-node hash references for tamper-evident verification.

use super::{AuditEvent, AuditTrail, EventType};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossRef {
    pub remote_node_id: Uuid,
    pub remote_chain_hash: String,
    pub observed_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederatedAuditEvent {
    pub local_event: AuditEvent,
    pub node_id: Uuid,
    pub node_chain_hash: String,
    pub cross_references: Vec<CrossRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationProof {
    pub node_id: Uuid,
    pub cross_references: Vec<CrossRef>,
    pub start_time: u64,
    pub end_time: u64,
    pub proof_digest: String,
}

#[derive(Debug)]
pub struct FederatedAuditTrail {
    node_id: Uuid,
    trail: AuditTrail,
    known_chains: HashMap<Uuid, String>,
    federated_events: Vec<FederatedAuditEvent>,
}

impl FederatedAuditTrail {
    pub fn new(node_id: Uuid) -> Self {
        Self {
            node_id,
            trail: AuditTrail::new(),
            known_chains: HashMap::new(),
            federated_events: Vec::new(),
        }
    }

    pub fn trail(&self) -> &AuditTrail {
        &self.trail
    }

    pub fn trail_mut(&mut self) -> &mut AuditTrail {
        &mut self.trail
    }

    pub fn node_id(&self) -> Uuid {
        self.node_id
    }

    pub fn known_chains(&self) -> &HashMap<Uuid, String> {
        &self.known_chains
    }

    pub fn federated_events(&self) -> &[FederatedAuditEvent] {
        &self.federated_events
    }

    /// Append an event to the local trail and attach cross-references from known remote chains.
    pub fn append_federated(
        &mut self,
        agent_id: Uuid,
        event_type: EventType,
        payload: serde_json::Value,
    ) -> Result<Uuid, super::AuditError> {
        let event_id = self.trail.append_event(agent_id, event_type, payload)?;

        let local_event = self
            .trail
            .events()
            .iter()
            .find(|e| e.event_id == event_id)
            .ok_or(super::AuditError::SerializationFailed)?
            .clone();

        let cross_references: Vec<CrossRef> = self
            .known_chains
            .iter()
            .map(|(node_id, hash)| CrossRef {
                remote_node_id: *node_id,
                remote_chain_hash: hash.clone(),
                observed_at: unix_now(),
            })
            .collect();

        let node_chain_hash = local_event.hash.clone();

        self.federated_events.push(FederatedAuditEvent {
            local_event,
            node_id: self.node_id,
            node_chain_hash,
            cross_references,
        });

        Ok(event_id)
    }

    /// Record a remote node's latest chain hash (called during replication sync).
    pub fn update_remote_hash(&mut self, remote_node_id: Uuid, chain_hash: String) {
        self.known_chains.insert(remote_node_id, chain_hash);
    }

    /// Verify a remote node's events: check chain integrity and that hashes match cross-references.
    pub fn verify_remote(
        &self,
        remote_node_id: Uuid,
        remote_events: &[AuditEvent],
    ) -> FederationVerifyResult {
        // Find all cross-references we have for this remote node
        let our_refs: Vec<&CrossRef> = self
            .federated_events
            .iter()
            .flat_map(|fe| fe.cross_references.iter())
            .filter(|cr| cr.remote_node_id == remote_node_id)
            .collect();

        if our_refs.is_empty() {
            return FederationVerifyResult::NoCrossReferences;
        }

        // Verify the remote chain's integrity by recomputing hashes from content.
        for (i, event) in remote_events.iter().enumerate() {
            if i > 0 && event.previous_hash != remote_events[i - 1].hash {
                return FederationVerifyResult::Mismatch {
                    expected_hash: remote_events[i - 1].hash.clone(),
                    observed_at: our_refs.first().map(|r| r.observed_at).unwrap_or(0),
                };
            }
            // Recompute this event's hash from its content
            let expected_hash = recompute_event_hash(event);
            if event.hash != expected_hash {
                return FederationVerifyResult::Mismatch {
                    expected_hash: event.hash.clone(),
                    observed_at: our_refs.first().map(|r| r.observed_at).unwrap_or(0),
                };
            }
        }

        // Then check that every cross-reference hash appears in the verified remote chain
        let remote_hashes: std::collections::HashSet<&str> =
            remote_events.iter().map(|e| e.hash.as_str()).collect();

        for cr in &our_refs {
            if !remote_hashes.contains(cr.remote_chain_hash.as_str()) {
                return FederationVerifyResult::Mismatch {
                    expected_hash: cr.remote_chain_hash.clone(),
                    observed_at: cr.observed_at,
                };
            }
        }

        FederationVerifyResult::Verified {
            cross_refs_checked: our_refs.len(),
        }
    }

    /// Export all cross-references for a time range as a verifiable proof bundle.
    pub fn export_federation_proof(&self, start_time: u64, end_time: u64) -> FederationProof {
        let cross_references: Vec<CrossRef> = self
            .federated_events
            .iter()
            .filter(|fe| {
                fe.local_event.timestamp >= start_time && fe.local_event.timestamp <= end_time
            })
            .flat_map(|fe| fe.cross_references.iter().cloned())
            .collect();

        let proof_digest = compute_proof_digest(&cross_references);

        FederationProof {
            node_id: self.node_id,
            cross_references,
            start_time,
            end_time,
            proof_digest,
        }
    }
}

/// Recompute an event's hash from its content fields to detect tampering.
fn recompute_event_hash(event: &AuditEvent) -> String {
    #[derive(Serialize)]
    struct CanonicalEventData<'a> {
        event_id: &'a str,
        timestamp: u64,
        agent_id: &'a str,
        event_type: &'a EventType,
        payload: &'a serde_json::Value,
    }

    let event_id_string = event.event_id.to_string();
    let agent_id_string = event.agent_id.to_string();
    let canonical = CanonicalEventData {
        event_id: &event_id_string,
        timestamp: event.timestamp,
        agent_id: &agent_id_string,
        event_type: &event.event_type,
        payload: &event.payload,
    };

    let serialized = match serde_json::to_vec(&canonical) {
        Ok(bytes) => bytes,
        Err(_) => {
            // Return a deterministic fallback hash that will never match a valid event,
            // causing integrity verification to fail (fail-closed).
            return "0000000000000000000000000000000000000000000000000000000000000001".to_string();
        }
    };

    let mut hasher = Sha256::new();
    hasher.update(event.previous_hash.as_bytes());
    hasher.update(serialized);
    format!("{:x}", hasher.finalize())
}

fn compute_proof_digest(refs: &[CrossRef]) -> String {
    let mut hasher = Sha256::new();
    for cr in refs {
        hasher.update(cr.remote_node_id.as_bytes());
        hasher.update(cr.remote_chain_hash.as_bytes());
        hasher.update(cr.observed_at.to_le_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FederationVerifyResult {
    Verified {
        cross_refs_checked: usize,
    },
    Mismatch {
        expected_hash: String,
        observed_at: u64,
    },
    NoCrossReferences,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn two_nodes_cross_reference_and_verify() {
        let node_a_id = Uuid::new_v4();
        let node_b_id = Uuid::new_v4();

        let mut node_a = FederatedAuditTrail::new(node_a_id);
        let mut node_b = FederatedAuditTrail::new(node_b_id);

        let agent = Uuid::new_v4();

        // Node A appends an event
        node_a
            .append_federated(agent, EventType::StateChange, json!({"action": "init"}))
            .expect("audit append");
        let a_hash = node_a.trail().events().last().unwrap().hash.clone();

        // Node B learns about A's chain hash
        node_b.update_remote_hash(node_a_id, a_hash.clone());

        // Node B appends an event — it will cross-reference A's hash
        node_b
            .append_federated(agent, EventType::ToolCall, json!({"tool": "search"}))
            .expect("audit append");

        // Node B's federated event should have a cross-ref to A
        let b_fed = node_b.federated_events().last().unwrap();
        assert_eq!(b_fed.cross_references.len(), 1);
        assert_eq!(b_fed.cross_references[0].remote_node_id, node_a_id);
        assert_eq!(b_fed.cross_references[0].remote_chain_hash, a_hash);

        // Node A learns about B's chain hash
        let b_hash = node_b.trail().events().last().unwrap().hash.clone();
        node_a.update_remote_hash(node_b_id, b_hash);

        // Node A appends — now cross-refs B
        node_a
            .append_federated(agent, EventType::StateChange, json!({"action": "sync"}))
            .expect("audit append");
        let a_fed = node_a.federated_events().last().unwrap();
        assert_eq!(a_fed.cross_references.len(), 1);
        assert_eq!(a_fed.cross_references[0].remote_node_id, node_b_id);

        // Verify: B verifies A's events against its cross-references
        let result = node_b.verify_remote(node_a_id, node_a.trail().events());
        assert!(matches!(
            result,
            FederationVerifyResult::Verified {
                cross_refs_checked: 1
            }
        ));
    }

    #[test]
    fn tamper_on_remote_detected_via_cross_ref_mismatch() {
        let node_a_id = Uuid::new_v4();
        let node_b_id = Uuid::new_v4();

        let mut node_a = FederatedAuditTrail::new(node_a_id);
        let mut node_b = FederatedAuditTrail::new(node_b_id);

        let agent = Uuid::new_v4();

        // A appends event, B records the hash
        node_a
            .append_federated(agent, EventType::StateChange, json!({"v": 1}))
            .expect("audit append");
        let a_hash = node_a.trail().events().last().unwrap().hash.clone();
        node_b.update_remote_hash(node_a_id, a_hash);

        // B appends (creating cross-ref to A)
        node_b
            .append_federated(agent, EventType::StateChange, json!({"v": 2}))
            .expect("audit append");

        // Tamper with A's event
        node_a.trail_mut().events_mut()[0].payload = json!({"v": 999});

        // B tries to verify A's tampered events — the hash no longer matches
        let result = node_b.verify_remote(node_a_id, node_a.trail().events());
        assert!(matches!(result, FederationVerifyResult::Mismatch { .. }));
    }

    #[test]
    fn federation_proof_export_and_verification() {
        let node_a_id = Uuid::new_v4();
        let node_b_id = Uuid::new_v4();

        let mut node_a = FederatedAuditTrail::new(node_a_id);

        let agent = Uuid::new_v4();

        // Record B's hash and append events
        node_a.update_remote_hash(node_b_id, "hash-from-b-1".to_string());
        node_a
            .append_federated(agent, EventType::StateChange, json!({"seq": 1}))
            .expect("audit append");

        node_a.update_remote_hash(node_b_id, "hash-from-b-2".to_string());
        node_a
            .append_federated(agent, EventType::StateChange, json!({"seq": 2}))
            .expect("audit append");

        // Export proof for full range
        let proof = node_a.export_federation_proof(0, u64::MAX);
        assert_eq!(proof.node_id, node_a_id);
        assert_eq!(proof.cross_references.len(), 2);
        assert!(!proof.proof_digest.is_empty());

        // Verify the proof digest is stable
        let proof2 = node_a.export_federation_proof(0, u64::MAX);
        assert_eq!(proof.proof_digest, proof2.proof_digest);

        // Export with narrow range that should capture nothing
        let empty_proof = node_a.export_federation_proof(0, 0);
        assert!(empty_proof.cross_references.is_empty());
    }

    #[test]
    fn no_cross_references_returns_no_cross_references() {
        let node_a_id = Uuid::new_v4();
        let node_b_id = Uuid::new_v4();

        let node_a = FederatedAuditTrail::new(node_a_id);
        let result = node_a.verify_remote(node_b_id, &[]);
        assert_eq!(result, FederationVerifyResult::NoCrossReferences);
    }
}
