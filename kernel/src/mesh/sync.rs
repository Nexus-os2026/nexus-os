//! Consciousness synchronisation — share agent states across mesh instances.

use super::MeshError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Tracks the synchronisation state for a single agent across the mesh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub agent_id: String,
    pub state_hash: String,
    pub vector_clock: HashMap<String, u64>,
    pub last_sync: u64,
}

/// A single field-level change to be applied to a remote agent state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDelta {
    pub agent_id: String,
    pub field: String,
    pub old_value: serde_json::Value,
    pub new_value: serde_json::Value,
    pub timestamp: u64,
}

/// Manages consciousness synchronisation between mesh peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsciousnessSync {
    local_peer_id: String,
    states: HashMap<String, SyncState>,
    pending_deltas: Vec<SyncDelta>,
}

impl ConsciousnessSync {
    /// Create a new sync manager for the given local peer.
    pub fn new(local_peer_id: String) -> Self {
        Self {
            local_peer_id,
            states: HashMap::new(),
            pending_deltas: Vec::new(),
        }
    }

    /// Register or update the sync state for an agent.
    pub fn sync_agent_state(
        &mut self,
        agent_id: &str,
        state_data: &serde_json::Value,
        timestamp: u64,
    ) -> Result<SyncState, MeshError> {
        let hash = Self::compute_hash(state_data);
        let mut clock = self
            .states
            .get(agent_id)
            .map(|s| s.vector_clock.clone())
            .unwrap_or_default();

        let counter = clock.entry(self.local_peer_id.clone()).or_insert(0);
        *counter += 1;

        let state = SyncState {
            agent_id: agent_id.to_string(),
            state_hash: hash,
            vector_clock: clock,
            last_sync: timestamp,
        };
        self.states.insert(agent_id.to_string(), state.clone());
        Ok(state)
    }

    /// Apply a delta from a remote peer.
    pub fn apply_delta(&mut self, delta: SyncDelta) -> Result<(), MeshError> {
        // Validate agent is tracked
        if !self.states.contains_key(&delta.agent_id) {
            // Auto-register with an empty state
            let state = SyncState {
                agent_id: delta.agent_id.clone(),
                state_hash: String::new(),
                vector_clock: HashMap::new(),
                last_sync: delta.timestamp,
            };
            self.states.insert(delta.agent_id.clone(), state);
        }

        if let Some(state) = self.states.get_mut(&delta.agent_id) {
            state.last_sync = delta.timestamp;
        }

        self.pending_deltas.push(delta);
        Ok(())
    }

    /// Get the current sync status for an agent.
    pub fn get_sync_status(&self, agent_id: &str) -> Result<&SyncState, MeshError> {
        self.states
            .get(agent_id)
            .ok_or_else(|| MeshError::SyncConflict(agent_id.into(), "agent not tracked".into()))
    }

    /// Resolve a conflict between two sync states using vector clock comparison.
    ///
    /// If neither state dominates the other, the most-recently-synced state wins
    /// (last-writer-wins).
    pub fn resolve_conflict(
        &self,
        local: &SyncState,
        remote: &SyncState,
    ) -> Result<SyncState, MeshError> {
        if local.agent_id != remote.agent_id {
            return Err(MeshError::SyncConflict(
                local.agent_id.clone(),
                "agent id mismatch".into(),
            ));
        }

        // Check vector clock dominance
        let local_dominates = Self::clock_dominates(&local.vector_clock, &remote.vector_clock);
        let remote_dominates = Self::clock_dominates(&remote.vector_clock, &local.vector_clock);

        if local_dominates {
            Ok(local.clone())
        } else if remote_dominates {
            Ok(remote.clone())
        } else {
            // Concurrent — last-writer-wins by timestamp
            if local.last_sync >= remote.last_sync {
                Ok(local.clone())
            } else {
                Ok(remote.clone())
            }
        }
    }

    /// Return pending deltas (useful for replication to other peers).
    pub fn drain_pending_deltas(&mut self) -> Vec<SyncDelta> {
        std::mem::take(&mut self.pending_deltas)
    }

    // --- helpers ---

    fn compute_hash(data: &serde_json::Value) -> String {
        let bytes = serde_json::to_vec(data).unwrap_or_default();
        let digest = Sha256::digest(&bytes);
        format!("{:x}", digest)
    }

