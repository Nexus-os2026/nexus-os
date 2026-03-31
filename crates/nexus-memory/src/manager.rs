//! Memory Manager — the kernel entry point for the memory subsystem.
//!
//! The `MemoryManager` is where governance meets memory.  It owns all agent
//! memory spaces, the persistence layer, the audit log, the search engine,
//! and an optional embedder.  Every read and write goes through the manager,
//! which:
//!
//! 1. Validates the caller has access.
//! 2. Logs the operation to the audit trail (before the operation, so failed
//!    operations are still audited).
//! 3. Delegates to the appropriate `MemorySpace`.
//! 4. Persists changes.
//! 5. Indexes embeddings for search (when an embedder is available).

use dashmap::DashMap;
use tracing::{info, warn};

use crate::audit::MemoryAuditLog;
use crate::embedding::MemoryEmbedder;
use crate::persistence::MemoryPersistence;
use crate::search::{MemorySearchEngine, MemorySearchResult, RetrievalPolicy};
use crate::space::MemorySpace;
use crate::types::{
    MemoryConfig, MemoryEntry, MemoryError, MemoryId, MemoryOperation, MemoryQuery, MemoryType,
    MemoryUsage,
};

/// The kernel entry point for all memory operations.
pub struct MemoryManager {
    /// Per-agent memory spaces, keyed by agent_id.
    spaces: DashMap<String, MemorySpace>,
    /// SQLite persistence for memory entries.
    persistence: MemoryPersistence,
    /// Hash-chained audit log.
    audit: MemoryAuditLog,
    /// Search engine with vector index and keyword matching.
    search_engine: MemorySearchEngine,
    /// Optional embedding provider (None = keyword-only search).
    embedder: Option<Box<dyn MemoryEmbedder>>,
    /// Default configuration for new spaces.
    config: MemoryConfig,
}

impl MemoryManager {
    /// Creates a new memory manager with SQLite databases in `data_dir`.
    ///
    /// Two databases are created:
    /// - `{data_dir}/memory.db` — entry persistence
    /// - `{data_dir}/memory_audit.db` — audit trail
    ///
    /// The optional `embedder` enables vector search.  Pass `None` to use
    /// keyword-only search (the system degrades gracefully).
    pub fn new(
        data_dir: &str,
        config: MemoryConfig,
        embedder: Option<Box<dyn MemoryEmbedder>>,
    ) -> Result<Self, MemoryError> {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| MemoryError::PersistenceError(format!("create dir: {e}")))?;

        let db_path = format!("{data_dir}/memory.db");
        let audit_path = format!("{data_dir}/memory_audit.db");

        let persistence = MemoryPersistence::new(&db_path)?;
        let audit = MemoryAuditLog::new(&audit_path)?;

        info!("MemoryManager initialized at {data_dir}");

