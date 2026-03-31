//! Per-agent memory container.
//!
//! A `MemorySpace` holds all memory types for a single agent.  Phase 1
//! includes Working and Episodic memory; Phase 2 adds Semantic memory;
//! Procedural is added in Phase 3.
//!
//! ACL and audit happen at the `MemoryManager` level — `MemorySpace` trusts
//! that the caller has already validated access.

use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;
use uuid::Uuid;

use crate::acl::MemoryAcl;
use crate::contradiction::{Contradiction, ContradictionResolution};
use crate::episodic::EpisodicMemory;
use crate::procedural::ProceduralMemory;
use crate::rollback::RollbackManager;
use crate::semantic::SemanticMemory;
use crate::types::{
    EpistemicClass, MemoryConfig, MemoryContent, MemoryEntry, MemoryError, MemoryId, MemoryQuery,
    MemoryScope, MemoryType, MemoryUsage, SensitivityClass, ValidationState,
};
use crate::working::WorkingMemory;

/// Per-agent memory container.
pub struct MemorySpace {
    /// Owning agent identifier.
    pub agent_id: String,
    /// Working memory (key-value scratch space).
    pub working: WorkingMemory,
    /// Episodic memory (append-only chronicle).
    pub episodic: EpisodicMemory,
    /// Semantic memory (knowledge base) — Phase 2.
    pub semantic: SemanticMemory,
    /// Procedural memory (learned behaviors) — Phase 3.
    pub procedural: ProceduralMemory,
    /// Access control list — Phase 4.
    pub acl: MemoryAcl,
    /// Checkpoint/rollback manager — Phase 3.
    pub rollback_mgr: RollbackManager,
    /// Configuration limits.
    config: MemoryConfig,
    /// Whether any write has occurred since the last checkpoint.
    dirty: AtomicBool,
}

impl MemorySpace {
    /// Creates a new memory space for the given agent.
    pub fn new(agent_id: String, config: MemoryConfig) -> Self {
        let acl = MemoryAcl::new(agent_id.clone(), 4); // default L4 autonomy
        Self {
            working: WorkingMemory::new(config.max_working_entries),
            episodic: EpisodicMemory::new(config.max_episodic_entries),
            semantic: SemanticMemory::new(config.max_semantic_entries),
            procedural: ProceduralMemory::new(config.max_procedural_entries),
            acl,
            rollback_mgr: RollbackManager::new(50),
            config,
            agent_id,
            dirty: AtomicBool::new(false),
        }
    }

    /// The main write pipeline.
    ///
    /// 1. Validates schema (content variant matches declared `memory_type`).
    /// 2. Checks quota.
    /// 3. Routes to the correct memory store.
    /// 4. Marks dirty for persistence.
    /// 5. Returns the `MemoryId`.
    pub fn write(&mut self, entry: MemoryEntry) -> Result<MemoryId, MemoryError> {
        // 1. Validate schema: content type must match declared memory type
        let expected = entry.content.expected_memory_type();
        if expected != entry.memory_type {
            return Err(MemoryError::TypeMismatch {
                content_type: expected,
                declared_type: entry.memory_type,
            });
        }

        // 2-3. Route to correct store (quota checked inside each store)
        let id = match entry.memory_type {
            MemoryType::Working => {
                let key = entry
                    .content
                    .context_key()
                    .ok_or_else(|| {
                        MemoryError::ValidationError(
                            "Working memory content must have a context key".into(),
                        )
                    })?
                    .to_string();
                let value = match &entry.content {
                    MemoryContent::Context { value, .. } => value.clone(),
                    _ => unreachable!(),
                };
                let written = self.working.set(&key, value, &self.agent_id)?;
                written.id
            }
            MemoryType::Episodic => self.episodic.append(entry)?,
            MemoryType::Semantic => {
                // Detect and resolve contradictions before writing
                let contradictions = self.semantic.find_contradictions(&entry);
                for c in &contradictions {
                    self.apply_contradiction_resolution(c)?;
                }

                // If any contradiction recommended Supersede, set supersedes on new entry
                let mut entry = entry;
                for c in &contradictions {
                    if matches!(
                        c.recommended_resolution,
                        ContradictionResolution::Supersede { .. }
                    ) {
                        entry.supersedes = Some(c.existing_entry_id);
                    }
                    if matches!(
                        c.recommended_resolution,
                        ContradictionResolution::FlagForReview { .. }
                    ) {
                        entry.validation_state = ValidationState::Contested;
                        if !entry.tags.contains(&"has_contradiction".to_string()) {
                            entry.tags.push("has_contradiction".into());
                        }
                    }
                    if matches!(
                        c.recommended_resolution,
                        ContradictionResolution::CoexistWithContext { .. }
                    ) && !entry.tags.contains(&"has_contradiction".to_string())
                    {
                        entry.tags.push("has_contradiction".into());
                    }
                }

                self.semantic.add(entry)?
            }
            MemoryType::Procedural => self.procedural.add_procedure(entry)?,
        };

        // 4. Mark dirty
        self.dirty.store(true, Ordering::Release);

        Ok(id)
    }