    /// Returns true when every entry in `a` is >= the corresponding entry in `b`.
    fn clock_dominates(a: &HashMap<String, u64>, b: &HashMap<String, u64>) -> bool {
        for (key, &b_val) in b {
            let a_val = a.get(key).copied().unwrap_or(0);
            if a_val < b_val {
                return false;
            }
        }
        // `a` must also have at least one entry strictly greater
        let mut any_greater = false;
        for (key, &a_val) in a {
            let b_val = b.get(key).copied().unwrap_or(0);
            if a_val > b_val {
                any_greater = true;
            }
        }
        any_greater
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_agent_state_increments_clock() {
        let mut sync = ConsciousnessSync::new("peer-a".into());
        let data = serde_json::json!({"mood": "curious"});

        let s1 = sync.sync_agent_state("agent-1", &data, 100).unwrap();
        assert_eq!(s1.vector_clock.get("peer-a"), Some(&1));

        let s2 = sync.sync_agent_state("agent-1", &data, 200).unwrap();
        assert_eq!(s2.vector_clock.get("peer-a"), Some(&2));
    }

    #[test]
    fn apply_delta_auto_registers() {
        let mut sync = ConsciousnessSync::new("peer-a".into());
        let delta = SyncDelta {
            agent_id: "agent-x".into(),
            field: "mood".into(),
            old_value: serde_json::json!("calm"),
            new_value: serde_json::json!("excited"),
            timestamp: 500,
        };
        sync.apply_delta(delta).unwrap();
        assert!(sync.get_sync_status("agent-x").is_ok());
    }

    #[test]
    fn get_sync_status_unknown_agent() {
        let sync = ConsciousnessSync::new("peer-a".into());
        assert!(sync.get_sync_status("nope").is_err());
    }

    #[test]
    fn resolve_conflict_local_dominates() {
        let local = SyncState {
            agent_id: "a1".into(),
            state_hash: "aaa".into(),
            vector_clock: [("p1".into(), 3), ("p2".into(), 2)].into(),
            last_sync: 100,
        };
        let remote = SyncState {
            agent_id: "a1".into(),
            state_hash: "bbb".into(),
            vector_clock: [("p1".into(), 2), ("p2".into(), 1)].into(),
            last_sync: 200,
        };
        let sync = ConsciousnessSync::new("p1".into());
        let winner = sync.resolve_conflict(&local, &remote).unwrap();
        assert_eq!(winner.state_hash, "aaa");
    }

    #[test]
    fn resolve_conflict_concurrent_last_writer_wins() {
        let local = SyncState {
            agent_id: "a1".into(),
            state_hash: "aaa".into(),
            vector_clock: [("p1".into(), 2)].into(),
            last_sync: 100,
        };
        let remote = SyncState {
            agent_id: "a1".into(),
            state_hash: "bbb".into(),
            vector_clock: [("p2".into(), 2)].into(),
            last_sync: 200,
        };
        let sync = ConsciousnessSync::new("p1".into());
        let winner = sync.resolve_conflict(&local, &remote).unwrap();
        // remote has higher timestamp
        assert_eq!(winner.state_hash, "bbb");
    }

    #[test]
    fn resolve_conflict_mismatched_agent() {
        let a = SyncState {
            agent_id: "a1".into(),
            state_hash: "".into(),
            vector_clock: HashMap::new(),
            last_sync: 0,
        };
        let b = SyncState {
            agent_id: "a2".into(),
            state_hash: "".into(),
            vector_clock: HashMap::new(),
            last_sync: 0,
        };
        let sync = ConsciousnessSync::new("p1".into());
        assert!(sync.resolve_conflict(&a, &b).is_err());
    }

    #[test]
    fn drain_pending_deltas() {
        let mut sync = ConsciousnessSync::new("peer-a".into());
        let delta = SyncDelta {
            agent_id: "agent-1".into(),
            field: "energy".into(),
            old_value: serde_json::json!(50),
            new_value: serde_json::json!(80),
            timestamp: 1000,
        };
        sync.apply_delta(delta).unwrap();
        let drained = sync.drain_pending_deltas();
        assert_eq!(drained.len(), 1);
        assert!(sync.drain_pending_deltas().is_empty());
    }

    #[test]
    fn sync_state_serde_roundtrip() {
        let state = SyncState {
            agent_id: "a1".into(),
            state_hash: "abc".into(),
            vector_clock: [("p1".into(), 5)].into(),
            last_sync: 999,
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: SyncState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.agent_id, "a1");
        assert_eq!(back.vector_clock.get("p1"), Some(&5));
    }
}
