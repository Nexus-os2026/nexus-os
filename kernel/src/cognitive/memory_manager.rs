//! Agent memory manager — episodic, semantic, and procedural memory via persistence.

use crate::errors::AgentError;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Trait for the persistence operations the memory manager needs.
/// Matches the subset of `StateStore` from nexus-persistence.
pub trait MemoryStore: Send + Sync {
    fn save_memory(
        &self,
        agent_id: &str,
        memory_type: &str,
        key: &str,
        value_json: &str,
    ) -> Result<(), String>;

    fn load_memories(
        &self,
        agent_id: &str,
        memory_type: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, String>;

    fn touch_memory(&self, id: i64) -> Result<(), String>;

    fn decay_memories(&self, agent_id: &str, decay_factor: f64) -> Result<(), String>;
}

/// A memory entry returned from the store.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryEntry {
    pub id: i64,
    pub agent_id: String,
    pub memory_type: String,
    pub key: String,
    pub value_json: String,
    pub relevance_score: f64,
    pub access_count: i64,
    pub created_at: String,
    pub last_accessed: String,
}

/// Manages episodic, semantic, and procedural memory for an agent.
pub struct AgentMemoryManager {
    store: Box<dyn MemoryStore>,
}

impl AgentMemoryManager {
    pub fn new(store: Box<dyn MemoryStore>) -> Self {
        Self { store }
    }

