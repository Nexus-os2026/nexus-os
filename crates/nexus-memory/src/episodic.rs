//! Episodic memory — append-only chronicle of agent events.
//!
//! **Invariant #2**: Episodic memory is NEVER deleted and NEVER updated.
//! This module deliberately provides no `delete` or `update` methods.

use chrono::{DateTime, Utc};

use crate::types::{EpisodeType, MemoryContent, MemoryEntry, MemoryError, MemoryId, MemoryType};

/// Append-only episodic memory store.
#[derive(Debug)]
pub struct EpisodicMemory {
    /// Entries in chronological order (oldest first).
    entries: Vec<MemoryEntry>,
    /// Maximum number of episodes.
    max_entries: usize,
}

impl EpisodicMemory {
    /// Creates a new episodic memory with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Appends an episode.  Returns the `MemoryId` of the new entry.
    ///
    /// # Errors
    ///
    /// Returns `TypeMismatch` if `episode.memory_type` is not `Episodic`.
    /// Returns `QuotaExceeded` if the store is full.
    pub fn append(&mut self, episode: MemoryEntry) -> Result<MemoryId, MemoryError> {
        if episode.memory_type != MemoryType::Episodic {
            return Err(MemoryError::TypeMismatch {
                content_type: episode.content.expected_memory_type(),
                declared_type: episode.memory_type,
            });
        }

        if self.entries.len() >= self.max_entries {
            return Err(MemoryError::QuotaExceeded {
                agent_id: episode.agent_id.clone(),
                memory_type: MemoryType::Episodic,
                current: self.entries.len(),
                max: self.max_entries,
            });
        }

        let id = episode.id;
        self.entries.push(episode);
        Ok(id)
    }

    /// Returns the most recent N episodes (newest first).
    pub fn query_recent(&self, limit: usize) -> Vec<&MemoryEntry> {
        self.entries.iter().rev().take(limit).collect()
    }

    /// Returns episodes matching the given event type (newest first).
    pub fn query_by_type(&self, event_type: &EpisodeType, limit: usize) -> Vec<&MemoryEntry> {
        self.entries
            .iter()
            .rev()
            .filter(|e| {
                matches!(&e.content, MemoryContent::Episode { event_type: et, .. } if et == event_type)
            })
            .take(limit)
            .collect()
    }

    /// Returns episodes created since `since` (newest first).
    pub fn query_since(&self, since: DateTime<Utc>) -> Vec<&MemoryEntry> {
        self.entries
            .iter()
            .rev()
            .filter(|e| e.created_at >= since)
            .collect()
    }

    /// Returns episodes associated with a specific task (newest first).
    pub fn query_by_task(&self, task_id: &str) -> Vec<&MemoryEntry> {
        self.entries
            .iter()
            .rev()
            .filter(|e| e.source_task_id.as_deref() == Some(task_id))
            .collect()
    }

    /// Returns the total number of episodes.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if there are no episodes.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns all episodes in chronological order.
    pub fn all(&self) -> &[MemoryEntry] {
        &self.entries
    }

    /// Returns all episodes mutably (used by rollback to tag entries).
    pub fn all_mut(&mut self) -> &mut [MemoryEntry] {
        &mut self.entries
    }

