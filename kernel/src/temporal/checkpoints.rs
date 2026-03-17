//! Temporal checkpoints — integration with the Time Machine for fork rollback.
//!
//! Before forking, the engine saves agent consciousness states and decision
//! context.  If a selected timeline fails in practice, we can roll back and
//! pick a different fork.

use crate::temporal::types::{TemporalCheckpoint, TemporalError};
use std::collections::HashMap;

/// Manages temporal checkpoints for fork-and-rollback workflows.
#[derive(Debug, Clone)]
pub struct TemporalCheckpointManager {
    checkpoints: Vec<TemporalCheckpoint>,
    max_checkpoints: usize,
}

impl Default for TemporalCheckpointManager {
    fn default() -> Self {
        Self {
            checkpoints: Vec::new(),
            max_checkpoints: 100,
        }
    }
}

impl TemporalCheckpointManager {
    pub fn new(max_checkpoints: usize) -> Self {
        Self {
            checkpoints: Vec::new(),
            max_checkpoints,
        }
    }

    /// Create a pre-fork checkpoint, capturing agent states before diverging.
    pub fn create_pre_fork_checkpoint(
        &mut self,
        fork_id: &str,
        decision_context: &str,
        agent_states: HashMap<String, serde_json::Value>,
    ) -> Result<TemporalCheckpoint, TemporalError> {
        let mut checkpoint = TemporalCheckpoint::new(fork_id, decision_context);
        checkpoint.agent_states = agent_states;

        self.checkpoints.push(checkpoint.clone());

        // Evict oldest if over capacity
        while self.checkpoints.len() > self.max_checkpoints {
            self.checkpoints.remove(0);
        }

        Ok(checkpoint)
    }

    /// Retrieve a checkpoint by ID for rollback.
    pub fn get_checkpoint(&self, checkpoint_id: &str) -> Option<&TemporalCheckpoint> {
        self.checkpoints
            .iter()
            .find(|c| c.checkpoint_id == checkpoint_id)
    }

    /// Retrieve a checkpoint by fork ID.
    pub fn get_by_fork(&self, fork_id: &str) -> Option<&TemporalCheckpoint> {
        self.checkpoints.iter().find(|c| c.fork_id == fork_id)
    }

    /// Rollback: return the saved agent states so the caller can restore them.
    pub fn rollback(&self, checkpoint_id: &str) -> Result<&TemporalCheckpoint, TemporalError> {
        self.checkpoints
            .iter()
            .find(|c| c.checkpoint_id == checkpoint_id)
            .ok_or_else(|| {
                TemporalError::CheckpointError(format!("checkpoint not found: {checkpoint_id}"))
            })
    }

    /// Remove a checkpoint after it is no longer needed (timeline committed).
    pub fn remove_checkpoint(&mut self, checkpoint_id: &str) -> bool {
        let before = self.checkpoints.len();
        self.checkpoints
            .retain(|c| c.checkpoint_id != checkpoint_id);
        self.checkpoints.len() < before
    }

    /// List all checkpoints.
    pub fn list_checkpoints(&self) -> &[TemporalCheckpoint] {
        &self.checkpoints
    }

    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn create_and_retrieve_checkpoint() {
        let mut mgr = TemporalCheckpointManager::default();
        let mut states = HashMap::new();
        states.insert("agent-1".into(), json!({"confidence": 0.7}));

        let cp = mgr
            .create_pre_fork_checkpoint("fork-1", "deploying", states)
            .unwrap();

        assert!(!cp.checkpoint_id.is_empty());
        assert_eq!(cp.fork_id, "fork-1");
        assert!(cp.agent_states.contains_key("agent-1"));

        // Retrieve by ID
        let found = mgr.get_checkpoint(&cp.checkpoint_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().fork_id, "fork-1");
    }

    #[test]
    fn retrieve_by_fork() {
        let mut mgr = TemporalCheckpointManager::default();
        mgr.create_pre_fork_checkpoint("fork-A", "ctx", HashMap::new())
            .unwrap();
        mgr.create_pre_fork_checkpoint("fork-B", "ctx", HashMap::new())
            .unwrap();

        let found = mgr.get_by_fork("fork-B");
        assert!(found.is_some());
        assert_eq!(found.unwrap().fork_id, "fork-B");
    }

    #[test]
    fn rollback_returns_states() {
        let mut mgr = TemporalCheckpointManager::default();
        let mut states = HashMap::new();
        states.insert("agent-1".into(), json!({"urgency": 0.9}));

        let cp = mgr
            .create_pre_fork_checkpoint("fork-1", "pre-deploy", states)
            .unwrap();

        let rolled = mgr.rollback(&cp.checkpoint_id).unwrap();
        assert_eq!(rolled.agent_states["agent-1"]["urgency"], 0.9);
    }

    #[test]
    fn rollback_not_found() {
        let mgr = TemporalCheckpointManager::default();
        let result = mgr.rollback("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn remove_checkpoint() {
        let mut mgr = TemporalCheckpointManager::default();
        let cp = mgr
            .create_pre_fork_checkpoint("fork-1", "ctx", HashMap::new())
            .unwrap();

        assert_eq!(mgr.checkpoint_count(), 1);
        assert!(mgr.remove_checkpoint(&cp.checkpoint_id));
        assert_eq!(mgr.checkpoint_count(), 0);
        assert!(!mgr.remove_checkpoint("nonexistent"));
    }

    #[test]
    fn eviction_on_capacity() {
        let mut mgr = TemporalCheckpointManager::new(3);
        for i in 0..5 {
            mgr.create_pre_fork_checkpoint(&format!("fork-{i}"), "ctx", HashMap::new())
                .unwrap();
        }
        assert_eq!(mgr.checkpoint_count(), 3);
        // Oldest (fork-0, fork-1) should be evicted
        assert!(mgr.get_by_fork("fork-0").is_none());
        assert!(mgr.get_by_fork("fork-1").is_none());
        assert!(mgr.get_by_fork("fork-4").is_some());
    }

    #[test]
    fn list_checkpoints() {
        let mut mgr = TemporalCheckpointManager::default();
        mgr.create_pre_fork_checkpoint("f1", "c", HashMap::new())
            .unwrap();
        mgr.create_pre_fork_checkpoint("f2", "c", HashMap::new())
            .unwrap();
        assert_eq!(mgr.list_checkpoints().len(), 2);
    }
}