    /// Store an episodic memory (what happened).
    pub fn store_episodic(
        &self,
        agent_id: &str,
        event: &str,
        outcome: &str,
    ) -> Result<(), AgentError> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let key = format!("episode:{timestamp}");
        let value = serde_json::json!({
            "event": event,
            "outcome": outcome,
            "timestamp": timestamp,
        });
        self.store
            .save_memory(agent_id, "episodic", &key, &value.to_string())
            .map_err(|e| AgentError::SupervisorError(format!("memory store error: {e}")))
    }

    /// Store a semantic memory (learned fact).
    pub fn store_semantic(&self, agent_id: &str, fact: &str) -> Result<(), AgentError> {
        let hash = hash_string(fact);
        let key = format!("fact:{hash}");
        let value = serde_json::json!({ "fact": fact });
        self.store
            .save_memory(agent_id, "semantic", &key, &value.to_string())
            .map_err(|e| AgentError::SupervisorError(format!("memory store error: {e}")))
    }

    /// Store a procedural memory (learned strategy with success rate).
    pub fn store_procedural(
        &self,
        agent_id: &str,
        strategy: &str,
        success_rate: f64,
    ) -> Result<(), AgentError> {
        let hash = hash_string(strategy);
        let key = format!("strategy:{hash}");
        let value = serde_json::json!({
            "strategy": strategy,
            "success_rate": success_rate,
        });
        self.store
            .save_memory(agent_id, "procedural", &key, &value.to_string())
            .map_err(|e| AgentError::SupervisorError(format!("memory store error: {e}")))
    }

    /// Recall memories relevant to a query, sorted by relevance.
    /// Touches each returned memory to update access count.
    pub fn recall_relevant(
        &self,
        agent_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, AgentError> {
        let mut memories = self
            .store
            .load_memories(agent_id, None, limit * 3) // load extra for keyword filtering
            .map_err(|e| AgentError::SupervisorError(format!("memory load error: {e}")))?;

        // Keyword-based relevance filtering
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        if !query_words.is_empty() {
            memories.sort_by(|a, b| {
                let score_a = keyword_score(&a.key, &a.value_json, &query_words);
                let score_b = keyword_score(&b.key, &b.value_json, &query_words);
                score_b
                    .partial_cmp(&score_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        memories.truncate(limit);

        // Touch each returned memory
        for mem in &memories {
            let _ = self.store.touch_memory(mem.id);
        }

        Ok(memories)
    }

    /// Run a decay cycle — reduce relevance of stale memories.
    pub fn run_decay_cycle(&self, agent_id: &str) -> Result<(), AgentError> {
        self.store
            .decay_memories(agent_id, 0.95)
            .map_err(|e| AgentError::SupervisorError(format!("memory decay error: {e}")))
    }

    /// Load memories of a specific type, returning the most recent/highest-id entries first.
    pub fn load_by_type(
        &self,
        agent_id: &str,
        memory_type: &str,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, AgentError> {
        let mut memories = self
            .store
            .load_memories(agent_id, Some(memory_type), limit)
            .map_err(|e| AgentError::SupervisorError(format!("memory load error: {e}")))?;
        memories.sort_by_key(|entry| entry.id);
        memories.reverse();
        Ok(memories)
    }
}

fn hash_string(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

fn keyword_score(key: &str, value_json: &str, query_words: &[&str]) -> f64 {
    let combined = format!("{} {}", key.to_lowercase(), value_json.to_lowercase());
    let mut score = 0.0;
    for word in query_words {
        if combined.contains(word) {
            score += 1.0;
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct InMemoryStore {
        memories: Mutex<Vec<MemoryEntry>>,
        next_id: Mutex<i64>,
    }

    impl InMemoryStore {
        fn new() -> Self {
            Self {
                memories: Mutex::new(Vec::new()),
                next_id: Mutex::new(1),
            }
        }
    }

    impl MemoryStore for InMemoryStore {
        fn save_memory(
            &self,
            agent_id: &str,
            memory_type: &str,
            key: &str,
            value_json: &str,
        ) -> Result<(), String> {
            let mut id = self.next_id.lock().unwrap();
            let entry = MemoryEntry {
                id: *id,
                agent_id: agent_id.to_string(),
                memory_type: memory_type.to_string(),
                key: key.to_string(),
                value_json: value_json.to_string(),
                relevance_score: 1.0,
                access_count: 0,
                created_at: "now".to_string(),
                last_accessed: "now".to_string(),
            };
            *id += 1;
            self.memories.lock().unwrap().push(entry);
            Ok(())
        }

        fn load_memories(
            &self,
            agent_id: &str,
            memory_type: Option<&str>,
            limit: usize,
        ) -> Result<Vec<MemoryEntry>, String> {
            let mems = self.memories.lock().unwrap();
            let filtered: Vec<MemoryEntry> = mems
                .iter()
                .filter(|m| m.agent_id == agent_id)
                .filter(|m| memory_type.is_none() || Some(m.memory_type.as_str()) == memory_type)
                .take(limit)
                .cloned()
                .collect();
            Ok(filtered)
        }

        fn touch_memory(&self, id: i64) -> Result<(), String> {
            let mut mems = self.memories.lock().unwrap();
            if let Some(m) = mems.iter_mut().find(|m| m.id == id) {
                m.access_count += 1;
            }
            Ok(())
        }

        fn decay_memories(&self, agent_id: &str, decay_factor: f64) -> Result<(), String> {
            let mut mems = self.memories.lock().unwrap();
            for m in mems.iter_mut().filter(|m| m.agent_id == agent_id) {
                m.relevance_score *= decay_factor;
            }
            Ok(())
        }
    }

    #[test]
    fn test_store_and_recall_episodic() {
        let store = InMemoryStore::new();
        let mgr = AgentMemoryManager::new(Box::new(store));
        mgr.store_episodic("agent1", "ran code analysis", "found 3 bugs")
            .unwrap();
        let memories = mgr.recall_relevant("agent1", "code analysis", 10).unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].memory_type, "episodic");
        assert!(memories[0].value_json.contains("code analysis"));
    }

    #[test]
    fn test_store_semantic() {
        let store = InMemoryStore::new();
        let mgr = AgentMemoryManager::new(Box::new(store));
        mgr.store_semantic("agent1", "Rust is a systems programming language")
            .unwrap();
        let memories = mgr
            .recall_relevant("agent1", "Rust programming", 10)
            .unwrap();
        assert_eq!(memories.len(), 1);
        assert!(memories[0].value_json.contains("Rust"));
    }

    #[test]
    fn test_store_procedural() {
        let store = InMemoryStore::new();
        let mgr = AgentMemoryManager::new(Box::new(store));
        mgr.store_procedural("agent1", "use chunked file reading for large files", 0.85)
            .unwrap();
        let memories = mgr.recall_relevant("agent1", "file reading", 10).unwrap();
        assert_eq!(memories.len(), 1);
        assert!(memories[0].value_json.contains("0.85"));
    }

    #[test]
    fn test_recall_touches_memories() {
        let store = InMemoryStore::new();
        let mgr = AgentMemoryManager::new(Box::new(store));
        mgr.store_semantic("agent1", "fact one").unwrap();
        let memories = mgr.recall_relevant("agent1", "fact", 10).unwrap();
        assert_eq!(memories[0].access_count, 0); // snapshot before touch
                                                 // Touch happens during recall — next recall shows updated count
        let memories = mgr.recall_relevant("agent1", "fact", 10).unwrap();
        assert_eq!(memories[0].access_count, 1);
    }

    #[test]
    fn test_decay_reduces_relevance() {
        let store = InMemoryStore::new();
        let mgr = AgentMemoryManager::new(Box::new(store));
        mgr.store_semantic("agent1", "some fact").unwrap();
        mgr.run_decay_cycle("agent1").unwrap();
        let memories = mgr.recall_relevant("agent1", "fact", 10).unwrap();
        assert!(memories[0].relevance_score < 1.0);
        assert!((memories[0].relevance_score - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_recall_keyword_ranking() {
        let store = InMemoryStore::new();
        let mgr = AgentMemoryManager::new(Box::new(store));
        mgr.store_semantic("agent1", "python is a scripting language")
            .unwrap();
        mgr.store_semantic("agent1", "rust is a systems programming language")
            .unwrap();
        let memories = mgr.recall_relevant("agent1", "rust systems", 10).unwrap();
        assert_eq!(memories.len(), 2);
        // Rust memory should rank first (matches more keywords)
        assert!(memories[0].value_json.contains("rust"));
    }

    #[test]
    fn test_recall_empty() {
        let store = InMemoryStore::new();
        let mgr = AgentMemoryManager::new(Box::new(store));
        let memories = mgr.recall_relevant("agent1", "anything", 10).unwrap();
        assert!(memories.is_empty());
    }

    #[test]
    fn test_different_agents_isolated() {
        let store = InMemoryStore::new();
        let mgr = AgentMemoryManager::new(Box::new(store));
        mgr.store_semantic("agent1", "agent1 fact").unwrap();
        mgr.store_semantic("agent2", "agent2 fact").unwrap();
        let m1 = mgr.recall_relevant("agent1", "fact", 10).unwrap();
        let m2 = mgr.recall_relevant("agent2", "fact", 10).unwrap();
        assert_eq!(m1.len(), 1);
        assert_eq!(m2.len(), 1);
        assert!(m1[0].value_json.contains("agent1"));
        assert!(m2[0].value_json.contains("agent2"));
    }
}
