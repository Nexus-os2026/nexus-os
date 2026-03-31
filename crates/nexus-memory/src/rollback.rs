//! Event-sourced rollback for the memory subsystem.
//!
//! Checkpoints capture a snapshot of entry IDs and versions.  Rollback restores
//! the memory space to the checkpoint state by soft-deleting post-checkpoint
//! entries and reverting modifications.
//!
//! ## Invariant
//!
//! **Episodic entries are NEVER deleted or modified** (Invariant #2).
//! They are tagged with `in_rolled_back_context` instead.

use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::space::MemorySpace;
use crate::types::*;

/// A checkpoint of memory state that can be rolled back to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCheckpoint {
    /// Unique identifier.
    pub id: Uuid,
    /// Owning agent.
    pub agent_id: String,
    /// When this checkpoint was created.
    pub created_at: chrono::DateTime<Utc>,
    /// Human-readable label (e.g. "before_risky_action").
    pub label: String,
    /// Entry versions at checkpoint time: MemoryId → version number.
    pub entry_versions: HashMap<MemoryId, u32>,
    /// IDs of entries that existed at checkpoint time.
    pub existing_entry_ids: Vec<MemoryId>,
    /// Working memory keys at checkpoint time.
    pub working_keys: Vec<String>,
}

/// Record of a rollback that was performed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackRecord {
    /// Unique identifier.
    pub id: Uuid,
    /// The checkpoint we rolled back to.
    pub checkpoint_id: Uuid,
    /// Owning agent.
    pub agent_id: String,
    /// When rollback was performed.
    pub performed_at: chrono::DateTime<Utc>,
    /// Why rollback was requested.
    pub reason: String,
    /// Entries soft-deleted during rollback.
    pub entries_soft_deleted: Vec<MemoryId>,
    /// Entries reverted to checkpoint version.
    pub entries_reverted: Vec<(MemoryId, u32)>,
    /// Working memory keys cleared.
    pub working_keys_cleared: Vec<String>,
    /// Episodic entries tagged (not deleted — Invariant #2).
    pub episodic_entries_tagged: Vec<MemoryId>,
}

/// Manages checkpoints and rollback for a memory space.
pub struct RollbackManager {
    /// Stored checkpoints, ordered by creation time.
    checkpoints: Vec<MemoryCheckpoint>,
    /// History of performed rollbacks.
    rollback_history: Vec<RollbackRecord>,
    /// Maximum number of checkpoints to keep.
    max_checkpoints: usize,
}

impl RollbackManager {
    /// Creates a new rollback manager.
    pub fn new(max_checkpoints: usize) -> Self {
        Self {
            checkpoints: Vec::new(),
            rollback_history: Vec::new(),
            max_checkpoints,
        }
    }

    /// Creates a checkpoint of the current memory space state.
    pub fn create_checkpoint(
        &mut self,
        agent_id: &str,
        label: &str,
        space: &MemorySpace,
    ) -> MemoryCheckpoint {
        // Collect all entry IDs and versions
        let mut entry_versions = HashMap::new();
        let mut existing_entry_ids = Vec::new();

        for entry in space.working.all() {
            entry_versions.insert(entry.id, entry.version);
            existing_entry_ids.push(entry.id);
        }
        for entry in space.episodic.all() {
            entry_versions.insert(entry.id, entry.version);
            existing_entry_ids.push(entry.id);
        }
        for entry in space.semantic.all() {
            entry_versions.insert(entry.id, entry.version);
            existing_entry_ids.push(entry.id);
        }
        for entry in space.procedural.all_procedures() {
            entry_versions.insert(entry.id, entry.version);
            existing_entry_ids.push(entry.id);
        }

        let working_keys: Vec<String> = space
            .working
            .all()
            .iter()
            .filter_map(|e| e.content.context_key().map(|k| k.to_string()))
            .collect();

        let checkpoint = MemoryCheckpoint {
            id: Uuid::new_v4(),
            agent_id: agent_id.to_string(),
            created_at: Utc::now(),
            label: label.to_string(),
            entry_versions,
            existing_entry_ids,
            working_keys,
        };

        // Enforce max checkpoints
        if self.checkpoints.len() >= self.max_checkpoints {
            self.checkpoints.remove(0);
        }

        self.checkpoints.push(checkpoint.clone());
        checkpoint
    }

