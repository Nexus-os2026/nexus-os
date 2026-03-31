//! Hardened garbage collection that respects epistemic class, compliance
//! invariants, and type-specific retention rules.
//!
//! ## Hard invariants
//!
//! - **Episodic entries are NEVER hard-deleted** (Invariant #2).
//! - **Semantic/Procedural entries are soft-deleted only** (Invariant #10).
//! - Corroborated, Observation, and UserAssertion entries are protected.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::space::MemorySpace;
use crate::types::*;

/// Configuration for garbage collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcConfig {
    /// How often to run GC (seconds).
    pub interval_seconds: u64,
    /// Retention score threshold — entries below this are GC candidates.
    pub semantic_retention_threshold: f32,
    /// Minimum age before an entry can be GC'd (seconds).
    pub min_age_seconds: u64,
    /// Maximum entries to process per GC run.
    pub max_entries_per_run: usize,
    /// Whether to compress old episodic entries into summaries.
    pub enable_episodic_compression: bool,
    /// Episodes older than this are compression candidates (seconds).
    pub episodic_compression_age_seconds: u64,
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 1800,
            semantic_retention_threshold: 0.3,
            min_age_seconds: 86400,
            max_entries_per_run: 500,
            enable_episodic_compression: false,
            episodic_compression_age_seconds: 2_592_000,
        }
    }
}

/// Report from a GC run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcReport {
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub entries_scanned: usize,
    pub working_cleared: usize,
    pub semantic_soft_deleted: usize,
    pub procedural_demoted: usize,
    pub episodic_compressed: usize,
    pub errors: Vec<String>,
}

/// The garbage collector for memory spaces.
pub struct MemoryGarbageCollector {
    config: GcConfig,
}

impl MemoryGarbageCollector {
    /// Creates a new GC with the given configuration.
    pub fn new(config: GcConfig) -> Self {
        Self { config }
    }

    /// Calculates the retention score for a memory entry.
    ///
    /// Higher score = more worth keeping.  Range: approximately 0.0–1.0.
    pub fn calculate_retention_score(&self, entry: &MemoryEntry) -> f32 {
        let now = Utc::now();
        let age_hours = (now - entry.created_at).num_hours().max(0) as f32;

        let importance_score = entry.importance;
        let recency_score = 1.0 - (age_hours / 720.0).min(1.0);
        let access_score = (entry.access_count as f32 / 100.0).min(1.0);
        let trust_score = entry.trust_score;
        let epistemic_score = epistemic_class_weight(&entry.epistemic_class);

        importance_score * 0.35
            + recency_score * 0.25
            + access_score * 0.20
            + trust_score * 0.10
            + epistemic_score * 0.10
    }

    /// Runs garbage collection on a memory space.
    pub fn run(&self, space: &mut MemorySpace) -> GcReport {
        let started_at = Utc::now();
        let mut entries_scanned = 0usize;
        let mut working_cleared = 0usize;
        let mut semantic_soft_deleted = 0usize;
        let mut procedural_demoted = 0usize;
        let mut errors = Vec::new();

        let now = Utc::now();

        // ── Working memory: clear expired entries ────────────────────
        let expired_keys: Vec<String> = space
            .working
            .all()
            .iter()
            .filter(|e| {
                entries_scanned += 1;
                e.is_expired()
            })
            .filter_map(|e| e.content.context_key().map(|k| k.to_string()))
            .collect();

        for key in &expired_keys {
            space.working.remove(key);
            working_cleared += 1;
        }

        // ── Episodic memory: NEVER delete (Invariant #2) ────────────
        // Just count for the scan total
        entries_scanned += space.episodic.len();
        // Compression disabled until Phase 5

        // ── Semantic memory: soft-delete low-retention entries ───────
        let semantic_candidates: Vec<(MemoryId, f32)> = space
            .semantic
            .all()
            .iter()
            .filter(|e| {
                entries_scanned += 1;
                e.validation_state != ValidationState::Revoked
            })
            .filter_map(|e| {
                let score = self.calculate_retention_score(e);
                if score < self.config.semantic_retention_threshold {
                    Some((e.id, score))
                } else {
                    None
                }
            })
            .take(self.config.max_entries_per_run)
            .collect();

        for (id, _score) in semantic_candidates {
            // Safety checks — get entry details
            let entry = match space.semantic.get(id) {
                Some(e) => e,
                None => continue,
            };

            // Do NOT delete Corroborated entries
            if entry.validation_state == ValidationState::Corroborated {
                continue;
            }

            // Do NOT delete Observation or UserAssertion (high trust)
            if matches!(
                entry.epistemic_class,
                EpistemicClass::Observation | EpistemicClass::UserAssertion
            ) {
                continue;
            }

            // Do NOT delete frequently accessed entries
            if entry.access_count > 10 {
                continue;
            }

            // Do NOT delete entries younger than min_age
            let age_secs = (now - entry.created_at).num_seconds();
            if age_secs < self.config.min_age_seconds as i64 {
                continue;
            }

            // Safe to soft-delete
            match space.semantic.soft_delete(id, "gc_low_retention_score") {
                Ok(_) => semantic_soft_deleted += 1,
                Err(e) => errors.push(format!("semantic gc: {e}")),
            }
        }

        // ── Procedural memory: demote stale low-success procedures ───
        let proc_candidates: Vec<MemoryId> = space
            .procedural
            .all_procedures()
            .iter()
            .filter(|e| {
                entries_scanned += 1;
                e.validation_state != ValidationState::Revoked
                    && e.validation_state != ValidationState::Deprecated
            })
            .filter(|e| {
                // Don't touch high-success procedures
                if e.trust_score >= 0.7 {
                    return false;
                }
                // Stale: not accessed in 30+ days
                let days_since = (now - e.last_accessed).num_days();
                days_since > 30 && e.trust_score < 0.5
            })
            .map(|e| e.id)
            .collect();

        for id in proc_candidates {
            match space.procedural.demote(id, "gc_stale_low_success") {
                Ok(_) => procedural_demoted += 1,
                Err(e) => errors.push(format!("procedural gc: {e}")),
            }
        }

        GcReport {
            started_at,
            completed_at: Utc::now(),
            entries_scanned,
            working_cleared,
            semantic_soft_deleted,
            procedural_demoted,
            episodic_compressed: 0,
            errors,
        }
    }

