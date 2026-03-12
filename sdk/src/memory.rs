//! Agent Long-Term Memory — persistent vector memory across sessions so agents
//! remember context from weeks ago.
//!
//! Each agent has an isolated memory store.  Entries carry an importance score
//! that decays over time, and low-importance entries are automatically evicted.
//! Persistence is plain JSON files at `~/.nexus/memory/{agent_id}.json`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single long-term memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub agent_id: String,
    pub content: String,
    pub memory_type: MemoryType,
    /// Importance score in the range `0.0..=1.0`.
    pub importance: f64,
    pub created_at: u64,
    pub last_accessed: u64,
    pub access_count: u32,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
}

/// Classification of what the memory represents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MemoryType {
    Fact,
    Preference,
    Conversation,
    Task,
    Error,
    Strategy,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fact => write!(f, "Fact"),
            Self::Preference => write!(f, "Preference"),
            Self::Conversation => write!(f, "Conversation"),
            Self::Task => write!(f, "Task"),
            Self::Error => write!(f, "Error"),
            Self::Strategy => write!(f, "Strategy"),
        }
    }
}

/// Configuration for the memory subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub max_entries_per_agent: usize,
    /// Whether importance decays over time.
    pub decay_enabled: bool,
    /// Importance reduction per day of inactivity.
    pub decay_rate: f64,
    /// Entries below this importance are auto-evicted.
    pub min_importance: f64,
    /// Directory where per-agent JSON files are persisted.
    pub persistence_dir: String,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self {
            max_entries_per_agent: 1000,
            decay_enabled: true,
            decay_rate: 0.01,
            min_importance: 0.1,
            persistence_dir: format!("{home}/.nexus/memory"),
        }
    }
}

/// Aggregate statistics about an agent's memory store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total: usize,
    pub by_type: HashMap<String, usize>,
    pub avg_importance: f64,
    pub oldest: Option<u64>,
    pub newest: Option<u64>,
}

// ---------------------------------------------------------------------------
// AgentMemory
// ---------------------------------------------------------------------------

/// Long-term memory store for all agents.
pub struct AgentMemory {
    config: MemoryConfig,
    memories: HashMap<String, Vec<MemoryEntry>>,
}

impl AgentMemory {
    pub fn new(config: MemoryConfig) -> Self {
        Self {
            config,
            memories: HashMap::new(),
        }
    }

    /// Store a new memory entry.
    pub fn remember(
        &mut self,
        agent_id: &str,
        content: &str,
        memory_type: MemoryType,
        importance: f64,
        tags: Vec<String>,
    ) -> MemoryEntry {
        let now = now_secs();
        let entry = MemoryEntry {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            content: content.to_string(),
            memory_type,
            importance: importance.clamp(0.0, 1.0),
            created_at: now,
            last_accessed: now,
            access_count: 0,
            tags,
            metadata: serde_json::Value::Null,
        };

        let entries = self.memories.entry(agent_id.to_string()).or_default();
        entries.push(entry.clone());

        // If over capacity, evict lowest-importance entry.
        while entries.len() > self.config.max_entries_per_agent {
            if let Some(min_idx) = entries
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    a.importance
                        .partial_cmp(&b.importance)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
            {
                entries.remove(min_idx);
            }
        }

        entry
    }

    /// Text-based recall: scores by keyword overlap + importance + recency.
    pub fn recall(&mut self, agent_id: &str, query: &str, max_results: usize) -> Vec<&MemoryEntry> {
        let now = now_secs();
        let entries = match self.memories.get_mut(agent_id) {
            Some(e) => e,
            None => return Vec::new(),
        };

        // Update access metadata for matched entries.
        let query_lower = query.to_lowercase();
        let mut scored: Vec<(usize, f64)> = entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let score = recall_score(e, &query_lower, now);
                if score > 0.0 {
                    Some((i, score))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max_results);

        // Bump access counts on returned entries.
        for &(idx, _) in &scored {
            entries[idx].last_accessed = now;
            entries[idx].access_count += 1;
        }

        let indices: Vec<usize> = scored.iter().map(|(i, _)| *i).collect();
        // Return references (re-borrow immutably).
        let entries = self.memories.get(agent_id).unwrap();
        indices.iter().map(|&i| &entries[i]).collect()
    }