    /// Rolls back a memory space to the given checkpoint.
    ///
    /// - Working memory: clears keys written after checkpoint
    /// - Episodic: tags post-checkpoint entries (NEVER deletes — Invariant #2)
    /// - Semantic: soft-deletes post-checkpoint entries
    /// - Procedural: soft-deletes post-checkpoint procedures
    pub fn rollback(
        &mut self,
        checkpoint_id: Uuid,
        reason: &str,
        space: &mut MemorySpace,
    ) -> Result<RollbackRecord, MemoryError> {
        let checkpoint = self
            .checkpoints
            .iter()
            .find(|c| c.id == checkpoint_id)
            .ok_or(MemoryError::CheckpointNotFound(checkpoint_id))?
            .clone();

        let checkpoint_ids: std::collections::HashSet<MemoryId> =
            checkpoint.existing_entry_ids.iter().copied().collect();
        let checkpoint_keys: std::collections::HashSet<&str> =
            checkpoint.working_keys.iter().map(|s| s.as_str()).collect();

        let mut entries_soft_deleted = Vec::new();
        let entries_reverted = Vec::new();
        let mut working_keys_cleared = Vec::new();
        let mut episodic_entries_tagged = Vec::new();

        // ── Working memory: clear post-checkpoint keys ───────────────
        let current_keys: Vec<String> = space
            .working
            .all()
            .iter()
            .filter_map(|e| e.content.context_key().map(|k| k.to_string()))
            .collect();

        for key in &current_keys {
            if !checkpoint_keys.contains(key.as_str()) {
                space.working.remove(key);
                working_keys_cleared.push(key.clone());
            }
        }

        // ── Episodic: tag post-checkpoint entries (NEVER delete) ─────
        for entry in space.episodic.all_mut() {
            if !checkpoint_ids.contains(&entry.id) {
                if !entry.tags.contains(&"in_rolled_back_context".to_string()) {
                    entry.tags.push("in_rolled_back_context".into());
                }
                episodic_entries_tagged.push(entry.id);
            }
        }

        // ── Semantic: soft-delete post-checkpoint entries ────────────
        let semantic_ids: Vec<MemoryId> = space
            .semantic
            .all()
            .iter()
            .filter(|e| !checkpoint_ids.contains(&e.id))
            .map(|e| e.id)
            .collect();

        for id in semantic_ids {
            if space.semantic.soft_delete(id, "rollback").is_ok() {
                entries_soft_deleted.push(id);
            }
        }

        // ── Procedural: soft-delete post-checkpoint procedures ───────
        let proc_ids: Vec<MemoryId> = space
            .procedural
            .all_procedures()
            .iter()
            .filter(|e| !checkpoint_ids.contains(&e.id))
            .map(|e| e.id)
            .collect();

        for id in proc_ids {
            if space.procedural.demote(id, "rollback").is_ok() {
                entries_soft_deleted.push(id);
            }
        }

        // ── Record the rollback ──────────────────────────────────────
        let record = RollbackRecord {
            id: Uuid::new_v4(),
            checkpoint_id,
            agent_id: checkpoint.agent_id.clone(),
            performed_at: Utc::now(),
            reason: reason.to_string(),
            entries_soft_deleted,
            entries_reverted,
            working_keys_cleared,
            episodic_entries_tagged,
        };

        self.rollback_history.push(record.clone());

        // Mark space dirty
        space.mark_dirty();

        Ok(record)
    }

    /// Stores a pre-built checkpoint (used by MemorySpace to avoid self-borrow).
    pub fn store_checkpoint(&mut self, checkpoint: MemoryCheckpoint) {
        if self.checkpoints.len() >= self.max_checkpoints {
            self.checkpoints.remove(0);
        }
        self.checkpoints.push(checkpoint);
    }

