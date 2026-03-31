//! Semantic memory — the agent's knowledge base.
//!
//! Stores facts, entities, assertions, and temporal facts with structured
//! query support.  Unlike episodic memory (append-only), semantic memory
//! supports updates, soft-deletion, and version tracking.
//!
//! ## Supported content types
//!
//! - **Triple** — subject-predicate-object relationships
//! - **Assertion** — natural-language statements with citations
//! - **EntityRecord** — entities with typed attributes
//! - **TemporalFact** — facts with temporal validity windows

use chrono::{DateTime, Utc};

use crate::contradiction::{detect_contradictions, Contradiction};
use crate::types::*;

/// Semantic memory store for a single agent.
pub struct SemanticMemory {
    entries: Vec<MemoryEntry>,
    max_entries: usize,
}

impl SemanticMemory {
    /// Creates a new semantic memory store with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Adds a semantic entry to the store.
    ///
    /// Validates that the content is a semantic variant and checks quota.
    pub fn add(&mut self, entry: MemoryEntry) -> Result<MemoryId, MemoryError> {
        // Validate content type
        let expected = entry.content.expected_memory_type();
        if expected != MemoryType::Semantic {
            return Err(MemoryError::TypeMismatch {
                content_type: expected,
                declared_type: MemoryType::Semantic,
            });
        }

        if entry.memory_type != MemoryType::Semantic {
            return Err(MemoryError::TypeMismatch {
                content_type: expected,
                declared_type: entry.memory_type,
            });
        }

        // Quota check
        let active_count = self
            .entries
            .iter()
            .filter(|e| e.validation_state != ValidationState::Revoked)
            .count();
        if active_count >= self.max_entries {
            return Err(MemoryError::QuotaExceeded {
                agent_id: entry.agent_id.clone(),
                memory_type: MemoryType::Semantic,
                current: active_count,
                max: self.max_entries,
            });
        }

        let id = entry.id;
        self.entries.push(entry);
        Ok(id)
    }

    /// Updates the content of a semantic entry.
    ///
    /// Bumps version, updates `updated_at`, returns the **old** entry
    /// (for version tracking by the persistence layer).
    pub fn update(
        &mut self,
        id: MemoryId,
        new_content: MemoryContent,
        new_version: u32,
    ) -> Result<MemoryEntry, MemoryError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or(MemoryError::EntryNotFound(id))?;

        let old = entry.clone();
        entry.content = new_content;
        entry.version = new_version;
        entry.updated_at = Utc::now();