    /// Recall entries filtered by type.
    pub fn recall_by_type(
        &self,
        agent_id: &str,
        memory_type: &MemoryType,
        max_results: usize,
    ) -> Vec<&MemoryEntry> {
        let entries = match self.memories.get(agent_id) {
            Some(e) => e,
            None => return Vec::new(),
        };

        let mut matched: Vec<&MemoryEntry> = entries
            .iter()
            .filter(|e| &e.memory_type == memory_type)
            .collect();

        matched.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matched.truncate(max_results);
        matched
    }

    /// Remove a specific memory entry.  Returns `true` if found.
    pub fn forget(&mut self, agent_id: &str, memory_id: &str) -> bool {
        if let Some(entries) = self.memories.get_mut(agent_id) {
            let before = entries.len();
            entries.retain(|e| e.id != memory_id);
            entries.len() < before
        } else {
            false
        }
    }

    /// Apply time-based importance decay to all entries for an agent.
    pub fn decay_importance(&mut self, agent_id: &str) {
        if !self.config.decay_enabled {
            return;
        }
        let now = now_secs();
        let rate = self.config.decay_rate;
        if let Some(entries) = self.memories.get_mut(agent_id) {
            for e in entries.iter_mut() {
                let days_since_access = (now.saturating_sub(e.last_accessed)) as f64 / 86400.0;
                let decay = rate * days_since_access;
                e.importance = (e.importance - decay).max(0.0);
            }
        }
    }

    /// Evict entries whose importance has dropped below `min_importance`.
    /// Returns the number of entries removed.
    pub fn evict_low_importance(&mut self, agent_id: &str) -> usize {
        let threshold = self.config.min_importance;
        if let Some(entries) = self.memories.get_mut(agent_id) {
            let before = entries.len();
            entries.retain(|e| e.importance >= threshold);
            before - entries.len()
        } else {
            0
        }
    }

    /// Look up a specific entry by ID.
    pub fn get_memory(&self, agent_id: &str, memory_id: &str) -> Option<&MemoryEntry> {
        self.memories
            .get(agent_id)?
            .iter()
            .find(|e| e.id == memory_id)
    }

    /// Aggregate statistics for an agent's memory store.
    pub fn get_stats(&self, agent_id: &str) -> MemoryStats {
        let entries = self.memories.get(agent_id);
        let entries = match entries {
            Some(e) => e,
            None => {
                return MemoryStats {
                    total: 0,
                    by_type: HashMap::new(),
                    avg_importance: 0.0,
                    oldest: None,
                    newest: None,
                }
            }
        };

        let mut by_type: HashMap<String, usize> = HashMap::new();
        let mut sum_importance: f64 = 0.0;
        let mut oldest: Option<u64> = None;
        let mut newest: Option<u64> = None;

        for e in entries {
            *by_type.entry(e.memory_type.to_string()).or_insert(0) += 1;
            sum_importance += e.importance;
            oldest = Some(oldest.map_or(e.created_at, |o: u64| o.min(e.created_at)));
            newest = Some(newest.map_or(e.created_at, |n: u64| n.max(e.created_at)));
        }

        MemoryStats {
            total: entries.len(),
            by_type,
            avg_importance: if entries.is_empty() {
                0.0
            } else {
                sum_importance / entries.len() as f64
            },
            oldest,
            newest,
        }
    }

    /// Persist an agent's memories to `persistence_dir/{agent_id}.json`.
    pub fn save(&self, agent_id: &str) -> Result<(), String> {
        let entries = self.memories.get(agent_id).cloned().unwrap_or_default();
        let dir = &self.config.persistence_dir;
        std::fs::create_dir_all(dir).map_err(|e| format!("create dir: {e}"))?;
        let path = format!("{dir}/{agent_id}.json");
        let json = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| format!("write: {e}"))
    }

    /// Load an agent's memories from disk, replacing any in-memory entries.
    pub fn load(&mut self, agent_id: &str) -> Result<(), String> {
        let path = format!("{}/{agent_id}.json", self.config.persistence_dir);
        let data = std::fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?;
        let entries: Vec<MemoryEntry> =
            serde_json::from_str(&data).map_err(|e| format!("parse: {e}"))?;
        self.memories.insert(agent_id.to_string(), entries);
        Ok(())
    }

    /// Remove all memories for an agent (in-memory only, does not delete file).
    pub fn clear(&mut self, agent_id: &str) {
        self.memories.remove(agent_id);
    }

    /// Read-only access to the config.
    pub fn config(&self) -> &MemoryConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Score a memory entry against a query, factoring in keyword overlap,
