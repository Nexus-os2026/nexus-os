//! Frontend integration types.

use std::collections::HashMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::consolidation::MemoryConsolidator;
use crate::context::{ContextBuilder, MemoryContext};
use crate::governance::MemoryPolicy;
use crate::index::MemoryIndex;
use crate::persistence::MemoryPersistence;
use crate::store::AgentMemoryStore;
use crate::types::{Memory, MemoryContent, MemoryMetadata, MemoryQuery, MemoryType, Valence};

/// In-memory state held by the Tauri app.
pub struct MemoryState {
    pub stores: RwLock<HashMap<String, AgentMemoryStore>>,
    pub index: RwLock<MemoryIndex>,
    pub policy: MemoryPolicy,
    pub data_dir: String,
}

impl Default for MemoryState {
    fn default() -> Self {
        Self {
            stores: RwLock::new(HashMap::new()),
            index: RwLock::new(MemoryIndex::new()),
            policy: MemoryPolicy::default(),
            data_dir: "data".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total: usize,
    pub by_type: HashMap<String, usize>,
    pub oldest: Option<u64>,
    pub newest: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationResult {
    pub merge_candidates: usize,
    pub forgettable: usize,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

fn get_or_create_store<'a>(
    stores: &'a mut HashMap<String, AgentMemoryStore>,
    agent_id: &str,
    max: usize,
) -> &'a mut AgentMemoryStore {
    stores
        .entry(agent_id.to_string())
        .or_insert_with(|| AgentMemoryStore::new(agent_id.into(), max))
}

pub fn memory_store(
    state: &MemoryState,
    agent_id: &str,
    memory_type: &str,
    summary: &str,
    tags: Vec<String>,
    importance: f64,
    domain: Option<String>,
) -> Result<String, String> {
    let mtype = parse_memory_type(memory_type)?;
    let memory = Memory {
        id: String::new(),
        agent_id: agent_id.into(),
        memory_type: mtype,
        content: MemoryContent {
            summary: summary.into(),
            data: None,
            raw_input: None,
            outcome: None,
        },
        metadata: MemoryMetadata {
            source_task: None,
            task_quality: None,
            related_memories: Vec::new(),
            domain,
            valence: Valence::Neutral,
            confidence: 0.8,
        },
        importance: importance.clamp(0.0, 1.0),
        access_count: 0,
        last_accessed: 0,
        created_at: 0,
        consolidated: false,
        tags,
    };

    let mut stores = state.stores.write().map_err(|e| format!("lock: {e}"))?;
    let store = get_or_create_store(&mut stores, agent_id, state.policy.max_memories_per_agent);
    let id = store.store(memory.clone());

    // Index
    let stored = store.get(&id).cloned();
    if let Some(m) = stored {
        if let Ok(mut idx) = state.index.write() {
            idx.index(&m);
        }
    }

    Ok(id)
}

pub fn memory_query(
    state: &MemoryState,
    agent_id: &str,
    query: &str,
    memory_type: Option<String>,
    tags: Option<Vec<String>>,
    limit: usize,
) -> Result<Vec<Memory>, String> {
    let mtype = memory_type.as_deref().map(parse_memory_type).transpose()?;
    let q = MemoryQuery {
        query: query.into(),
        memory_type: mtype,
        tags,
        limit: limit.clamp(1, 100),
        ..Default::default()
    };

    let mut stores = state.stores.write().map_err(|e| format!("lock: {e}"))?;
    let store = get_or_create_store(&mut stores, agent_id, state.policy.max_memories_per_agent);
    Ok(store.query(&q))
}

pub fn memory_get(state: &MemoryState, agent_id: &str, memory_id: &str) -> Result<Memory, String> {
    let stores = state.stores.read().map_err(|e| format!("lock: {e}"))?;
    stores
        .get(agent_id)
        .and_then(|s| s.get(memory_id).cloned())
        .ok_or_else(|| "Memory not found".into())
}

pub fn memory_delete(state: &MemoryState, agent_id: &str, memory_id: &str) -> Result<bool, String> {
    let mut stores = state.stores.write().map_err(|e| format!("lock: {e}"))?;
    let deleted = stores
        .get_mut(agent_id)
        .map(|s| s.delete(memory_id))
        .unwrap_or(false);

    if deleted {
        if let Ok(mut idx) = state.index.write() {
            idx.remove(memory_id);
        }
    }
    Ok(deleted)
}

pub fn memory_build_context(
    state: &MemoryState,
    agent_id: &str,
    task_description: &str,
    max_memories: usize,
) -> Result<MemoryContext, String> {
    let mut stores = state.stores.write().map_err(|e| format!("lock: {e}"))?;
    let store = get_or_create_store(&mut stores, agent_id, state.policy.max_memories_per_agent);
    Ok(ContextBuilder::build_context(
        store,
        task_description,
        max_memories.clamp(1, 20),
        4000,
    ))
}

pub fn memory_get_stats(state: &MemoryState, agent_id: &str) -> Result<MemoryStats, String> {
    let stores = state.stores.read().map_err(|e| format!("lock: {e}"))?;
    let store = stores
        .get(agent_id)
        .ok_or_else(|| "No memories for agent".to_string())?;

    let memories = store.all();
    let oldest = memories.iter().map(|m| m.created_at).min();
    let newest = memories.iter().map(|m| m.created_at).max();

    Ok(MemoryStats {
        total: store.len(),
        by_type: store.count_by_type(),
        oldest,
        newest,
    })
}

pub fn memory_consolidate(
    state: &MemoryState,
    agent_id: &str,
) -> Result<ConsolidationResult, String> {
    let stores = state.stores.read().map_err(|e| format!("lock: {e}"))?;
    let store = stores
        .get(agent_id)
        .ok_or_else(|| "No memories for agent".to_string())?;

    let memories = store.all();
    let merge_candidates = MemoryConsolidator::merge_similar(memories, 0.5);
    let forgettable = MemoryConsolidator::identify_forgettable(memories, 30);

    Ok(ConsolidationResult {
        merge_candidates: merge_candidates.len(),
        forgettable: forgettable.len(),
    })
}

pub fn memory_save(state: &MemoryState, agent_id: &str) -> Result<String, String> {
    let stores = state.stores.read().map_err(|e| format!("lock: {e}"))?;
    let store = stores
        .get(agent_id)
        .ok_or_else(|| "No memories for agent".to_string())?;
    MemoryPersistence::save(store, &state.data_dir)?;
    Ok(format!("Saved {} memories", store.len()))
}

pub fn memory_load(state: &MemoryState, agent_id: &str) -> Result<String, String> {
    let loaded = MemoryPersistence::load(
        agent_id,
        &state.data_dir,
        state.policy.max_memories_per_agent,
    );
    let count = loaded.len();

    // Re-index
    if let Ok(mut idx) = state.index.write() {
        for m in loaded.all() {
            idx.index(m);
        }
    }

    let mut stores = state.stores.write().map_err(|e| format!("lock: {e}"))?;
    stores.insert(agent_id.into(), loaded);
    Ok(format!("Loaded {count} memories"))
}

pub fn memory_list_agents(state: &MemoryState) -> Vec<String> {
    MemoryPersistence::list_agents(&state.data_dir)
}

pub fn memory_get_policy(state: &MemoryState) -> MemoryPolicy {
    state.policy.clone()
}

fn parse_memory_type(s: &str) -> Result<MemoryType, String> {
    match s.to_lowercase().as_str() {
        "episodic" => Ok(MemoryType::Episodic),
        "semantic" => Ok(MemoryType::Semantic),
        "procedural" => Ok(MemoryType::Procedural),
        "relational" => Ok(MemoryType::Relational),
        other => Err(format!("Unknown memory type: {other}")),
    }
}
