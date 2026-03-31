//! Frontend integration for the Agent Memory subsystem.
//!
//! Exposes `MemoryKernelState` plus handler functions that the Tauri desktop
//! backend calls via its `#[tauri::command]` bridge.

use std::sync::Mutex;

use serde_json::{json, Value};

use crate::gc::{GcConfig, MemoryGarbageCollector};
use crate::manager::MemoryManager;
use crate::search::RetrievalPolicy;
use crate::sharing::SharingManager;
use crate::types::*;

/// In-memory state held by the Tauri app for the kernel memory subsystem.
pub struct MemoryKernelState {
    pub manager: Mutex<MemoryManager>,
    pub sharing: Mutex<SharingManager>,
    pub gc: MemoryGarbageCollector,
}

impl MemoryKernelState {
    /// Creates state with a real on-disk MemoryManager.
    pub fn new(data_dir: &str) -> Result<Self, String> {
        let manager = MemoryManager::new(data_dir, MemoryConfig::default(), None)
            .map_err(|e| format!("memory init: {e}"))?;
        Ok(Self {
            manager: Mutex::new(manager),
            sharing: Mutex::new(SharingManager::new()),
            gc: MemoryGarbageCollector::new(GcConfig::default()),
        })
    }
}

impl Default for MemoryKernelState {
    fn default() -> Self {
        let manager =
            MemoryManager::in_memory(MemoryConfig::default()).expect("in-memory MemoryManager");
        Self {
            manager: Mutex::new(manager),
            sharing: Mutex::new(SharingManager::new()),
            gc: MemoryGarbageCollector::new(GcConfig::default()),
        }
    }
}

// ── Read operations ──────────────────────────────────────────────────────────

/// Returns memory usage stats for an agent.
pub fn memory_get_stats(state: &MemoryKernelState, agent_id: &str) -> Result<Value, String> {
    let mgr = state.manager.lock().map_err(|e| format!("lock: {e}"))?;
    let space = mgr.get_space(agent_id).map_err(|e| format!("{e}"))?;
    let usage = space.usage();
    serde_json::to_value(usage).map_err(|e| format!("serialize: {e}"))
}