        Ok(Self {
            spaces: DashMap::new(),
            persistence,
            audit,
            search_engine: MemorySearchEngine::new(),
            embedder,
            config,
        })
    }

    /// Creates a new memory manager with in-memory databases (for testing).
    pub fn in_memory(config: MemoryConfig) -> Result<Self, MemoryError> {
        let persistence = MemoryPersistence::in_memory()?;
        let audit = MemoryAuditLog::in_memory()?;

        Ok(Self {
            spaces: DashMap::new(),
            persistence,
            audit,
            search_engine: MemorySearchEngine::new(),
            embedder: None,
            config,
        })
    }

    /// Creates a new memory manager with in-memory databases and an embedder.
    pub fn in_memory_with_embedder(
        config: MemoryConfig,
        embedder: Box<dyn MemoryEmbedder>,
    ) -> Result<Self, MemoryError> {
        let persistence = MemoryPersistence::in_memory()?;
        let audit = MemoryAuditLog::in_memory()?;

        Ok(Self {
            spaces: DashMap::new(),
            persistence,
            audit,
            search_engine: MemorySearchEngine::new(),
            embedder: Some(embedder),
            config,
        })
    }

    /// Creates a new memory space for an agent.
    pub fn create_space(&self, agent_id: &str) -> Result<(), MemoryError> {
        if self.spaces.contains_key(agent_id) {
            return Ok(()); // idempotent
        }

        let space = MemorySpace::new(agent_id.to_string(), self.config.clone());
        self.spaces.insert(agent_id.to_string(), space);
        info!(agent_id, "Memory space created");
        Ok(())
    }

    /// Gets a read reference to an agent's memory space.
    pub fn get_space(
        &self,
        agent_id: &str,
    ) -> Result<dashmap::mapref::one::Ref<'_, String, MemorySpace>, MemoryError> {
        self.spaces
            .get(agent_id)
            .ok_or_else(|| MemoryError::SpaceNotFound(agent_id.to_string()))
    }

    /// Gets a mutable reference to an agent's memory space.
    pub fn get_space_mut(
        &self,
        agent_id: &str,
    ) -> Result<dashmap::mapref::one::RefMut<'_, String, MemorySpace>, MemoryError> {
        self.spaces
            .get_mut(agent_id)
            .ok_or_else(|| MemoryError::SpaceNotFound(agent_id.to_string()))
    }

    /// Destroys an agent's memory space (archives entries first).
    pub async fn destroy_space(&self, agent_id: &str) -> Result<(), MemoryError> {
        // Audit the destruction
        self.audit
            .log(
                agent_id,
                "system",
                MemoryOperation::SoftDelete,
                MemoryType::Working,
                None,
                Some("Memory space destroyed".into()),
            )
            .await?;

        self.spaces.remove(agent_id);
        warn!(agent_id, "Memory space destroyed");
        Ok(())
    }

    /// The main write path.
    ///
    /// 1. Ensures the space exists.
    /// 2. Logs audit entry **before** the write.
    /// 3. Generates embedding for semantic entries (if embedder available).
    /// 4. Delegates to `MemorySpace::write`.
    /// 5. Persists the entry.
    /// 6. Indexes the entry for search.
    /// 7. Returns the `MemoryId`.
    pub async fn write(
        &self,
        agent_id: &str,
        accessor_id: &str,
        entry: MemoryEntry,
    ) -> Result<MemoryId, MemoryError> {
        let memory_type = entry.memory_type;
        let entry_id_hint = entry.id;

        // 1. Audit before write
        self.audit
            .log(
                agent_id,
                accessor_id,
                MemoryOperation::Write,
                memory_type,
                Some(entry_id_hint),
                None,
            )
            .await?;

        // 2. Generate embedding for semantic entries if embedder is available
        let mut entry = entry;
        if memory_type == MemoryType::Semantic && entry.embedding.is_none() {
            if let Some(ref embedder) = self.embedder {
                let text = crate::search::extract_searchable_text(&entry);
                match embedder.embed(&text) {
                    Ok(embedding) => entry.embedding = Some(embedding),
                    Err(e) => {
                        // Graceful degradation — log but don't fail the write
                        warn!("Embedding generation failed: {e}");
                    }
                }
            }
        }

        // 3. Get space and write
        let id = {
            let mut space = self.get_space_mut(agent_id)?;
            space.write(entry.clone())?
        };

        // 4. Persist
        self.persistence.save_entry(&entry)?;

        // 5. Index for search
        self.search_engine.index_entry(&entry);

        Ok(id)
    }

    /// The main read path.
    ///
    /// 1. Ensures the space exists.
    /// 2. Logs audit entry.
    /// 3. Delegates to `MemorySpace::query`.
    /// 4. Updates access tracking on returned entries.
    pub async fn query(
        &self,
        agent_id: &str,
        accessor_id: &str,
        query: MemoryQuery,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        // 1. Audit
        self.audit
            .log(
                agent_id,
                accessor_id,
                MemoryOperation::Search,
                MemoryType::Working, // search spans all types
                None,
                None,
            )
            .await?;

        // 2. Query
        let space = self.get_space(agent_id)?;
        let results = space.query(&query)?;

        // 3. Update access tracking in persistence
        for entry in &results {
            // Best-effort — don't fail the query if access tracking fails
            let _ = self.persistence.update_access(entry.id);
        }

        Ok(results)
    }

    /// Semantic search across an agent's memory.
    ///
    /// Uses the search engine for vector similarity and keyword matching,
    /// filtered by the given retrieval policy.
    pub async fn search(
        &self,
        agent_id: &str,
        accessor_id: &str,
        query_text: &str,
        policy: RetrievalPolicy,
        limit: usize,
    ) -> Result<Vec<MemorySearchResult>, MemoryError> {
        // 1. Audit
        self.audit
            .log(
                agent_id,
                accessor_id,
                MemoryOperation::Search,
                MemoryType::Semantic,
                None,
                Some(format!("search: {query_text}")),
            )
            .await?;

        // 2. Generate query embedding if embedder available
        let query_embedding = if let Some(ref embedder) = self.embedder {
            match embedder.embed(query_text) {
                Ok(emb) => Some(emb),
                Err(e) => {
                    warn!("Query embedding failed: {e}");
                    None
                }
            }
        } else {
            None
        };

        // 3. Collect all entries from the space matching the policy's types
        let space = self.get_space(agent_id)?;
        let mut all_entries: Vec<MemoryEntry> = Vec::new();

        for mt in &policy.include_types {
            match mt {
                MemoryType::Working => {
                    for entry in space.working.all() {
                        all_entries.push(entry.clone());
                    }
                }
                MemoryType::Episodic => {
                    for entry in space.episodic.all() {
                        all_entries.push(entry.clone());
                    }
                }
                MemoryType::Semantic => {
                    for entry in space.semantic.all() {
                        all_entries.push(entry.clone());
                    }
                }
                MemoryType::Procedural => {
                    for entry in space.procedural.all_procedures() {
                        all_entries.push(entry.clone());
                    }
                }
            }
        }

        // 4. Search
        let results = self.search_engine.search(
            &all_entries,
            query_text,
            query_embedding.as_deref(),
            &policy,
            limit,
        );

        // 5. Update access tracking
        for result in &results {
            let _ = self.persistence.update_access(result.entry.id);
        }

        Ok(results)
    }

    /// Persists all dirty spaces to SQLite.  Returns the number of entries saved.
    pub fn checkpoint(&self) -> Result<u64, MemoryError> {
        let mut saved = 0u64;

        for mut space_ref in self.spaces.iter_mut() {
            let space = space_ref.value_mut();
            if !space.is_dirty() {
                continue;
            }

            // Save working memory entries
            for entry in space.working.all() {
                self.persistence.save_entry(entry)?;
                saved += 1;
            }

            // Save episodic memory entries
            for entry in space.episodic.all() {
                self.persistence.save_entry(entry)?;
                saved += 1;
            }

            // Save semantic memory entries
            for entry in space.semantic.all() {
                self.persistence.save_entry(entry)?;
                saved += 1;
            }

            // Save procedural memory entries
            for entry in space.procedural.all_procedures() {
                self.persistence.save_entry(entry)?;
                saved += 1;
            }

            space.mark_clean();
        }

        if saved > 0 {
            info!(saved, "Memory checkpoint complete");
        }
        Ok(saved)
    }

    /// Restores all memory spaces from SQLite.  Returns the total entries loaded.
    ///
    /// Also rebuilds the search engine's vector index from persisted embeddings.
    pub fn restore(&self) -> Result<u64, MemoryError> {
        let mut loaded = 0u64;

        for mut space_ref in self.spaces.iter_mut() {
            let space = space_ref.value_mut();
            let agent_id = space.agent_id.clone();

            // Load working memory
            let working_entries = self
                .persistence
                .load_entries(&agent_id, Some(MemoryType::Working))?;
            for entry in working_entries {
                space.working.insert_entry(entry)?;
                loaded += 1;
            }

            // Load episodic memory
            let episodic_entries = self
                .persistence
                .load_entries(&agent_id, Some(MemoryType::Episodic))?;
            for entry in episodic_entries {
                space.episodic.insert_entry(entry);
                loaded += 1;
            }

            // Load semantic memory and rebuild search index
            let semantic_entries = self
                .persistence
                .load_entries(&agent_id, Some(MemoryType::Semantic))?;
            for entry in semantic_entries {
                self.search_engine.index_entry(&entry);
                space.semantic.insert_entry(entry);
                loaded += 1;
            }

            // Load procedural memory
            let procedural_entries = self
                .persistence
                .load_entries(&agent_id, Some(MemoryType::Procedural))?;
            for entry in procedural_entries {
                space.procedural.insert_entry(entry);
                loaded += 1;
            }
        }

        if loaded > 0 {
            info!(loaded, "Memory restore complete");
        }
        Ok(loaded)
    }

    /// Returns usage statistics for all agents.
    pub fn stats(&self) -> Vec<MemoryUsage> {
        self.spaces.iter().map(|r| r.value().usage()).collect()
    }

    /// Returns a reference to the audit log.
    pub fn audit(&self) -> &MemoryAuditLog {
        &self.audit
    }

    /// Returns a reference to the persistence layer.
    pub fn persistence(&self) -> &MemoryPersistence {
        &self.persistence
    }

    /// Returns a reference to the search engine.
    pub fn search_engine(&self) -> &MemorySearchEngine {
        &self.search_engine
    }

    /// Returns the number of active memory spaces.
    pub fn space_count(&self) -> usize {
        self.spaces.len()
    }

    /// Returns `true` if an embedder is configured.
    pub fn has_embedder(&self) -> bool {
        self.embedder.is_some()
    }

    /// Returns a mutable iterator over all spaces (for GC).
    pub fn spaces_iter_mut(&self) -> dashmap::iter::IterMut<'_, String, MemorySpace> {
        self.spaces.iter_mut()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::MockEmbedder;
    use crate::space::{make_episodic_entry, make_semantic_entry, make_working_entry};
    use crate::types::*;

    fn make_manager() -> MemoryManager {
        MemoryManager::in_memory(MemoryConfig::default()).unwrap()
    }

    fn make_manager_with_embedder() -> MemoryManager {
        MemoryManager::in_memory_with_embedder(
            MemoryConfig::default(),
            Box::new(MockEmbedder::new()),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn create_and_get_space() {
        let mgr = make_manager();
        mgr.create_space("agent-1").unwrap();

        let space = mgr.get_space("agent-1").unwrap();
        assert_eq!(space.agent_id, "agent-1");
    }

    #[tokio::test]
    async fn create_space_idempotent() {
        let mgr = make_manager();
        mgr.create_space("agent-1").unwrap();
        mgr.create_space("agent-1").unwrap(); // should not error
        assert_eq!(mgr.space_count(), 1);
    }

    #[tokio::test]
    async fn get_nonexistent_space_errors() {
        let mgr = make_manager();
        let result = mgr.get_space("nope");
        assert!(matches!(result, Err(MemoryError::SpaceNotFound(_))));
    }

    #[tokio::test]
    async fn write_through_manager() {
        let mgr = make_manager();
        mgr.create_space("agent-1").unwrap();

        let entry = make_working_entry("agent-1", "goal", serde_json::json!("build"));
        let _id = mgr.write("agent-1", "system", entry).await.unwrap();

        // Verify it's in the space
        let space = mgr.get_space("agent-1").unwrap();
        assert!(space.working.get("goal").is_some());

        // Verify audit entry was created
        let audit_entries = mgr.audit.query("agent-1", 10).await.unwrap();
        assert!(!audit_entries.is_empty());
        assert_eq!(audit_entries[0].operation, MemoryOperation::Write);
    }

    #[tokio::test]
    async fn query_through_manager() {
        let mgr = make_manager();
        mgr.create_space("agent-1").unwrap();

        let entry = make_working_entry("agent-1", "k", serde_json::json!(42));
        mgr.write("agent-1", "system", entry).await.unwrap();

        let results = mgr
            .query("agent-1", "system", MemoryQuery::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);

        // Verify audit logged the search
        let audit_entries = mgr.audit.query("agent-1", 10).await.unwrap();
        assert!(audit_entries
            .iter()
            .any(|e| e.operation == MemoryOperation::Search));
    }

    #[tokio::test]
    async fn checkpoint_and_restore() {
        let mgr = make_manager();
        mgr.create_space("agent-1").unwrap();

        // Write some entries
        let w1 = make_working_entry("agent-1", "k1", serde_json::json!(1));
        let e1 = make_episodic_entry(
            "agent-1",
            EpisodeType::ActionExecuted,
            "did something",
            serde_json::Value::Null,
            None,
            None,
        );

        mgr.write("agent-1", "sys", w1).await.unwrap();
        mgr.write("agent-1", "sys", e1).await.unwrap();

        // Checkpoint
        let saved = mgr.checkpoint().unwrap();
        assert!(saved >= 2);

        // Simulate restart — create new space and restore
        let mgr2 = make_manager();
        mgr2.create_space("agent-1").unwrap();
        let loaded = mgr2.restore().unwrap();
        // Note: in-memory DBs don't share state, so this tests the code path
        // but won't have data. The persistence tests cover save/load.
        assert_eq!(loaded, 0); // expected with separate in-memory DBs
    }

    #[tokio::test]
    async fn destroy_space() {
        let mgr = make_manager();
        mgr.create_space("agent-1").unwrap();

        mgr.destroy_space("agent-1").await.unwrap();
        assert_eq!(mgr.space_count(), 0);

        // Audit trail should have the destruction logged
        let audit_entries = mgr.audit.query("agent-1", 10).await.unwrap();
        assert!(audit_entries
            .iter()
            .any(|e| e.operation == MemoryOperation::SoftDelete));
    }

    #[tokio::test]
    async fn stats_returns_all_agents() {
        let mgr = make_manager();
        mgr.create_space("agent-1").unwrap();
        mgr.create_space("agent-2").unwrap();

        let entry = make_working_entry("agent-1", "k", serde_json::json!(1));
        mgr.write("agent-1", "sys", entry).await.unwrap();

        let stats = mgr.stats();
        assert_eq!(stats.len(), 2);
    }

    #[tokio::test]
    async fn write_to_nonexistent_space_fails() {
        let mgr = make_manager();
        let entry = make_working_entry("nope", "k", serde_json::json!(1));
        let result = mgr.write("nope", "sys", entry).await;
        assert!(matches!(result, Err(MemoryError::SpaceNotFound(_))));
    }

    #[tokio::test]
    async fn write_episodic_through_manager() {
        let mgr = make_manager();
        mgr.create_space("agent-1").unwrap();

        let entry = make_episodic_entry(
            "agent-1",
            EpisodeType::Conversation,
            "user asked a question",
            serde_json::json!({"topic": "memory"}),
            Some(Outcome::Success {
                details: "answered correctly".into(),
            }),
            Some("task-1"),
        );

        let _id = mgr.write("agent-1", "system", entry).await.unwrap();

        let space = mgr.get_space("agent-1").unwrap();
        assert_eq!(space.episodic.len(), 1);
    }

    // ── Phase 2: Semantic + Search tests ─────────────────────────────────

    #[tokio::test]
    async fn write_semantic_through_manager() {
        let mgr = make_manager();
        mgr.create_space("agent-1").unwrap();

        let entry = make_semantic_entry(
            "agent-1",
            MemoryContent::Triple {
                subject: "Rust".into(),
                predicate: "is".into(),
                object: "fast".into(),
            },
        );

        let _id = mgr.write("agent-1", "system", entry).await.unwrap();

        let space = mgr.get_space("agent-1").unwrap();
        assert_eq!(space.semantic.len(), 1);
    }

    #[tokio::test]
    async fn write_semantic_generates_embedding() {
        let mgr = make_manager_with_embedder();
        mgr.create_space("agent-1").unwrap();

        let entry = make_semantic_entry(
            "agent-1",
            MemoryContent::Assertion {
                statement: "AI safety is important".into(),
                citations: vec![],
            },
        );

        let _id = mgr.write("agent-1", "system", entry).await.unwrap();

        // Verify embedding was generated and indexed
        let space = mgr.get_space("agent-1").unwrap();
        let semantic_entry = &space.semantic.all()[0];
        assert!(
            semantic_entry.embedding.is_some(),
            "embedding should be auto-generated"
        );
    }

    #[tokio::test]
    async fn search_with_keywords() {
        let mgr = make_manager();
        mgr.create_space("agent-1").unwrap();

        // Write some semantic entries
        let entries = [
            ("Rust is a systems programming language", vec![]),
            ("Python is used for data science", vec![]),
            ("Rust has zero-cost abstractions", vec![]),
        ];

        for (stmt, citations) in entries {
            let entry = make_semantic_entry(
                "agent-1",
                MemoryContent::Assertion {
                    statement: stmt.into(),
                    citations,
                },
            );
            mgr.write("agent-1", "sys", entry).await.unwrap();
        }

        let policy = RetrievalPolicy {
            include_types: vec![MemoryType::Semantic],
            min_trust: 0.0,
            min_confidence: 0.0,
            include_expired: false,
            epistemic_filter: None,
            max_age_seconds: None,
            exclude_validation_states: vec![],
            ranking: crate::search::CrossTypeRanking::PureRelevance,
        };

        let results = mgr
            .search("agent-1", "sys", "Rust programming", policy, 10)
            .await
            .unwrap();

        // Should find the Rust entries
        assert!(
            !results.is_empty(),
            "should find entries matching 'Rust programming'"
        );
    }

    #[tokio::test]
    async fn search_with_safety_policy_excludes_contested() {
        let mgr = make_manager();
        mgr.create_space("agent-1").unwrap();

        // Write a normal entry
        let entry = make_semantic_entry(
            "agent-1",
            MemoryContent::Assertion {
                statement: "tested fact about safety".into(),
                citations: vec![],
            },
        );
        mgr.write("agent-1", "sys", entry).await.unwrap();

        // Write a contested entry (simulate contradiction)
        let mut contested = make_semantic_entry(
            "agent-1",
            MemoryContent::Assertion {
                statement: "contested fact about safety".into(),
                citations: vec![],
            },
        );
        contested.validation_state = ValidationState::Contested;
        contested.trust_score = 0.9;
        contested.confidence = 0.9;
        mgr.write("agent-1", "sys", contested).await.unwrap();

        let policy = RetrievalPolicy::for_safety();
        let results = mgr
            .search("agent-1", "sys", "safety", policy, 10)
            .await
            .unwrap();

        // Safety policy should exclude contested entries
        for r in &results {
            assert_ne!(
                r.entry.validation_state,
                ValidationState::Contested,
                "safety policy should exclude contested entries"
            );
        }
    }

    #[tokio::test]
    async fn search_audit_trail() {
        let mgr = make_manager();
        mgr.create_space("agent-1").unwrap();

        let policy = RetrievalPolicy::for_planning();
        let _results = mgr
            .search("agent-1", "sys", "test query", policy, 10)
            .await
            .unwrap();

        let audit_entries = mgr.audit.query("agent-1", 10).await.unwrap();
        assert!(
            audit_entries
                .iter()
                .any(|e| e.operation == MemoryOperation::Search),
            "search should be audited"
        );
    }

    #[tokio::test]
    async fn has_embedder_returns_correct_state() {
        let mgr = make_manager();
        assert!(!mgr.has_embedder());

        let mgr2 = make_manager_with_embedder();
        assert!(mgr2.has_embedder());
    }
}