    /// Inserts a pre-built entry (used during restore from persistence).
    pub fn insert_entry(&mut self, entry: MemoryEntry) {
        self.entries.push(entry);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use uuid::Uuid;

    fn make_episode(agent_id: &str, et: EpisodeType, task_id: Option<&str>) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: agent_id.into(),
            memory_type: MemoryType::Episodic,
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content: MemoryContent::Episode {
                event_type: et,
                summary: "test".into(),
                details: serde_json::Value::Null,
                outcome: None,
                duration_ms: None,
            },
            embedding: None,
            created_at: now,
            updated_at: now,
            valid_from: now,
            valid_to: None,
            trust_score: 0.9,
            importance: 0.5,
            confidence: 0.8,
            supersedes: None,
            derived_from: vec![],
            source_task_id: task_id.map(|s| s.to_string()),
            source_conversation_id: None,
            scope: MemoryScope::Agent,
            sensitivity: SensitivityClass::Internal,
            access_count: 0,
            last_accessed: now,
            version: 1,
            ttl: None,
            tags: vec![],
        }
    }

    #[test]
    fn append_and_query_recent() {
        let mut em = EpisodicMemory::new(100);

        let id1 = em
            .append(make_episode("a", EpisodeType::ActionExecuted, None))
            .unwrap();
        let id2 = em
            .append(make_episode("a", EpisodeType::Conversation, None))
            .unwrap();

        assert_eq!(em.len(), 2);

        let recent = em.query_recent(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, id2);

        let recent2 = em.query_recent(10);
        assert_eq!(recent2.len(), 2);
        assert_eq!(recent2[0].id, id2); // newest first
        assert_eq!(recent2[1].id, id1);
    }

    #[test]
    fn query_by_type() {
        let mut em = EpisodicMemory::new(100);

        em.append(make_episode("a", EpisodeType::ActionExecuted, None))
            .unwrap();
        em.append(make_episode("a", EpisodeType::Conversation, None))
            .unwrap();
        em.append(make_episode("a", EpisodeType::ActionExecuted, None))
            .unwrap();

        let actions = em.query_by_type(&EpisodeType::ActionExecuted, 10);
        assert_eq!(actions.len(), 2);

        let convos = em.query_by_type(&EpisodeType::Conversation, 10);
        assert_eq!(convos.len(), 1);
    }

    #[test]
    fn query_by_task() {
        let mut em = EpisodicMemory::new(100);

        em.append(make_episode(
            "a",
            EpisodeType::ActionExecuted,
            Some("task-1"),
        ))
        .unwrap();
        em.append(make_episode("a", EpisodeType::Conversation, Some("task-2")))
            .unwrap();
        em.append(make_episode("a", EpisodeType::GoalAchieved, Some("task-1")))
            .unwrap();

        let task1 = em.query_by_task("task-1");
        assert_eq!(task1.len(), 2);
    }

    #[test]
    fn query_since() {
        let mut em = EpisodicMemory::new(100);

        let before = Utc::now();
        em.append(make_episode("a", EpisodeType::ActionExecuted, None))
            .unwrap();
        em.append(make_episode("a", EpisodeType::Conversation, None))
            .unwrap();

        let results = em.query_since(before);
        assert_eq!(results.len(), 2);

        let future = Utc::now() + chrono::Duration::hours(1);
        let results = em.query_since(future);
        assert!(results.is_empty());
    }

    #[test]
    fn rejects_non_episodic() {
        let mut em = EpisodicMemory::new(100);
        let now = Utc::now();

        let entry = MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "a".into(),
            memory_type: MemoryType::Working, // wrong type
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content: MemoryContent::Context {
                key: "k".into(),
                value: serde_json::Value::Null,
            },
            embedding: None,
            created_at: now,
            updated_at: now,
            valid_from: now,
            valid_to: None,
            trust_score: 0.5,
            importance: 0.5,
            confidence: 0.5,
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

        assert!(matches!(
            em.append(entry),
            Err(MemoryError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn quota_enforcement() {
        let mut em = EpisodicMemory::new(2);

        em.append(make_episode("a", EpisodeType::ActionExecuted, None))
            .unwrap();
        em.append(make_episode("a", EpisodeType::Conversation, None))
            .unwrap();

        let result = em.append(make_episode("a", EpisodeType::GoalAchieved, None));
        assert!(matches!(result, Err(MemoryError::QuotaExceeded { .. })));
    }

    #[test]
    fn all_returns_chronological() {
        let mut em = EpisodicMemory::new(100);

        let id1 = em
            .append(make_episode("a", EpisodeType::ActionExecuted, None))
            .unwrap();
        let id2 = em
            .append(make_episode("a", EpisodeType::Conversation, None))
            .unwrap();

        let all = em.all();
        assert_eq!(all[0].id, id1); // oldest first
        assert_eq!(all[1].id, id2);
    }
}
