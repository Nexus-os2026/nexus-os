//! # Nexus Memory — Agent Memory Management Subsystem
//!
//! A kernel subsystem providing governed memory for every Nexus OS agent.
//! Each agent gets a `MemorySpace` with four memory types:
//!
//! | Type | Description | Phase |
//! |------|-------------|-------|
//! | Working | Key-value scratch space for the current task | 1 |
//! | Episodic | Append-only chronicle of events and actions | 1 |
//! | Semantic | Structured knowledge (triples, assertions, entities) | 2 |
//! | Procedural | Learned procedures with promotion gates and regression | 3 (this release) |
//!
//! ## Invariants
//!
//! 1. **Every memory operation is audited** — reads, writes, deletes, searches.
//! 2. **Episodic memory is append-only** — never deleted, never modified.
//! 3. **Every entry has an epistemic class** — trust is not optional.
//! 4. **Hash-chained audit trail** — tamper-proof operation log.
//!
//! ## Quick Start
//!
//! ```no_run
//! use nexus_memory::{MemoryManager, MemoryConfig};
//! use nexus_memory::space::{make_working_entry, make_episodic_entry};
//! use nexus_memory::types::EpisodeType;
//!
//! # async fn example() -> Result<(), nexus_memory::MemoryError> {
//! let mgr = MemoryManager::new("/tmp/nexus-memory", MemoryConfig::default(), None)?;
//! mgr.create_space("agent-1")?;
//!
//! // Write a working memory entry
//! let entry = make_working_entry("agent-1", "current_goal", serde_json::json!("research AI safety"));
//! mgr.write("agent-1", "system", entry).await?;
//!
//! // Record an episode
//! let episode = make_episodic_entry(
//!     "agent-1",
//!     EpisodeType::ActionExecuted,
//!     "Searched for papers on AI alignment",
//!     serde_json::json!({"results": 42}),
//!     None,
//!     Some("task-1"),
//! );
//! mgr.write("agent-1", "system", episode).await?;
//! # Ok(())
//! # }
//! ```

pub mod acl;
pub mod audit;
pub mod contradiction;
pub mod embedding;
pub mod episodic;
pub mod gc;
pub mod manager;
pub mod persistence;
pub mod procedural;
pub mod rollback;
pub mod search;
pub mod semantic;
pub mod sharing;
pub mod space;
pub mod tauri_commands;
pub mod types;
pub mod working;

// Re-export key types at crate root for convenience.
pub use acl::MemoryAcl;
pub use contradiction::{Contradiction, ContradictionResolution, ContradictionType};
pub use embedding::{EmbedError, MemoryEmbedder, MockEmbedder, OllamaEmbedder};
pub use gc::{GcConfig, GcReport, MemoryGarbageCollector};
pub use manager::MemoryManager;
pub use procedural::ProceduralMemory;
pub use rollback::{MemoryCheckpoint, RollbackManager, RollbackRecord};
pub use search::{
    CrossTypeRanking, MatchType, MemorySearchEngine, MemorySearchResult, RetrievalPolicy,
};
pub use semantic::SemanticMemory;
pub use sharing::{RevocationResult, SharingManager, TaintMarker};
pub use space::MemorySpace;
pub use types::{
    EpisodeType, EpistemicClass, EpistemicClassFilter, MemoryAccess, MemoryAuditEntry,
    MemoryConfig, MemoryContent, MemoryEntry, MemoryError, MemoryId, MemoryOperation, MemoryQuery,
    MemoryScope, MemoryType, MemoryUsage, ProcedureCandidate, ProcedureExecution, ProcedureState,
    PromotionApprover, PromotionEvidence, PromotionThresholds, RegressionAction, RegressionEvent,
    RegressionTrigger, SensitivityClass, ValidationState,
};