    /// Returns `true` if enough time has passed since the last GC run.
    pub fn should_run(&self, last_run: Option<DateTime<Utc>>) -> bool {
        match last_run {
            None => true,
            Some(last) => {
                let elapsed = (Utc::now() - last).num_seconds();
                elapsed >= self.config.interval_seconds as i64
            }
        }
    }

    /// Returns the GC configuration.
    pub fn config(&self) -> &GcConfig {
        &self.config
    }
}

/// Returns the epistemic class retention weight (higher = more protected).
fn epistemic_class_weight(class: &EpistemicClass) -> f32 {
    match class {
        EpistemicClass::SystemGenerated => 1.0,
        EpistemicClass::Observation => 0.95,
        EpistemicClass::UserAssertion => 0.9,
        EpistemicClass::LearnedBehavior { .. } => 0.8,
        EpistemicClass::Inference { .. } => 0.6,
        EpistemicClass::SharedKnowledge { .. } => 0.5,
        EpistemicClass::Summary { .. } => 0.4,
        EpistemicClass::Imported { .. } => 0.35,
        EpistemicClass::CachedRetrieval { .. } => 0.3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Duration;
    use uuid::Uuid;

    fn make_gc() -> MemoryGarbageCollector {
        MemoryGarbageCollector::new(GcConfig::default())
    }

    fn make_space() -> MemorySpace {
        MemorySpace::new("agent-1".into(), MemoryConfig::default())
    }

    fn make_entry_with_scores(
        importance: f32,
        trust: f32,
        access_count: u64,
        age_hours: i64,
        class: EpistemicClass,
    ) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "a".into(),
            memory_type: MemoryType::Semantic,
            epistemic_class: class,
            validation_state: ValidationState::Unverified,
            content: MemoryContent::Assertion {
                statement: "test".into(),
                citations: vec![],
            },
            embedding: None,
            created_at: now - Duration::hours(age_hours),
            updated_at: now,
            valid_from: now - Duration::hours(age_hours),
            valid_to: None,
            trust_score: trust,
            importance,
            confidence: 0.5,
            supersedes: None,
            derived_from: vec![],
            source_task_id: None,
            source_conversation_id: None,
            scope: MemoryScope::Agent,
            sensitivity: SensitivityClass::Internal,
            access_count,
            last_accessed: now,
            version: 1,
            ttl: None,
            tags: vec![],
        }
    }

    // ── Retention score ──────────────────────────────────────────────

    #[test]
    fn retention_high_importance_recent_frequent() {
        let gc = make_gc();
        let entry = make_entry_with_scores(0.9, 0.9, 50, 1, EpistemicClass::Observation);
        let score = gc.calculate_retention_score(&entry);
        assert!(
            score > 0.7,
            "high importance + recent + frequent = high score, got {score}"
        );
    }

    #[test]
    fn retention_old_unused_low_importance() {
        let gc = make_gc();
        let entry = make_entry_with_scores(
            0.1,
            0.2,
            0,
            800,
            EpistemicClass::CachedRetrieval {
                source_url: None,
                retrieved_at: Utc::now(),
            },
        );
        let score = gc.calculate_retention_score(&entry);
        assert!(
            score < 0.3,
            "old + unused + low importance = low score, got {score}"
        );
    }

    #[test]
    fn epistemic_weight_ordering() {
        let obs = epistemic_class_weight(&EpistemicClass::Observation);
        let inf = epistemic_class_weight(&EpistemicClass::Inference {
            derived_from: vec![],
        });
        let cached = epistemic_class_weight(&EpistemicClass::CachedRetrieval {
            source_url: None,
            retrieved_at: Utc::now(),
        });
        assert!(obs > inf);
        assert!(inf > cached);
    }

    // ── GC working memory ────────────────────────────────────────────

    #[test]
    fn gc_clears_expired_working_memory() {
        let gc = make_gc();
        let mut space = make_space();

        // Write an entry then manually expire it
        space
            .write(crate::space::make_working_entry(
                "agent-1",
                "expired_key",
                serde_json::json!("val"),
            ))
            .unwrap();

        // Set TTL to -1 to force expiry
        if let Some(entry) = space.working.get_mut("expired_key") {
            entry.ttl = Some(-1);
        }

        let report = gc.run(&mut space);
        assert_eq!(report.working_cleared, 1);
    }

    // ── GC semantic memory ───────────────────────────────────────────

    #[test]
    fn gc_soft_deletes_low_retention_semantic() {
        let gc = MemoryGarbageCollector::new(GcConfig {
            semantic_retention_threshold: 0.5,
            min_age_seconds: 0, // no min age for test
            ..Default::default()
        });
        let mut space = make_space();

        // Low-retention entry: old, low importance, CachedRetrieval
        let mut entry = make_entry_with_scores(
            0.05,
            0.1,
            0,
            800,
            EpistemicClass::CachedRetrieval {
                source_url: None,
                retrieved_at: Utc::now(),
            },
        );
        entry.memory_type = MemoryType::Semantic;
        space.write(entry).unwrap();

        let report = gc.run(&mut space);
        assert_eq!(report.semantic_soft_deleted, 1);
    }

    #[test]
    fn gc_does_not_delete_corroborated() {
        let gc = MemoryGarbageCollector::new(GcConfig {
            semantic_retention_threshold: 0.99, // very high threshold
            min_age_seconds: 0,
            ..Default::default()
        });
        let mut space = make_space();

        let mut entry = make_entry_with_scores(
            0.1,
            0.1,
            0,
            800,
            EpistemicClass::Inference {
                derived_from: vec![],
            },
        );
        entry.memory_type = MemoryType::Semantic;
        entry.validation_state = ValidationState::Corroborated;
        space.write(entry).unwrap();

        let report = gc.run(&mut space);
        assert_eq!(
            report.semantic_soft_deleted, 0,
            "Corroborated should be protected"
        );
    }

    #[test]
    fn gc_does_not_delete_observation() {
        let gc = MemoryGarbageCollector::new(GcConfig {
            semantic_retention_threshold: 0.99,
            min_age_seconds: 0,
            ..Default::default()
        });
        let mut space = make_space();

        let mut entry = make_entry_with_scores(0.01, 0.01, 0, 800, EpistemicClass::Observation);
        entry.memory_type = MemoryType::Semantic;
        space.write(entry).unwrap();

        let report = gc.run(&mut space);
        assert_eq!(
            report.semantic_soft_deleted, 0,
            "Observation should be protected"
        );
    }

    #[test]
    fn gc_does_not_delete_user_assertion() {
        let gc = MemoryGarbageCollector::new(GcConfig {
            semantic_retention_threshold: 0.99,
            min_age_seconds: 0,
            ..Default::default()
        });
        let mut space = make_space();

        let mut entry = make_entry_with_scores(0.01, 0.01, 0, 800, EpistemicClass::UserAssertion);
        entry.memory_type = MemoryType::Semantic;
        space.write(entry).unwrap();

        let report = gc.run(&mut space);
        assert_eq!(
            report.semantic_soft_deleted, 0,
            "UserAssertion should be protected"
        );
    }

    #[test]
    fn gc_does_not_delete_frequently_accessed() {
        let gc = MemoryGarbageCollector::new(GcConfig {
            semantic_retention_threshold: 0.99,
            min_age_seconds: 0,
            ..Default::default()
        });
        let mut space = make_space();

        let mut entry = make_entry_with_scores(
            0.01,
            0.01,
            50,
            800,
            EpistemicClass::CachedRetrieval {
                source_url: None,
                retrieved_at: Utc::now(),
            },
        );
        entry.memory_type = MemoryType::Semantic;
        space.write(entry).unwrap();

        let report = gc.run(&mut space);
        assert_eq!(
            report.semantic_soft_deleted, 0,
            "Frequently accessed (>10) should be protected"
        );
    }

    #[test]
    fn gc_does_not_delete_young_entries() {
        let gc = MemoryGarbageCollector::new(GcConfig {
            semantic_retention_threshold: 0.99,
            min_age_seconds: 86400, // 24 hours
            ..Default::default()
        });
        let mut space = make_space();

        // Entry only 1 hour old
        let mut entry = make_entry_with_scores(
            0.01,
            0.01,
            0,
            1,
            EpistemicClass::CachedRetrieval {
                source_url: None,
                retrieved_at: Utc::now(),
            },
        );
        entry.memory_type = MemoryType::Semantic;
        space.write(entry).unwrap();

        let report = gc.run(&mut space);
        assert_eq!(
            report.semantic_soft_deleted, 0,
            "Young entries should be protected by min_age"
        );
    }

    // ── GC procedural memory ─────────────────────────────────────────

    #[test]
    fn gc_demotes_stale_low_success_procedural() {
        let gc = make_gc();
        let mut space = make_space();

        let now = Utc::now();
        let entry = MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "agent-1".into(),
            memory_type: MemoryType::Procedural,
            epistemic_class: EpistemicClass::LearnedBehavior {
                evidence_task_ids: vec![],
                success_rate: 0.3,
            },
            validation_state: ValidationState::Unverified,
            content: MemoryContent::Procedure {
                name: "stale".into(),
                description: "old proc".into(),
                trigger_condition: "trigger".into(),
                steps: vec![],
            },
            embedding: None,
            created_at: now - Duration::days(60),
            updated_at: now - Duration::days(60),
            valid_from: now - Duration::days(60),
            valid_to: None,
            trust_score: 0.3,
            importance: 0.3,
            confidence: 0.3,
            supersedes: None,
            derived_from: vec![],
            source_task_id: None,
            source_conversation_id: None,
            scope: MemoryScope::Agent,
            sensitivity: SensitivityClass::Internal,
            access_count: 0,
            last_accessed: now - Duration::days(45),
            version: 1,
            ttl: None,
            tags: vec![],
        };
        space.write(entry).unwrap();

        let report = gc.run(&mut space);
        assert_eq!(report.procedural_demoted, 1);
    }

    #[test]
    fn gc_does_not_demote_high_success_procedural() {
        let gc = make_gc();
        let mut space = make_space();

        let now = Utc::now();
        let entry = MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "agent-1".into(),
            memory_type: MemoryType::Procedural,
            epistemic_class: EpistemicClass::LearnedBehavior {
                evidence_task_ids: vec![],
                success_rate: 0.9,
            },
            validation_state: ValidationState::Corroborated,
            content: MemoryContent::Procedure {
                name: "good".into(),
                description: "good proc".into(),
                trigger_condition: "trigger".into(),
                steps: vec![],
            },
            embedding: None,
            created_at: now - Duration::days(60),
            updated_at: now - Duration::days(60),
            valid_from: now - Duration::days(60),
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
            access_count: 5,
            last_accessed: now - Duration::days(45),
            version: 1,
            ttl: None,
            tags: vec![],
        };
        space.write(entry).unwrap();

        let report = gc.run(&mut space);
        assert_eq!(
            report.procedural_demoted, 0,
            "High-success should be protected"
        );
    }

    // ── GC episodic memory ───────────────────────────────────────────

    #[test]
    fn gc_never_deletes_episodic() {
        let gc = make_gc();
        let mut space = make_space();

        // Add an old episodic entry
        space
            .write(crate::space::make_episodic_entry(
                "agent-1",
                EpisodeType::ActionExecuted,
                "old action",
                serde_json::Value::Null,
                None,
                None,
            ))
            .unwrap();

        let count_before = space.episodic.len();
        let report = gc.run(&mut space);
        assert_eq!(
            space.episodic.len(),
            count_before,
            "Episodic entries must never be deleted"
        );
        assert_eq!(report.episodic_compressed, 0);
    }

    // ── GC timing ────────────────────────────────────────────────────

    #[test]
    fn should_run_when_never_run() {
        let gc = make_gc();
        assert!(gc.should_run(None));
    }

    #[test]
    fn should_not_run_when_recent() {
        let gc = make_gc();
        assert!(!gc.should_run(Some(Utc::now())));
    }

    #[test]
    fn gc_report_has_counts() {
        let gc = make_gc();
        let mut space = make_space();
        let report = gc.run(&mut space);
        assert!(report.errors.is_empty());
        assert!(report.completed_at >= report.started_at);
    }
}