    /// Queries across available memory types.
    pub fn query(&self, query: &MemoryQuery) -> Result<Vec<MemoryEntry>, MemoryError> {
        let mut results = Vec::new();
        let now = Utc::now();

        let types = query.memory_types.as_deref().unwrap_or(&[
            MemoryType::Working,
            MemoryType::Episodic,
            MemoryType::Semantic,
            MemoryType::Procedural,
        ]);

        for mt in types {
            match mt {
                MemoryType::Working => {
                    for entry in self.working.all() {
                        if self.matches_query(entry, query, now) {
                            results.push(entry.clone());
                        }
                    }
                }
                MemoryType::Episodic => {
                    for entry in self.episodic.all() {
                        if self.matches_query(entry, query, now) {
                            results.push(entry.clone());
                        }
                    }
                }
                MemoryType::Semantic => {
                    for entry in self.semantic.all() {
                        if self.matches_query(entry, query, now) {
                            results.push(entry.clone());
                        }
                    }
                }
                MemoryType::Procedural => {
                    for entry in self.procedural.all_procedures() {
                        if self.matches_query(entry, query, now) {
                            results.push(entry.clone());
                        }
                    }
                }
            }
        }

        // Sort by created_at descending (newest first)
        results.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // Apply limit
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    /// Returns usage statistics for this agent.
    pub fn usage(&self) -> MemoryUsage {
        MemoryUsage {
            agent_id: self.agent_id.clone(),
            working_count: self.working.len() as u64,
            episodic_count: self.episodic.len() as u64,
            semantic_count: self.semantic.len() as u64,
            procedural_count: self.procedural.procedure_count() as u64,
            total_size_bytes: 0, // placeholder — accurate sizing requires serialization
        }
    }

    /// Clears all working memory.
    pub fn clear_working(&mut self) {
        self.working.clear();
        self.dirty.store(true, Ordering::Release);
    }

    /// Returns `true` if any write has occurred since the last checkpoint.
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Acquire)
    }

    /// Marks the space as clean (after a successful checkpoint).
    pub fn mark_clean(&self) {
        self.dirty.store(false, Ordering::Release);
    }

    /// Marks the space as dirty (after rollback or external modification).
    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Release);
    }

    /// Returns a reference to the configuration.
    pub fn config(&self) -> &MemoryConfig {
        &self.config
    }

    /// Creates a checkpoint of the current state.
    ///
    /// Convenience wrapper that avoids self-borrow issues with
    /// `rollback_mgr.create_checkpoint(...)`.
    pub fn create_checkpoint(&mut self, label: &str) -> crate::rollback::MemoryCheckpoint {
        // Split borrow: take rollback_mgr out, pass self to it, put it back
        // We can't call rollback_mgr.create_checkpoint(&self) because self
        // is borrowed mutably. Instead, collect state snapshot first.
        let agent_id = self.agent_id.clone();
        let mut entry_versions = std::collections::HashMap::new();
        let mut existing_entry_ids = Vec::new();

        for entry in self.working.all() {
            entry_versions.insert(entry.id, entry.version);
            existing_entry_ids.push(entry.id);
        }
        for entry in self.episodic.all() {
            entry_versions.insert(entry.id, entry.version);
            existing_entry_ids.push(entry.id);
        }
        for entry in self.semantic.all() {
            entry_versions.insert(entry.id, entry.version);
            existing_entry_ids.push(entry.id);
        }
        for entry in self.procedural.all_procedures() {
            entry_versions.insert(entry.id, entry.version);
            existing_entry_ids.push(entry.id);
        }

        let working_keys: Vec<String> = self
            .working
            .all()
            .iter()
            .filter_map(|e| e.content.context_key().map(|k| k.to_string()))
            .collect();

        let checkpoint = crate::rollback::MemoryCheckpoint {
            id: uuid::Uuid::new_v4(),
            agent_id: agent_id.clone(),
            created_at: chrono::Utc::now(),
            label: label.to_string(),
            entry_versions,
            existing_entry_ids,
            working_keys,
        };

        // Store in rollback manager
        if self.rollback_mgr.checkpoint_count() >= 50 {
            // Manager handles trimming internally via create_checkpoint
        }
        // Since we can't call create_checkpoint on the manager (it needs &MemorySpace),
        // we store the checkpoint directly
        self.rollback_mgr.store_checkpoint(checkpoint.clone());

        checkpoint
    }

    /// Rolls back to a checkpoint.
    ///
    /// Temporarily takes the `RollbackManager` out of self to avoid
    /// double-mutable-borrow, then puts it back.
    pub fn rollback(
        &mut self,
        checkpoint_id: uuid::Uuid,
        reason: &str,
    ) -> Result<crate::rollback::RollbackRecord, MemoryError> {
        // Take rollback_mgr out to avoid &mut self + &mut self.rollback_mgr conflict
        let mut mgr = std::mem::replace(&mut self.rollback_mgr, RollbackManager::new(50));
        let result = mgr.rollback(checkpoint_id, reason, self);
        self.rollback_mgr = mgr;
        result
    }

    // ── Internal helpers ────────────────────────────────────────────────

    /// Applies a contradiction resolution to the existing entry in semantic memory.
    fn apply_contradiction_resolution(
        &mut self,
        contradiction: &Contradiction,
    ) -> Result<(), MemoryError> {
        match &contradiction.recommended_resolution {
            ContradictionResolution::Supersede { .. } => {
                if let Some(entry) = self.semantic.get_mut(contradiction.existing_entry_id) {
                    entry.validation_state = ValidationState::Deprecated;
                    entry.updated_at = Utc::now();
                }
            }
            ContradictionResolution::TemporalSuccession { .. } => {
                if let Some(entry) = self.semantic.get_mut(contradiction.existing_entry_id) {
                    entry.valid_to = Some(Utc::now());
                    entry.updated_at = Utc::now();
                }
            }
            ContradictionResolution::FlagForReview { .. } => {
                if let Some(entry) = self.semantic.get_mut(contradiction.existing_entry_id) {
                    entry.validation_state = ValidationState::Contested;
                    if !entry.tags.contains(&"has_contradiction".to_string()) {
                        entry.tags.push("has_contradiction".into());
                    }
                }
            }
            ContradictionResolution::CoexistWithContext { .. } => {
                if let Some(entry) = self.semantic.get_mut(contradiction.existing_entry_id) {
                    if !entry.tags.contains(&"has_contradiction".to_string()) {
                        entry.tags.push("has_contradiction".into());
                    }
                }
            }
        }
        Ok(())
    }

    fn matches_query(
        &self,
        entry: &MemoryEntry,
        query: &MemoryQuery,
        now: chrono::DateTime<Utc>,
    ) -> bool {
        // Expiry check
        if !query.include_expired && entry.is_expired() {
            return false;
        }

        // Trust filter
        if let Some(min_trust) = query.min_trust {
            if entry.trust_score < min_trust {
                return false;
            }
        }

        // Confidence filter
        if let Some(min_conf) = query.min_confidence {
            if entry.confidence < min_conf {
                return false;
            }
        }

        // Epistemic class filter
        if let Some(ref filters) = query.epistemic_filter {
            if !filters.contains(&entry.epistemic_class.to_filter()) {
                return false;
            }
        }

        // Validation state filter
        if let Some(ref states) = query.validation_states {
            if !states.contains(&entry.validation_state) {
                return false;
            }
        }

        // Tag filter (any match)
        if let Some(ref tags) = query.tags {
            if !tags.iter().any(|t| entry.tags.contains(t)) {
                return false;
            }
        }

        // Scope filter
        if let Some(ref scope) = query.scope {
            if entry.scope != *scope {
                return false;
            }
        }

        // Since filter
        if let Some(since) = query.since {
            if entry.created_at < since {
                return false;
            }
        }

        let _ = now; // used for future temporal validity checks
        true
    }
}