/// importance, and recency.
fn recall_score(entry: &MemoryEntry, query_lower: &str, now: u64) -> f64 {
    let content_lower = entry.content.to_lowercase();

    // Keyword overlap
    let mut keyword_score: f64 = 0.0;
    if content_lower.contains(query_lower) {
        keyword_score = 1.0;
    } else {
        let words: Vec<&str> = query_lower.split_whitespace().collect();
        if !words.is_empty() {
            let hits = words.iter().filter(|w| content_lower.contains(*w)).count();
            keyword_score = 0.5 * (hits as f64 / words.len() as f64);
        }
    }

    // Tag match bonus
    for tag in &entry.tags {
        if tag.to_lowercase().contains(query_lower) {
            keyword_score += 0.3;
        }
    }

    if keyword_score <= 0.0 {
        return 0.0;
    }

    // Importance weight
    let importance_factor = entry.importance;

    // Recency boost: entries accessed recently get a bonus (max 0.2 for
    // entries accessed within the last hour, tapering to 0 over a week).
    let secs_since = now.saturating_sub(entry.last_accessed) as f64;
    let recency_boost = 0.2 * (1.0 - (secs_since / 604800.0).min(1.0));

    keyword_score + importance_factor * 0.5 + recency_boost
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> MemoryConfig {
        MemoryConfig {
            max_entries_per_agent: 1000,
            decay_enabled: true,
            decay_rate: 0.01,
            min_importance: 0.1,
            persistence_dir: "/tmp/nexus-memory-test".to_string(),
        }
    }

    fn test_memory() -> AgentMemory {
        AgentMemory::new(test_config())
    }

    #[test]
    fn test_remember_and_recall() {
        let mut mem = test_memory();
        mem.remember("a1", "rust is fast", MemoryType::Fact, 0.8, vec![]);
        mem.remember("a1", "python is dynamic", MemoryType::Fact, 0.6, vec![]);

        let results = mem.recall("a1", "rust", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("rust"));
    }

    #[test]
    fn test_recall_by_type() {
        let mut mem = test_memory();
        mem.remember("a1", "fact one", MemoryType::Fact, 0.5, vec![]);
        mem.remember("a1", "pref one", MemoryType::Preference, 0.7, vec![]);
        mem.remember("a1", "fact two", MemoryType::Fact, 0.9, vec![]);

        let facts = mem.recall_by_type("a1", &MemoryType::Fact, 10);
        assert_eq!(facts.len(), 2);
        // Should be sorted by importance descending
        assert!(facts[0].importance >= facts[1].importance);

        let prefs = mem.recall_by_type("a1", &MemoryType::Preference, 10);
        assert_eq!(prefs.len(), 1);
    }

    #[test]
    fn test_forget() {
        let mut mem = test_memory();
        let entry = mem.remember("a1", "forget me", MemoryType::Fact, 0.5, vec![]);
        assert!(mem.forget("a1", &entry.id));
        assert!(mem.get_memory("a1", &entry.id).is_none());
        // Double forget
        assert!(!mem.forget("a1", &entry.id));
    }

    #[test]
    fn test_importance_ordering() {
        let mut mem = test_memory();
        mem.remember("a1", "low importance data", MemoryType::Fact, 0.2, vec![]);
        mem.remember("a1", "high importance data", MemoryType::Fact, 0.9, vec![]);
        mem.remember("a1", "mid importance data", MemoryType::Fact, 0.5, vec![]);

        let results = mem.recall("a1", "data", 10);
        assert_eq!(results.len(), 3);
        // Highest importance should rank first (importance is a factor in score)
        assert!(results[0].importance >= results[1].importance);
    }

    #[test]
    fn test_recency_boost() {
        let mut mem = test_memory();
        let e1 = mem.remember("a1", "old info", MemoryType::Fact, 0.5, vec![]);
        mem.remember("a1", "new info", MemoryType::Fact, 0.5, vec![]);

        // Backdate the first entry
        let entries = mem.memories.get_mut("a1").unwrap();
        entries
            .iter_mut()
            .find(|e| e.id == e1.id)
            .unwrap()
            .last_accessed -= 604800; // 1 week ago

        let results = mem.recall("a1", "info", 10);
        assert_eq!(results.len(), 2);
        // More recent entry should score higher
        assert!(results[0].content.contains("new"));
    }

    #[test]
    fn test_decay_importance() {
        let mut mem = test_memory();
        let entry = mem.remember("a1", "decaying", MemoryType::Fact, 0.5, vec![]);

        // Simulate last access was 10 days ago
        let entries = mem.memories.get_mut("a1").unwrap();
        entries
            .iter_mut()
            .find(|e| e.id == entry.id)
            .unwrap()
            .last_accessed -= 10 * 86400;

        mem.decay_importance("a1");

        let updated = mem.get_memory("a1", &entry.id).unwrap();
        // decay = 0.01 * 10 = 0.1, so importance should be ~0.4
        assert!(updated.importance < 0.5);
        assert!(updated.importance > 0.3);
    }

    #[test]
    fn test_evict_low_importance() {
        let mut mem = test_memory();
        mem.remember("a1", "important", MemoryType::Fact, 0.8, vec![]);
        mem.remember("a1", "trivial", MemoryType::Fact, 0.05, vec![]);
        mem.remember("a1", "borderline", MemoryType::Fact, 0.1, vec![]);

        let evicted = mem.evict_low_importance("a1");
        assert_eq!(evicted, 1); // only "trivial" (0.05 < 0.1)
        assert_eq!(mem.get_stats("a1").total, 2);
    }

    #[test]
    fn test_max_entries() {
        let mut mem = AgentMemory::new(MemoryConfig {
            max_entries_per_agent: 3,
            ..test_config()
        });

        mem.remember("a1", "e1", MemoryType::Fact, 0.9, vec![]);
        mem.remember("a1", "e2", MemoryType::Fact, 0.1, vec![]);
        mem.remember("a1", "e3", MemoryType::Fact, 0.8, vec![]);
        mem.remember("a1", "e4", MemoryType::Fact, 0.7, vec![]);

        // Should have 3 entries; lowest importance (0.1) evicted
        let stats = mem.get_stats("a1");
        assert_eq!(stats.total, 3);

        // The 0.1 entry should be gone
        let entries = mem.memories.get("a1").unwrap();
        assert!(!entries.iter().any(|e| e.content == "e2"));
    }

    #[test]
    fn test_save_and_load() {
        let dir = format!("/tmp/nexus-memory-test-{}", Uuid::new_v4());
        let config = MemoryConfig {
            persistence_dir: dir.clone(),
            ..test_config()
        };

        let mut mem = AgentMemory::new(config.clone());
        mem.remember(
            "a1",
            "persistent fact",
            MemoryType::Fact,
            0.7,
            vec!["tag1".to_string()],
        );
        mem.save("a1").unwrap();

        // Load into a fresh instance
        let mut mem2 = AgentMemory::new(config);
        mem2.load("a1").unwrap();
        let stats = mem2.get_stats("a1");
        assert_eq!(stats.total, 1);

        let entries = mem2.memories.get("a1").unwrap();
        assert_eq!(entries[0].content, "persistent fact");
        assert_eq!(entries[0].tags, vec!["tag1".to_string()]);

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_clear_all() {
        let mut mem = test_memory();
        mem.remember("a1", "one", MemoryType::Fact, 0.5, vec![]);
        mem.remember("a1", "two", MemoryType::Fact, 0.5, vec![]);
        mem.clear("a1");
        assert_eq!(mem.get_stats("a1").total, 0);
    }

    #[test]
    fn test_stats() {
        let mut mem = test_memory();
        mem.remember("a1", "f1", MemoryType::Fact, 0.8, vec![]);
        mem.remember("a1", "p1", MemoryType::Preference, 0.6, vec![]);

        let stats = mem.get_stats("a1");
        assert_eq!(stats.total, 2);
        assert_eq!(stats.by_type.get("Fact"), Some(&1));
        assert_eq!(stats.by_type.get("Preference"), Some(&1));
        assert!((stats.avg_importance - 0.7).abs() < 0.01);
        assert!(stats.oldest.is_some());
        assert!(stats.newest.is_some());
    }

    #[test]
    fn test_multiple_agents_isolated() {
        let mut mem = test_memory();
        mem.remember("a1", "agent one data", MemoryType::Fact, 0.5, vec![]);
        mem.remember("a2", "agent two data", MemoryType::Fact, 0.5, vec![]);

        let r1 = mem.recall("a1", "data", 10);
        assert_eq!(r1.len(), 1);
        assert!(r1[0].content.contains("one"));

        let r2 = mem.recall("a2", "data", 10);
        assert_eq!(r2.len(), 1);
        assert!(r2[0].content.contains("two"));

        assert_eq!(mem.get_stats("a1").total, 1);
        assert_eq!(mem.get_stats("a2").total, 1);
    }
}