        Ok(old)
    }

    /// Returns a reference to an entry by ID.
    pub fn get(&self, id: MemoryId) -> Option<&MemoryEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Returns a mutable reference to an entry by ID.
    pub fn get_mut(&mut self, id: MemoryId) -> Option<&mut MemoryEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// Queries Triple entries by subject, predicate, and/or object.
    ///
    /// Any parameter can be `None` to act as a wildcard.
    pub fn query_triples(
        &self,
        subject: Option<&str>,
        predicate: Option<&str>,
        object: Option<&str>,
    ) -> Vec<&MemoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.validation_state != ValidationState::Revoked)
            .filter(|e| {
                if let MemoryContent::Triple {
                    subject: s,
                    predicate: p,
                    object: o,
                } = &e.content
                {
                    let s_match = subject.is_none_or(|q| s.eq_ignore_ascii_case(q));
                    let p_match = predicate.is_none_or(|q| p.eq_ignore_ascii_case(q));
                    let o_match = object.is_none_or(|q| o.eq_ignore_ascii_case(q));
                    s_match && p_match && o_match
                } else {
                    false
                }
            })
            .collect()
    }

    /// Queries EntityRecord entries by type and/or name substring.
    pub fn query_entities(
        &self,
        entity_type: Option<&str>,
        name_contains: Option<&str>,
    ) -> Vec<&MemoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.validation_state != ValidationState::Revoked)
            .filter(|e| {
                if let MemoryContent::EntityRecord {
                    name,
                    entity_type: et,
                    ..
                } = &e.content
                {
                    let type_match = entity_type.is_none_or(|q| et.eq_ignore_ascii_case(q));
                    let name_match = name_contains
                        .is_none_or(|q| name.to_lowercase().contains(&q.to_lowercase()));
                    type_match && name_match
                } else {
                    false
                }
            })
            .collect()
    }

    /// Keyword search on Assertion statements.
    pub fn query_assertions(&self, keyword: &str) -> Vec<&MemoryEntry> {
        let kw = keyword.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.validation_state != ValidationState::Revoked)
            .filter(|e| {
                if let MemoryContent::Assertion { statement, .. } = &e.content {
                    statement.to_lowercase().contains(&kw)
                } else {
                    false
                }
            })
            .collect()
    }

    /// Returns TemporalFact entries valid at the given time.
    pub fn query_temporal(&self, at_time: DateTime<Utc>) -> Vec<&MemoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.validation_state != ValidationState::Revoked)
            .filter(|e| {
                if let MemoryContent::TemporalFact {
                    effective_from,
                    effective_to,
                    ..
                } = &e.content
                {
                    *effective_from <= at_time && effective_to.is_none_or(|to| at_time < to)
                } else {
                    false
                }
            })
            .collect()
    }

    /// Detects contradictions between existing entries and a new entry.
    pub fn find_contradictions(&self, new_entry: &MemoryEntry) -> Vec<Contradiction> {
        detect_contradictions(&self.entries, new_entry)
    }

    /// Returns all entries (including revoked, for completeness).
    pub fn all(&self) -> &[MemoryEntry] {
        &self.entries
    }

    /// Returns the number of entries (including revoked).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Soft-deletes an entry by setting its validation state to `Revoked`.
    ///
    /// Does NOT remove the entry from the vector — it remains for audit trail
    /// purposes.  Returns the entry after modification.
    pub fn soft_delete(&mut self, id: MemoryId, _reason: &str) -> Result<MemoryEntry, MemoryError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or(MemoryError::EntryNotFound(id))?;

        entry.validation_state = ValidationState::Revoked;
        entry.updated_at = Utc::now();

        Ok(entry.clone())
    }

    /// Inserts a pre-existing entry directly (used by restore from persistence).
    pub fn insert_entry(&mut self, entry: MemoryEntry) {
        self.entries.push(entry);
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_semantic(content: MemoryContent) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "agent-1".into(),
            memory_type: MemoryType::Semantic,
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content,
            embedding: None,
            created_at: now,
            updated_at: now,
            valid_from: now,
            valid_to: None,
            trust_score: 0.9,
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
        }
    }

    // ── Add and retrieve ─────────────────────────────────────────────────

    #[test]
    fn add_triple_and_query() {
        let mut mem = SemanticMemory::new(100);
        let entry = make_semantic(MemoryContent::Triple {
            subject: "Earth".into(),
            predicate: "orbits".into(),
            object: "Sun".into(),
        });
        let id = mem.add(entry).unwrap();

        let results = mem.query_triples(Some("Earth"), Some("orbits"), None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
    }

    #[test]
    fn query_triples_wildcard() {
        let mut mem = SemanticMemory::new(100);
        mem.add(make_semantic(MemoryContent::Triple {
            subject: "Earth".into(),
            predicate: "orbits".into(),
            object: "Sun".into(),
        }))
        .unwrap();
        mem.add(make_semantic(MemoryContent::Triple {
            subject: "Mars".into(),
            predicate: "orbits".into(),
            object: "Sun".into(),
        }))
        .unwrap();

        // Wildcard subject — should return both
        let results = mem.query_triples(None, Some("orbits"), Some("Sun"));
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn add_entity_and_query_by_type() {
        let mut mem = SemanticMemory::new(100);
        let mut attrs = HashMap::new();
        attrs.insert("version".into(), serde_json::json!("9.0"));

        mem.add(make_semantic(MemoryContent::EntityRecord {
            name: "Nexus".into(),
            entity_type: "Software".into(),
            attributes: attrs,
        }))
        .unwrap();

        let results = mem.query_entities(Some("Software"), None);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn query_entities_by_name() {
        let mut mem = SemanticMemory::new(100);
        mem.add(make_semantic(MemoryContent::EntityRecord {
            name: "Nexus OS".into(),
            entity_type: "Software".into(),
            attributes: HashMap::new(),
        }))
        .unwrap();
        mem.add(make_semantic(MemoryContent::EntityRecord {
            name: "Linux".into(),
            entity_type: "Software".into(),
            attributes: HashMap::new(),
        }))
        .unwrap();

        let results = mem.query_entities(None, Some("nexus"));
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn add_assertion_and_keyword_query() {
        let mut mem = SemanticMemory::new(100);
        mem.add(make_semantic(MemoryContent::Assertion {
            statement: "Rust is a memory-safe systems language".into(),
            citations: vec!["rust-lang.org".into()],
        }))
        .unwrap();

        let results = mem.query_assertions("memory-safe");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn add_temporal_fact_and_query_at_time() {
        let mut mem = SemanticMemory::new(100);
        let t1 = Utc::now() - chrono::Duration::days(10);
        let t2 = Utc::now() + chrono::Duration::days(10);

        mem.add(make_semantic(MemoryContent::TemporalFact {
            statement: "CEO is Alice".into(),
            effective_from: t1,
            effective_to: Some(t2),
            context: "leadership".into(),
        }))
        .unwrap();

        // Now is within range
        let results = mem.query_temporal(Utc::now());
        assert_eq!(results.len(), 1);

        // Far future is outside range
        let results = mem.query_temporal(Utc::now() + chrono::Duration::days(20));
        assert!(results.is_empty());
    }

    #[test]
    fn temporal_open_ended_valid() {
        let mut mem = SemanticMemory::new(100);
        let t1 = Utc::now() - chrono::Duration::days(5);

        mem.add(make_semantic(MemoryContent::TemporalFact {
            statement: "CTO is Bob".into(),
            effective_from: t1,
            effective_to: None, // open-ended
            context: "leadership".into(),
        }))
        .unwrap();

        let results = mem.query_temporal(Utc::now() + chrono::Duration::days(365));
        assert_eq!(results.len(), 1);
    }

    // ── Quota ────────────────────────────────────────────────────────────

    #[test]
    fn quota_enforcement() {
        let mut mem = SemanticMemory::new(2);
        mem.add(make_semantic(MemoryContent::Triple {
            subject: "a".into(),
            predicate: "b".into(),
            object: "c".into(),
        }))
        .unwrap();
        mem.add(make_semantic(MemoryContent::Triple {
            subject: "d".into(),
            predicate: "e".into(),
            object: "f".into(),
        }))
        .unwrap();

        let result = mem.add(make_semantic(MemoryContent::Triple {
            subject: "g".into(),
            predicate: "h".into(),
            object: "i".into(),
        }));
        assert!(matches!(result, Err(MemoryError::QuotaExceeded { .. })));
    }

    // ── Soft delete ──────────────────────────────────────────────────────

    #[test]
    fn soft_delete_sets_revoked() {
        let mut mem = SemanticMemory::new(100);
        let entry = make_semantic(MemoryContent::Triple {
            subject: "x".into(),
            predicate: "y".into(),
            object: "z".into(),
        });
        let id = mem.add(entry).unwrap();

        let deleted = mem.soft_delete(id, "outdated").unwrap();
        assert_eq!(deleted.validation_state, ValidationState::Revoked);

        // Revoked entries are excluded from queries
        let results = mem.query_triples(Some("x"), None, None);
        assert!(results.is_empty());
    }

    #[test]
    fn soft_delete_nonexistent_fails() {
        let mut mem = SemanticMemory::new(100);
        let result = mem.soft_delete(Uuid::new_v4(), "gone");
        assert!(matches!(result, Err(MemoryError::EntryNotFound(_))));
    }

    // ── Update ───────────────────────────────────────────────────────────

    #[test]
    fn update_bumps_version_and_returns_old() {
        let mut mem = SemanticMemory::new(100);
        let entry = make_semantic(MemoryContent::Triple {
            subject: "x".into(),
            predicate: "is".into(),
            object: "old".into(),
        });
        let id = mem.add(entry).unwrap();

        let old = mem
            .update(
                id,
                MemoryContent::Triple {
                    subject: "x".into(),
                    predicate: "is".into(),
                    object: "new".into(),
                },
                2,
            )
            .unwrap();

        // Old entry had version 1 and old content
        assert_eq!(old.version, 1);
        if let MemoryContent::Triple { object, .. } = &old.content {
            assert_eq!(object, "old");
        } else {
            panic!("expected Triple");
        }

        // Current entry has version 2 and new content
        let current = mem.get(id).unwrap();
        assert_eq!(current.version, 2);
        if let MemoryContent::Triple { object, .. } = &current.content {
            assert_eq!(object, "new");
        } else {
            panic!("expected Triple");
        }
    }

    // ── Rejects non-semantic ─────────────────────────────────────────────

    #[test]
    fn rejects_non_semantic_content() {
        let mut mem = SemanticMemory::new(100);
        let now = Utc::now();
        let entry = MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "a".into(),
            memory_type: MemoryType::Semantic,
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content: MemoryContent::Context {
                key: "k".into(),
                value: serde_json::json!(1),
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
            mem.add(entry),
            Err(MemoryError::TypeMismatch { .. })
        ));
    }

    // ── Contradiction integration ────────────────────────────────────────

    #[test]
    fn find_contradictions_through_semantic_memory() {
        let mut mem = SemanticMemory::new(100);
        mem.add(make_semantic(MemoryContent::Triple {
            subject: "Earth".into(),
            predicate: "shape".into(),
            object: "sphere".into(),
        }))
        .unwrap();

        let new = make_semantic(MemoryContent::Triple {
            subject: "Earth".into(),
            predicate: "shape".into(),
            object: "flat".into(),
        });

        let contradictions = mem.find_contradictions(&new);
        assert_eq!(contradictions.len(), 1);
    }
}