    /// Returns a checkpoint by ID.
    pub fn get_checkpoint(&self, id: Uuid) -> Option<&MemoryCheckpoint> {
        self.checkpoints.iter().find(|c| c.id == id)
    }

    /// Lists all checkpoints for an agent.
    pub fn list_checkpoints(&self, agent_id: &str) -> Vec<&MemoryCheckpoint> {
        self.checkpoints
            .iter()
            .filter(|c| c.agent_id == agent_id)
            .collect()
    }

    /// Returns the rollback history.
    pub fn rollback_history(&self) -> &[RollbackRecord] {
        &self.rollback_history
    }

    /// Returns the number of stored checkpoints.
    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::{make_episodic_entry, make_semantic_entry, make_working_entry};

    fn make_space() -> MemorySpace {
        MemorySpace::new("agent-1".into(), MemoryConfig::default())
    }

    #[test]
    fn create_checkpoint_captures_state() {
        let mut space = make_space();
        space
            .write(make_working_entry(
                "agent-1",
                "goal",
                serde_json::json!("build"),
            ))
            .unwrap();
        space
            .write(make_episodic_entry(
                "agent-1",
                EpisodeType::ActionExecuted,
                "did something",
                serde_json::Value::Null,
                None,
                None,
            ))
            .unwrap();

        let mut mgr = RollbackManager::new(10);
        let cp = mgr.create_checkpoint("agent-1", "pre-task", &space);

        assert_eq!(cp.agent_id, "agent-1");
        assert_eq!(cp.label, "pre-task");
        assert_eq!(cp.existing_entry_ids.len(), 2);
        assert!(cp.working_keys.contains(&"goal".to_string()));
    }

    #[test]
    fn rollback_clears_post_checkpoint_working() {
        let mut space = make_space();
        space
            .write(make_working_entry("agent-1", "k1", serde_json::json!(1)))
            .unwrap();

        let mut mgr = RollbackManager::new(10);
        let cp = mgr.create_checkpoint("agent-1", "cp1", &space);

        // Write after checkpoint
        space
            .write(make_working_entry("agent-1", "k2", serde_json::json!(2)))
            .unwrap();
        assert!(space.working.get("k2").is_some());

        // Rollback
        let record = mgr.rollback(cp.id, "test", &mut space).unwrap();
        assert!(space.working.get("k2").is_none(), "k2 should be cleared");
        assert!(space.working.get("k1").is_some(), "k1 should survive");
        assert!(record.working_keys_cleared.contains(&"k2".to_string()));
    }

    #[test]
    fn rollback_does_not_delete_episodic() {
        let mut space = make_space();

        let mut mgr = RollbackManager::new(10);
        let cp = mgr.create_checkpoint("agent-1", "cp1", &space);

        // Write episodic after checkpoint
        space
            .write(make_episodic_entry(
                "agent-1",
                EpisodeType::ActionExecuted,
                "post-checkpoint action",
                serde_json::Value::Null,
                None,
                None,
            ))
            .unwrap();

        let pre_count = space.episodic.len();
        let record = mgr.rollback(cp.id, "test", &mut space).unwrap();

        // Episodic count unchanged (Invariant #2)
        assert_eq!(space.episodic.len(), pre_count);
        assert!(!record.episodic_entries_tagged.is_empty());
    }

    #[test]
    fn rollback_tags_episodic_entries() {
        let mut space = make_space();

        let mut mgr = RollbackManager::new(10);
        let cp = mgr.create_checkpoint("agent-1", "cp1", &space);

        space
            .write(make_episodic_entry(
                "agent-1",
                EpisodeType::ActionExecuted,
                "tagged action",
                serde_json::Value::Null,
                None,
                None,
            ))
            .unwrap();

        mgr.rollback(cp.id, "test", &mut space).unwrap();

        // Check the episodic entry was tagged
        let tagged = space
            .episodic
            .all()
            .iter()
            .any(|e| e.tags.contains(&"in_rolled_back_context".to_string()));
        assert!(tagged, "post-checkpoint episodic entry should be tagged");
    }