/// Helper to create a working memory entry for the write pipeline.
pub fn make_working_entry(agent_id: &str, key: &str, value: serde_json::Value) -> MemoryEntry {
    let now = Utc::now();
    MemoryEntry {
        id: Uuid::new_v4(),
        schema_version: 1,
        agent_id: agent_id.into(),
        memory_type: MemoryType::Working,
        epistemic_class: EpistemicClass::Observation,
        validation_state: ValidationState::Unverified,
        content: MemoryContent::Context {
            key: key.into(),
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
    }
}

/// Helper to create an episodic memory entry.
pub fn make_episodic_entry(
    agent_id: &str,
    event_type: crate::types::EpisodeType,
    summary: &str,
    details: serde_json::Value,
    outcome: Option<crate::types::Outcome>,
    task_id: Option<&str>,
) -> MemoryEntry {
    let now = Utc::now();
    MemoryEntry {
        id: Uuid::new_v4(),
        schema_version: 1,
        agent_id: agent_id.into(),
        memory_type: MemoryType::Episodic,
        epistemic_class: EpistemicClass::Observation,
        validation_state: ValidationState::Unverified,
        content: MemoryContent::Episode {
            event_type,
            summary: summary.into(),
            details,
            outcome,
            duration_ms: None,
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
        source_task_id: task_id.map(|s| s.into()),
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

/// Helper to create a semantic memory entry for the write pipeline.
pub fn make_semantic_entry(agent_id: &str, content: MemoryContent) -> MemoryEntry {
    let now = Utc::now();
    MemoryEntry {
        id: Uuid::new_v4(),
        schema_version: 1,
        agent_id: agent_id.into(),
        memory_type: MemoryType::Semantic,
        epistemic_class: EpistemicClass::Observation,
        validation_state: ValidationState::Unverified,
        content,
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
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn make_space() -> MemorySpace {
        MemorySpace::new("agent-1".into(), MemoryConfig::default())
    }

    #[test]
    fn write_working_entry() {
        let mut space = make_space();
        let entry = make_working_entry("agent-1", "goal", serde_json::json!("test"));

        let _id = space.write(entry).unwrap();
        assert!(space.working.get("goal").is_some());
        assert!(space.is_dirty());
    }

    #[test]
    fn write_episodic_entry() {
        let mut space = make_space();
        let entry = make_episodic_entry(
            "agent-1",
            EpisodeType::ActionExecuted,
            "did something",
            serde_json::Value::Null,
            None,
            None,
        );

        let _id = space.write(entry).unwrap();
        assert_eq!(space.episodic.len(), 1);
        assert!(space.is_dirty());
    }

    #[test]
    fn write_rejects_type_mismatch() {
        let mut space = make_space();
        let now = Utc::now();
        // Episodic content but Working type
        let entry = MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "agent-1".into(),
            memory_type: MemoryType::Working,
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content: MemoryContent::Episode {
                event_type: EpisodeType::Conversation,
                summary: "x".into(),
                details: serde_json::Value::Null,
                outcome: None,
                duration_ms: None,
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
            space.write(entry),
            Err(MemoryError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn query_across_types() {
        let mut space = make_space();

        space
            .write(make_working_entry("agent-1", "k1", serde_json::json!(1)))
            .unwrap();
        space
            .write(make_episodic_entry(
                "agent-1",
                EpisodeType::ActionExecuted,
                "x",
                serde_json::Value::Null,
                None,
                None,
            ))
            .unwrap();

        let all = space.query(&MemoryQuery::default()).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn query_with_trust_filter() {
        let mut space = make_space();

        space
            .write(make_working_entry("agent-1", "k", serde_json::json!(1)))
            .unwrap();

        let high = space
            .query(&MemoryQuery {
                min_trust: Some(0.99),
                ..Default::default()
            })
            .unwrap();
        assert!(
            high.is_empty(),
            "Observation trust is 0.95, should not match 0.99"
        );

        let low = space
            .query(&MemoryQuery {
                min_trust: Some(0.5),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(low.len(), 1);
    }

    #[test]
    fn query_with_limit() {
        let mut space = make_space();
        for i in 0..5 {
            space
                .write(make_working_entry(
                    "agent-1",
                    &format!("k{i}"),
                    serde_json::json!(i),
                ))
                .unwrap();
        }

        let limited = space
            .query(&MemoryQuery {
                limit: Some(3),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(limited.len(), 3);
    }

    #[test]
    fn usage_stats() {
        let mut space = make_space();
        space
            .write(make_working_entry("agent-1", "k", serde_json::json!(1)))
            .unwrap();
        space
            .write(make_episodic_entry(
                "agent-1",
                EpisodeType::Conversation,
                "x",
                serde_json::Value::Null,
                None,
                None,
            ))
            .unwrap();

        let usage = space.usage();
        assert_eq!(usage.working_count, 1);
        assert_eq!(usage.episodic_count, 1);
    }

    #[test]
    fn clear_working_and_dirty() {
        let mut space = make_space();
        space
            .write(make_working_entry("agent-1", "k", serde_json::json!(1)))
            .unwrap();

        space.mark_clean();
        assert!(!space.is_dirty());

        space.clear_working();
        assert!(space.is_dirty());
        assert!(space.working.is_empty());
    }

    #[test]
    fn write_semantic_entry() {
        let mut space = make_space();
        let now = Utc::now();
        let entry = MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "a".into(),
            memory_type: MemoryType::Semantic,
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content: MemoryContent::Triple {
                subject: "s".into(),
                predicate: "p".into(),
                object: "o".into(),
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

        let _id = space.write(entry).unwrap();
        assert_eq!(space.semantic.len(), 1);
        assert!(space.is_dirty());
    }

    #[test]
    fn write_semantic_with_contradiction_tags() {
        let mut space = make_space();
        let now = Utc::now();

        // Write first triple
        let entry1 = MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "a".into(),
            memory_type: MemoryType::Semantic,
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content: MemoryContent::Triple {
                subject: "Earth".into(),
                predicate: "shape".into(),
                object: "sphere".into(),
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
        space.write(entry1).unwrap();

        // Write contradicting triple
        let entry2 = MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "a".into(),
            memory_type: MemoryType::Semantic,
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content: MemoryContent::Triple {
                subject: "Earth".into(),
                predicate: "shape".into(),
                object: "flat".into(),
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
        space.write(entry2).unwrap();

        // Both should exist, both should have contradiction tag
        assert_eq!(space.semantic.len(), 2);
        for entry in space.semantic.all() {
            assert!(
                entry.tags.contains(&"has_contradiction".to_string()),
                "entry should be tagged with has_contradiction"
            );
        }
    }

    #[test]
    fn query_includes_semantic() {
        let mut space = make_space();
        let now = Utc::now();

        // Add a semantic entry
        let entry = MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "agent-1".into(),
            memory_type: MemoryType::Semantic,
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content: MemoryContent::Assertion {
                statement: "test assertion".into(),
                citations: vec![],
            },
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
        };
        space.write(entry).unwrap();

        // Default query should include semantic
        let results = space.query(&MemoryQuery::default()).unwrap();
        assert_eq!(results.len(), 1);
    }
}