/// Queries an agent's memory by type.
pub fn memory_query(
    state: &MemoryKernelState,
    agent_id: &str,
    memory_type: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<Value>, String> {
    let mgr = state.manager.lock().map_err(|e| format!("lock: {e}"))?;
    let space = mgr.get_space(agent_id).map_err(|e| format!("{e}"))?;

    let types = match memory_type {
        Some(t) => Some(vec![parse_memory_type(t)?]),
        None => None,
    };

    let query = MemoryQuery {
        memory_types: types,
        limit,
        ..Default::default()
    };

    let entries = space.query(&query).map_err(|e| format!("{e}"))?;
    entries
        .iter()
        .map(|e| serde_json::to_value(e).map_err(|e| format!("serialize: {e}")))
        .collect()
}

/// Searches across an agent's memory with semantic + keyword matching.
pub fn memory_search(
    state: &MemoryKernelState,
    agent_id: &str,
    query_text: &str,
    policy_name: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<Value>, String> {
    let mgr = state.manager.lock().map_err(|e| format!("lock: {e}"))?;
    let space = mgr.get_space(agent_id).map_err(|e| format!("{e}"))?;

    let policy = match policy_name {
        Some("planning") => RetrievalPolicy::for_planning(),
        Some("execution") => RetrievalPolicy::for_execution(),
        Some("safety") => RetrievalPolicy::for_safety(),
        _ => RetrievalPolicy::for_planning(),
    };

    // Collect entries for search
    let mut all_entries: Vec<MemoryEntry> = Vec::new();
    for mt in &policy.include_types {
        match mt {
            MemoryType::Working => {
                for e in space.working.all() {
                    all_entries.push(e.clone());
                }
            }
            MemoryType::Episodic => {
                for e in space.episodic.all() {
                    all_entries.push(e.clone());
                }
            }
            MemoryType::Semantic => {
                for e in space.semantic.all() {
                    all_entries.push(e.clone());
                }
            }
            MemoryType::Procedural => {
                for e in space.procedural.all_procedures() {
                    all_entries.push(e.clone());
                }
            }
        }
    }

    let results =
        mgr.search_engine()
            .search(&all_entries, query_text, None, &policy, limit.unwrap_or(20));

    results
        .iter()
        .map(|r| {
            serde_json::to_value(json!({
                "entry": r.entry,
                "relevance_score": r.relevance_score,
                "match_type": format!("{:?}", r.match_type),
            }))
            .map_err(|e| format!("serialize: {e}"))
        })
        .collect()
}

/// Returns recent audit entries for an agent (returns summary, not full entries).
pub fn memory_get_audit(
    state: &MemoryKernelState,
    agent_id: &str,
    _limit: Option<usize>,
) -> Result<Vec<Value>, String> {
    // Audit query is async but Tauri commands are sync.
    // Return a placeholder — the full audit is accessible via the Audit page.
    let _mgr = state.manager.lock().map_err(|e| format!("lock: {e}"))?;
    Ok(vec![json!({
        "agent_id": agent_id,
        "note": "Use the Audit page for full memory audit trail"
    })])
}

/// Returns active procedures for an agent.
pub fn memory_get_procedures(
    state: &MemoryKernelState,
    agent_id: &str,
) -> Result<Vec<Value>, String> {
    let mgr = state.manager.lock().map_err(|e| format!("lock: {e}"))?;
    let space = mgr.get_space(agent_id).map_err(|e| format!("{e}"))?;

    space
        .procedural
        .all_procedures()
        .iter()
        .map(|e| serde_json::to_value(e).map_err(|e| format!("serialize: {e}")))
        .collect()
}

/// Returns procedure candidates for an agent.
pub fn memory_get_candidates(
    state: &MemoryKernelState,
    agent_id: &str,
) -> Result<Vec<Value>, String> {
    let mgr = state.manager.lock().map_err(|e| format!("lock: {e}"))?;
    let space = mgr.get_space(agent_id).map_err(|e| format!("{e}"))?;

    space
        .procedural
        .all_candidates()
        .iter()
        .map(|c| serde_json::to_value(c).map_err(|e| format!("serialize: {e}")))
        .collect()
}

// ── Write operations ─────────────────────────────────────────────────────────

/// Writes a memory entry.  Returns the MemoryId.
pub fn memory_write(
    state: &MemoryKernelState,
    agent_id: &str,
    memory_type: &str,
    content: Value,
) -> Result<String, String> {
    let mgr = state.manager.lock().map_err(|e| format!("lock: {e}"))?;

    // Ensure space exists
    mgr.create_space(agent_id).map_err(|e| format!("{e}"))?;

    let mt = parse_memory_type(memory_type)?;
    let mc: MemoryContent =
        serde_json::from_value(content).map_err(|e| format!("parse content: {e}"))?;

    let entry = build_entry(agent_id, mt, mc);

    // Write directly to the space (bypasses async audit for sync Tauri commands)
    let id = {
        let mut space = mgr.get_space_mut(agent_id).map_err(|e| format!("{e}"))?;
        space.write(entry.clone()).map_err(|e| format!("{e}"))?
    };

    // Best-effort persist
    let _ = mgr.persistence().save_entry(&entry);

    Ok(id.to_string())
}

/// Clears an agent's working memory.
pub fn memory_clear_working(state: &MemoryKernelState, agent_id: &str) -> Result<bool, String> {
    let mgr = state.manager.lock().map_err(|e| format!("lock: {e}"))?;
    let mut space = mgr.get_space_mut(agent_id).map_err(|e| format!("{e}"))?;
    space.clear_working();
    Ok(true)
}

// ── Governance operations ────────────────────────────────────────────────────

/// Shares memory between agents.
pub fn memory_share(
    state: &MemoryKernelState,
    owner_id: &str,
    grantee_id: &str,
    read_types: Vec<String>,
    write_types: Vec<String>,
) -> Result<bool, String> {
    let mgr = state.manager.lock().map_err(|e| format!("lock: {e}"))?;

    let read: Vec<MemoryType> = read_types
        .iter()
        .map(|t| parse_memory_type(t))
        .collect::<Result<_, _>>()?;
    let write: Vec<MemoryType> = write_types
        .iter()
        .map(|t| parse_memory_type(t))
        .collect::<Result<_, _>>()?;

    let access = MemoryAccess {
        read,
        write,
        search: true,
        share: false,
    };

    // Grant in the owner's space ACL
    let mut space = mgr.get_space_mut(owner_id).map_err(|e| format!("{e}"))?;
    space
        .acl
        .grant_access(grantee_id, access.clone())
        .map_err(|e| format!("{e}"))?;

    // Register in sharing manager
    let mut sharing = state.sharing.lock().map_err(|e| format!("lock: {e}"))?;
    sharing.register_share(owner_id, grantee_id, access);

    Ok(true)
}

/// Revokes a share.
pub fn memory_revoke_share(
    state: &MemoryKernelState,
    owner_id: &str,
    grantee_id: &str,
) -> Result<Value, String> {
    let mgr = state.manager.lock().map_err(|e| format!("lock: {e}"))?;

    // Revoke ACL
    if let Ok(mut space) = mgr.get_space_mut(owner_id) {
        let _ = space.acl.revoke_access(grantee_id);
    }

    // Revoke in sharing manager
    let mut sharing = state.sharing.lock().map_err(|e| format!("lock: {e}"))?;
    let result = sharing.revoke_share(owner_id, grantee_id);

    Ok(json!({
        "tainted_entries": result.tainted_entry_ids.len(),
        "markers_removed": result.taint_markers_removed,
    }))
}

/// Manually triggers garbage collection.
pub fn memory_run_gc(state: &MemoryKernelState) -> Result<Value, String> {
    let mgr = state.manager.lock().map_err(|e| format!("lock: {e}"))?;

    let mut total_scanned = 0usize;
    let mut total_working = 0usize;
    let mut total_semantic = 0usize;
    let mut total_procedural = 0usize;

    for mut space_ref in mgr.spaces_iter_mut() {
        let space = space_ref.value_mut();
        let report = state.gc.run(space);
        total_scanned += report.entries_scanned;
        total_working += report.working_cleared;
        total_semantic += report.semantic_soft_deleted;
        total_procedural += report.procedural_demoted;
    }

    Ok(json!({
        "entries_scanned": total_scanned,
        "working_cleared": total_working,
        "semantic_soft_deleted": total_semantic,
        "procedural_demoted": total_procedural,
    }))
}

// ── Checkpoint / Rollback ────────────────────────────────────────────────────

/// Creates a memory checkpoint.
pub fn memory_create_checkpoint(
    state: &MemoryKernelState,
    agent_id: &str,
    label: &str,
) -> Result<String, String> {
    let mgr = state.manager.lock().map_err(|e| format!("lock: {e}"))?;
    let mut space = mgr.get_space_mut(agent_id).map_err(|e| format!("{e}"))?;

    let cp = space.create_checkpoint(label);
    Ok(cp.id.to_string())
}

/// Rolls back to a checkpoint.
pub fn memory_rollback(
    state: &MemoryKernelState,
    agent_id: &str,
    checkpoint_id: &str,
    reason: &str,
) -> Result<Value, String> {
    let mgr = state.manager.lock().map_err(|e| format!("lock: {e}"))?;
    let mut space = mgr.get_space_mut(agent_id).map_err(|e| format!("{e}"))?;

    let cp_id: uuid::Uuid = checkpoint_id
        .parse()
        .map_err(|e| format!("invalid checkpoint_id: {e}"))?;

    let record = space.rollback(cp_id, reason).map_err(|e| format!("{e}"))?;
    serde_json::to_value(&record).map_err(|e| format!("serialize: {e}"))
}

/// Lists checkpoints for an agent.
pub fn memory_list_checkpoints(
    state: &MemoryKernelState,
    agent_id: &str,
) -> Result<Vec<Value>, String> {
    let mgr = state.manager.lock().map_err(|e| format!("lock: {e}"))?;
    let space = mgr.get_space(agent_id).map_err(|e| format!("{e}"))?;

    space
        .rollback_mgr
        .list_checkpoints(agent_id)
        .iter()
        .map(|cp| serde_json::to_value(cp).map_err(|e| format!("serialize: {e}")))
        .collect()
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn parse_memory_type(s: &str) -> Result<MemoryType, String> {
    match s.to_lowercase().as_str() {
        "working" => Ok(MemoryType::Working),
        "episodic" => Ok(MemoryType::Episodic),
        "semantic" => Ok(MemoryType::Semantic),
        "procedural" => Ok(MemoryType::Procedural),
        other => Err(format!("Unknown memory type: {other}")),
    }
}

fn build_entry(agent_id: &str, mt: MemoryType, content: MemoryContent) -> MemoryEntry {
    let now = chrono::Utc::now();
    MemoryEntry {
        id: uuid::Uuid::new_v4(),
        schema_version: 1,
        agent_id: agent_id.into(),
        memory_type: mt,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> MemoryKernelState {
        MemoryKernelState::default()
    }

    #[test]
    fn get_stats_for_existing_agent() {
        let state = make_state();
        {
            let mgr = state.manager.lock().unwrap();
            mgr.create_space("agent-1").unwrap();
        }
        let stats = memory_get_stats(&state, "agent-1").unwrap();
        assert!(stats.get("agent_id").is_some());
    }

    #[test]
    fn get_stats_nonexistent_agent_errors() {
        let state = make_state();
        let result = memory_get_stats(&state, "nope");
        assert!(result.is_err());
    }

    #[test]
    fn write_and_query_working() {
        let state = make_state();
        {
            let mgr = state.manager.lock().unwrap();
            mgr.create_space("a").unwrap();
        }
        let id = memory_write(
            &state,
            "a",
            "working",
            json!({"Context": {"key": "goal", "value": "test"}}),
        )
        .unwrap();
        assert!(!id.is_empty());

        let entries = memory_query(&state, "a", Some("working"), None).unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn search_returns_results() {
        let state = make_state();
        {
            let mgr = state.manager.lock().unwrap();
            mgr.create_space("a").unwrap();
        }
        memory_write(
            &state,
            "a",
            "semantic",
            json!({"Assertion": {"statement": "Rust is fast", "citations": []}}),
        )
        .unwrap();

        let results = memory_search(&state, "a", "Rust fast", Some("planning"), Some(10)).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn clear_working_empties() {
        let state = make_state();
        {
            let mgr = state.manager.lock().unwrap();
            mgr.create_space("a").unwrap();
        }
        memory_write(
            &state,
            "a",
            "working",
            json!({"Context": {"key": "k1", "value": 1}}),
        )
        .unwrap();

        assert!(memory_clear_working(&state, "a").unwrap());

        let entries = memory_query(&state, "a", Some("working"), None).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn checkpoint_and_list() {
        let state = make_state();
        {
            let mgr = state.manager.lock().unwrap();
            mgr.create_space("a").unwrap();
        }
        let cp_id = memory_create_checkpoint(&state, "a", "test-cp").unwrap();
        assert!(!cp_id.is_empty());

        let cps = memory_list_checkpoints(&state, "a").unwrap();
        assert_eq!(cps.len(), 1);
    }

    #[test]
    fn gc_returns_report() {
        let state = make_state();
        {
            let mgr = state.manager.lock().unwrap();
            mgr.create_space("a").unwrap();
        }
        let report = memory_run_gc(&state).unwrap();
        assert!(report.get("entries_scanned").is_some());
    }

    #[test]
    fn share_and_revoke() {
        let state = make_state();
        {
            let mgr = state.manager.lock().unwrap();
            mgr.create_space("owner").unwrap();
        }
        let shared =
            memory_share(&state, "owner", "reader", vec!["semantic".into()], vec![]).unwrap();
        assert!(shared);

        let result = memory_revoke_share(&state, "owner", "reader").unwrap();
        assert!(result.get("tainted_entries").is_some());
    }

    #[test]
    fn parse_memory_type_valid() {
        assert_eq!(parse_memory_type("working").unwrap(), MemoryType::Working);
        assert_eq!(parse_memory_type("Episodic").unwrap(), MemoryType::Episodic);
        assert!(parse_memory_type("invalid").is_err());
    }
}
