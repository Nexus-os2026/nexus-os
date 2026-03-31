//! Working memory — the agent's scratch space for the current task.
//!
//! Working memory is a key-value store with a hard entry limit.  Entries are
//! keyed by a string context key and automatically classified as
//! `EpistemicClass::Observation` (the agent wrote it, highest trust).

use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

use crate::types::{
    EpistemicClass, MemoryContent, MemoryEntry, MemoryError, MemoryScope, MemoryType,
    SensitivityClass, ValidationState,
};

/// In-memory working memory with a hard entry limit.
#[derive(Debug)]
pub struct WorkingMemory {
    /// Entries keyed by context key.
    entries: HashMap<String, MemoryEntry>,
    /// Maximum number of concurrent entries.
    max_entries: usize,
}

impl WorkingMemory {
    /// Creates a new working memory with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
        }
    }

    /// Sets a context key-value pair, creating or updating the entry.
    ///
    /// Returns the created/updated `MemoryEntry`.  If the key already exists,
    /// the entry is updated in place (version incremented).  If the store is
    /// full and the key is new, returns `QuotaExceeded`.
    pub fn set(
        &mut self,
        key: &str,
        value: serde_json::Value,
        agent_id: &str,
    ) -> Result<MemoryEntry, MemoryError> {
        let now = Utc::now();

        if let Some(existing) = self.entries.get_mut(key) {
            existing.content = MemoryContent::Context {
                key: key.to_string(),
                value,
            };
            existing.updated_at = now;
            existing.version += 1;
            return Ok(existing.clone());
        }

        // New key — check quota
        if self.entries.len() >= self.max_entries {
            return Err(MemoryError::QuotaExceeded {
                agent_id: agent_id.to_string(),
                memory_type: MemoryType::Working,
                current: self.entries.len(),
                max: self.max_entries,
            });
        }

        let entry = MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: agent_id.to_string(),
            memory_type: MemoryType::Working,
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content: MemoryContent::Context {
                key: key.to_string(),
                value,
            },
            embedding: None,
            created_at: now,
            updated_at: now,
            valid_from: now,
            valid_to: None,
            trust_score: EpistemicClass::Observation.default_trust(),
            importance: 0.5,
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

        self.entries.insert(key.to_string(), entry.clone());
        Ok(entry)
    }

    /// Gets a working memory entry by context key.
    pub fn get(&self, key: &str) -> Option<&MemoryEntry> {
        self.entries.get(key)
    }

    /// Gets a mutable working memory entry by context key.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut MemoryEntry> {
        self.entries.get_mut(key)
    }

    /// Removes and returns a working memory entry.
    pub fn remove(&mut self, key: &str) -> Option<MemoryEntry> {
        self.entries.remove(key)
    }

    /// Clears all working memory (e.g. on task completion).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns all working memory entries.
    pub fn all(&self) -> Vec<&MemoryEntry> {
        self.entries.values().collect()
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if working memory is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Inserts a pre-built entry (used during restore from persistence).
    pub fn insert_entry(&mut self, entry: MemoryEntry) -> Result<(), MemoryError> {
        let key = match &entry.content {
            MemoryContent::Context { key, .. } => key.clone(),
            _ => {
                return Err(MemoryError::TypeMismatch {
                    content_type: entry.content.expected_memory_type(),
                    declared_type: MemoryType::Working,
                });
            }
        };
        self.entries.insert(key, entry);
        Ok(())
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_remove() {
        let mut wm = WorkingMemory::new(10);
        let entry = wm
            .set("goal", serde_json::json!("build stuff"), "agent-1")
            .unwrap();
        assert_eq!(entry.memory_type, MemoryType::Working);
        assert_eq!(entry.version, 1);

        let got = wm.get("goal").unwrap();
        assert_eq!(got.id, entry.id);

        let removed = wm.remove("goal").unwrap();
        assert_eq!(removed.id, entry.id);
        assert!(wm.get("goal").is_none());
    }

    #[test]
    fn update_increments_version() {
        let mut wm = WorkingMemory::new(10);
        wm.set("k", serde_json::json!(1), "a").unwrap();
        let updated = wm.set("k", serde_json::json!(2), "a").unwrap();
        assert_eq!(updated.version, 2);
    }

    #[test]
    fn quota_enforcement() {
        let mut wm = WorkingMemory::new(2);
        wm.set("a", serde_json::json!(1), "agent").unwrap();
        wm.set("b", serde_json::json!(2), "agent").unwrap();

        let result = wm.set("c", serde_json::json!(3), "agent");
        assert!(matches!(result, Err(MemoryError::QuotaExceeded { .. })));
    }

    #[test]
    fn quota_allows_update_when_full() {
        let mut wm = WorkingMemory::new(1);
        wm.set("a", serde_json::json!(1), "agent").unwrap();
        // Update existing key should succeed even at capacity
        let result = wm.set("a", serde_json::json!(2), "agent");
        assert!(result.is_ok());
    }

    #[test]
    fn clear_removes_all() {
        let mut wm = WorkingMemory::new(10);
        wm.set("a", serde_json::json!(1), "agent").unwrap();
        wm.set("b", serde_json::json!(2), "agent").unwrap();
        assert_eq!(wm.len(), 2);

        wm.clear();
        assert!(wm.is_empty());
    }

    #[test]
    fn all_returns_entries() {
        let mut wm = WorkingMemory::new(10);
        wm.set("x", serde_json::json!(1), "a").unwrap();
        wm.set("y", serde_json::json!(2), "a").unwrap();
        assert_eq!(wm.all().len(), 2);
    }

    #[test]
    fn entries_have_correct_defaults() {
        let mut wm = WorkingMemory::new(10);
        let entry = wm.set("k", serde_json::Value::Null, "agent-1").unwrap();
        assert_eq!(entry.epistemic_class, EpistemicClass::Observation);
        assert_eq!(entry.validation_state, ValidationState::Unverified);
        assert_eq!(entry.scope, MemoryScope::Agent);
        assert_eq!(entry.sensitivity, SensitivityClass::Internal);
        assert_eq!(entry.access_count, 0);
    }
}