    #[test]
    fn rollback_soft_deletes_semantic() {
        let mut space = make_space();

        let mut mgr = RollbackManager::new(10);
        let cp = mgr.create_checkpoint("agent-1", "cp1", &space);

        // Write semantic after checkpoint
        space
            .write(make_semantic_entry(
                "agent-1",
                MemoryContent::Triple {
                    subject: "x".into(),
                    predicate: "is".into(),
                    object: "y".into(),
                },
            ))
            .unwrap();

        let record = mgr.rollback(cp.id, "test", &mut space).unwrap();
        assert!(!record.entries_soft_deleted.is_empty());

        // Semantic entry should be revoked
        let triples = space.semantic.query_triples(Some("x"), None, None);
        assert!(triples.is_empty(), "revoked entries should be filtered");
    }

    #[test]
    fn rollback_soft_deletes_procedures() {
        let mut space = make_space();
        let mut mgr = RollbackManager::new(10);
        let cp = mgr.create_checkpoint("agent-1", "cp1", &space);

        // Write procedure after checkpoint
        let now = Utc::now();
        let proc_entry = MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "agent-1".into(),
            memory_type: MemoryType::Procedural,
            epistemic_class: EpistemicClass::LearnedBehavior {
                evidence_task_ids: vec!["t1".into()],
                success_rate: 0.9,
            },
            validation_state: ValidationState::Corroborated,
            content: MemoryContent::Procedure {
                name: "test-proc".into(),
                description: "test".into(),
                trigger_condition: "trigger".into(),
                steps: vec![],
            },
            embedding: None,
            created_at: now,
            updated_at: now,
            valid_from: now,
            valid_to: None,
            trust_score: 0.9,
            importance: 0.8,
            confidence: 0.9,
            supersedes: None,
            derived_from: vec![],
            source_task_id: None,
            source_conversation_id: None,
            scope: MemoryScope::Agent,
            sensitivity: SensitivityClass::Internal,
            access_count: 0,
            last_accessed: now,
            version: 1,
            ttl: None,
            tags: vec![],
        };
        space.write(proc_entry).unwrap();
        assert_eq!(space.procedural.procedure_count(), 1);

        let record = mgr.rollback(cp.id, "test", &mut space).unwrap();
        assert_eq!(space.procedural.procedure_count(), 0);
        assert!(!record.entries_soft_deleted.is_empty());
    }

    #[test]
    fn rollback_record_documents_changes() {
        let mut space = make_space();
        space
            .write(make_working_entry("agent-1", "k1", serde_json::json!(1)))
            .unwrap();

        let mut mgr = RollbackManager::new(10);
        let cp = mgr.create_checkpoint("agent-1", "cp1", &space);

        space
            .write(make_working_entry("agent-1", "k2", serde_json::json!(2)))
            .unwrap();

        let record = mgr.rollback(cp.id, "bad action", &mut space).unwrap();
        assert_eq!(record.checkpoint_id, cp.id);
        assert_eq!(record.reason, "bad action");
        assert!(!record.working_keys_cleared.is_empty());
    }

    #[test]
    fn max_checkpoints_enforced() {
        let space = make_space();
        let mut mgr = RollbackManager::new(3);

        for i in 0..5 {
            mgr.create_checkpoint("agent-1", &format!("cp{i}"), &space);
        }

        assert_eq!(mgr.checkpoint_count(), 3);
        // Oldest should be removed
        assert!(mgr.list_checkpoints("agent-1")[0].label == "cp2");
    }

    #[test]
    fn cannot_rollback_nonexistent_checkpoint() {
        let mut space = make_space();
        let mut mgr = RollbackManager::new(10);

        let result = mgr.rollback(Uuid::new_v4(), "test", &mut space);
        assert!(matches!(result, Err(MemoryError::CheckpointNotFound(_))));
    }

    #[test]
    fn rollback_history_recorded() {
        let mut space = make_space();
        let mut mgr = RollbackManager::new(10);
        let cp = mgr.create_checkpoint("agent-1", "cp1", &space);

        mgr.rollback(cp.id, "undo", &mut space).unwrap();

        assert_eq!(mgr.rollback_history().len(), 1);
        assert_eq!(mgr.rollback_history()[0].reason, "undo");
    }
}
