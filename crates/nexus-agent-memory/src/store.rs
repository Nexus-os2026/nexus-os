use crate::types::{Memory, MemoryQuery};

/// The agent memory store — holds all memories for a single agent.
pub struct AgentMemoryStore {
    agent_id: String,
    memories: Vec<Memory>,
    max_memories: usize,
    consolidation_threshold: f64,
}

impl AgentMemoryStore {
    pub fn new(agent_id: String, max_memories: usize) -> Self {
        Self {
            agent_id,
            memories: Vec::new(),
            max_memories,
            consolidation_threshold: 0.8,
        }
    }

    /// Store a new memory. Returns the assigned ID.
    pub fn store(&mut self, mut memory: Memory) -> String {
        memory.id = uuid::Uuid::new_v4().to_string();
        memory.agent_id.clone_from(&self.agent_id);
        memory.created_at = epoch_now();
        memory.last_accessed = epoch_now();
        memory.access_count = 0;

        let id = memory.id.clone();
        self.memories.push(memory);

        if self.memories.len() > self.max_memories {
            self.trigger_consolidation();
        }

        id
    }

    /// Store a pre-built memory (loaded from disk). Does not overwrite ID/timestamps.
    pub fn store_existing(&mut self, memory: Memory) {
        self.memories.push(memory);
    }

    /// Retrieve memories matching a query.
    pub fn query(&mut self, query: &MemoryQuery) -> Vec<Memory> {
        let query_lower = query.query.to_lowercase();

        let mut scored: Vec<(usize, f64)> = self
            .memories
            .iter()
            .enumerate()
            .filter(|(_, m)| matches_query(m, query))
            .map(|(i, m)| (i, relevance_score(m, &query_lower)))
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(query.limit);

        // Update access counts
        let now = epoch_now();
        for &(idx, _) in &scored {
            self.memories[idx].access_count += 1;
            self.memories[idx].last_accessed = now;
        }

        scored
            .iter()
            .map(|&(idx, _)| self.memories[idx].clone())
            .collect()
    }

    pub fn get(&self, memory_id: &str) -> Option<&Memory> {
        self.memories.iter().find(|m| m.id == memory_id)
    }

    pub fn update_importance(&mut self, memory_id: &str, importance: f64) -> bool {
        if let Some(m) = self.memories.iter_mut().find(|m| m.id == memory_id) {
            m.importance = importance.clamp(0.0, 1.0);
            true
        } else {
            false
        }
    }

    pub fn delete(&mut self, memory_id: &str) -> bool {
        let before = self.memories.len();
        self.memories.retain(|m| m.id != memory_id);
        self.memories.len() < before
    }

    pub fn all(&self) -> &[Memory] {
        &self.memories
    }

    pub fn count_by_type(&self) -> std::collections::HashMap<String, usize> {
        let mut counts = std::collections::HashMap::new();
        for m in &self.memories {
            *counts.entry(format!("{:?}", m.memory_type)).or_insert(0) += 1;
        }
        counts
    }

    pub fn len(&self) -> usize {
        self.memories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.memories.is_empty()
    }

    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    fn trigger_consolidation(&mut self) {
        let target = (self.max_memories as f64 * self.consolidation_threshold) as usize;
        if self.memories.len() <= target {
            return;
        }

        let now = epoch_now();
        let mut scored: Vec<(usize, f64)> = self
            .memories
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let retention = m.importance * 0.4
                    + (m.access_count as f64).min(20.0) / 20.0 * 0.3
                    + {
                        let age = now.saturating_sub(m.created_at);
                        1.0 / (1.0 + age as f64 / 86400.0)
                    } * 0.2
                    + if m.consolidated { 0.0 } else { 0.1 };
                (i, retention)
            })
            .collect();

        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let to_remove = self.memories.len() - target;
        let mut remove_indices: Vec<usize> =
            scored.iter().take(to_remove).map(|(i, _)| *i).collect();
        remove_indices.sort_unstable();
        remove_indices.reverse();
        for idx in remove_indices {
            if idx < self.memories.len() {
                self.memories.remove(idx);
            }
        }

        tracing::info!(
            agent = %self.agent_id,
            after = self.memories.len(),
            removed = to_remove,
            "Memory consolidation complete"
        );
    }
}

fn matches_query(memory: &Memory, query: &MemoryQuery) -> bool {
    if let Some(ref mt) = query.memory_type {
        if memory.memory_type != *mt {
            return false;
        }
    }
    if let Some(ref tags) = query.tags {
        if !tags.iter().any(|t| memory.tags.contains(t)) {
            return false;
        }
    }
    if let Some(ref domain) = query.domain {
        if memory.metadata.domain.as_deref() != Some(domain.as_str()) {
            return false;
        }
    }
    if let Some(min) = query.min_importance {
        if memory.importance < min {
            return false;
        }
    }
    if let Some(after) = query.after {
        if memory.created_at < after {
            return false;
        }
    }
    if let Some(before) = query.before {
        if memory.created_at > before {
            return false;
        }
    }
    if !query.query.is_empty() {
        let words: Vec<String> = query
            .query
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() >= 2)
            .map(|w| w.to_string())
            .collect();
        let content = memory.content.summary.to_lowercase();
        let tags: Vec<String> = memory.tags.iter().map(|t| t.to_lowercase()).collect();
        let domain_lower = memory
            .metadata
            .domain
            .as_ref()
            .map(|d| d.to_lowercase())
            .unwrap_or_default();
        let any_match = words.iter().any(|w| {
            content.contains(w.as_str())
                || tags.iter().any(|t| t.contains(w.as_str()))
                || domain_lower.contains(w.as_str())
        });
        if !any_match {
            return false;
        }
    }
    true
}

fn relevance_score(memory: &Memory, query_lower: &str) -> f64 {
    let mut score = 0.0;
    let content_lower = memory.content.summary.to_lowercase();
    let words: Vec<&str> = query_lower
        .split_whitespace()
        .filter(|w| w.len() >= 2)
        .collect();
    if !words.is_empty() {
        let word_hits = words.iter().filter(|w| content_lower.contains(*w)).count();
        score += 0.4 * (word_hits as f64 / words.len() as f64);
    }
    if !words.is_empty()
        && memory
            .tags
            .iter()
            .any(|t| words.iter().any(|w| t.to_lowercase().contains(w)))
    {
        score += 0.2;
    }
    score += memory.importance * 0.2;
    let age_hours = epoch_now().saturating_sub(memory.last_accessed) / 3600;
    score += 1.0 / (1.0 + age_hours as f64 / 24.0) * 0.1;
    let freq = (memory.access_count as f64).min(10.0) / 10.0;
    score += freq * 0.1;
    score
}

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MemoryContent, MemoryMetadata, MemoryType, Valence};

    fn make_memory(summary: &str, mtype: MemoryType, importance: f64, tags: &[&str]) -> Memory {
        Memory {
            id: String::new(),
            agent_id: String::new(),
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
                domain: None,
                valence: Valence::Neutral,
                confidence: 0.8,
            },
            importance,
            access_count: 0,
            last_accessed: 0,
            created_at: 0,
            consolidated: false,
            tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn make_memory_with_domain(
        summary: &str,
        mtype: MemoryType,
        importance: f64,
        domain: &str,
    ) -> Memory {
        let mut m = make_memory(summary, mtype, importance, &[]);
        m.metadata.domain = Some(domain.into());
        m
    }

    #[test]
    fn test_store_and_retrieve() {
        let mut store = AgentMemoryStore::new("agent-1".into(), 100);
        let m = make_memory(
            "deployed service X and it failed",
            MemoryType::Episodic,
            0.8,
            &["deploy", "failure"],
        );
        store.store(m);

        let results = store.query(&MemoryQuery {
            query: "deploy".into(),
            limit: 5,
            ..Default::default()
        });
        assert_eq!(results.len(), 1);
        assert!(results[0].content.summary.contains("deployed"));
    }

    #[test]
    fn test_store_multiple_types() {
        let mut store = AgentMemoryStore::new("agent-1".into(), 100);
        store.store(make_memory(
            "event happened",
            MemoryType::Episodic,
            0.5,
            &[],
        ));
        store.store(make_memory(
            "postgres needs 2GB",
            MemoryType::Semantic,
            0.7,
            &[],
        ));
        store.store(make_memory(
            "step1 build step2 test",
            MemoryType::Procedural,
            0.6,
            &[],
        ));
        store.store(make_memory(
            "agent-devops is reliable",
            MemoryType::Relational,
            0.9,
            &[],
        ));

        let counts = store.count_by_type();
        assert_eq!(counts.len(), 4);
        assert_eq!(store.len(), 4);
    }

    #[test]
    fn test_query_by_type_filter() {
        let mut store = AgentMemoryStore::new("agent-1".into(), 100);
        store.store(make_memory("event A", MemoryType::Episodic, 0.5, &[]));
        store.store(make_memory("fact B", MemoryType::Semantic, 0.5, &[]));
        store.store(make_memory("event C", MemoryType::Episodic, 0.5, &[]));

        let results = store.query(&MemoryQuery {
            memory_type: Some(MemoryType::Episodic),
            limit: 10,
            ..Default::default()
        });
        assert_eq!(results.len(), 2);
        assert!(results
            .iter()
            .all(|m| m.memory_type == MemoryType::Episodic));
    }

    #[test]
    fn test_query_by_tag_filter() {
        let mut store = AgentMemoryStore::new("agent-1".into(), 100);
        store.store(make_memory(
            "tagged A",
            MemoryType::Semantic,
            0.5,
            &["deploy"],
        ));
        store.store(make_memory(
            "tagged B",
            MemoryType::Semantic,
            0.5,
            &["database"],
        ));

        let results = store.query(&MemoryQuery {
            tags: Some(vec!["deploy".into()]),
            limit: 10,
            ..Default::default()
        });
        assert_eq!(results.len(), 1);
        assert!(results[0].tags.contains(&"deploy".to_string()));
    }

    #[test]
    fn test_query_by_domain_filter() {
        let mut store = AgentMemoryStore::new("agent-1".into(), 100);
        store.store(make_memory_with_domain(
            "in devops",
            MemoryType::Semantic,
            0.5,
            "devops",
        ));
        store.store(make_memory_with_domain(
            "in finance",
            MemoryType::Semantic,
            0.5,
            "finance",
        ));

        let results = store.query(&MemoryQuery {
            domain: Some("devops".into()),
            limit: 10,
            ..Default::default()
        });
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_query_by_importance() {
        let mut store = AgentMemoryStore::new("agent-1".into(), 100);
        store.store(make_memory("low", MemoryType::Semantic, 0.2, &[]));
        store.store(make_memory("high", MemoryType::Semantic, 0.9, &[]));

        let results = store.query(&MemoryQuery {
            min_importance: Some(0.5),
            limit: 10,
            ..Default::default()
        });
        assert_eq!(results.len(), 1);
        assert!(results[0].content.summary.contains("high"));
    }

    #[test]
    fn test_query_relevance_ranking() {
        let mut store = AgentMemoryStore::new("agent-1".into(), 100);
        store.store(make_memory(
            "unrelated stuff",
            MemoryType::Semantic,
            0.9,
            &[],
        ));
        store.store(make_memory(
            "deploy service X failed",
            MemoryType::Episodic,
            0.5,
            &["deploy"],
        ));

        let results = store.query(&MemoryQuery {
            query: "deploy".into(),
            limit: 10,
            ..Default::default()
        });
        // The deploy-related memory should rank first despite lower importance
        assert!(!results.is_empty());
        assert!(results[0].content.summary.contains("deploy"));
    }

    #[test]
    fn test_memory_access_count() {
        let mut store = AgentMemoryStore::new("agent-1".into(), 100);
        let id = store.store(make_memory("remember this", MemoryType::Semantic, 0.5, &[]));

        let _ = store.query(&MemoryQuery {
            query: "remember".into(),
            limit: 5,
            ..Default::default()
        });

        let m = store.get(&id).unwrap();
        assert_eq!(m.access_count, 1);
    }

    #[test]
    fn test_consolidation_removes_low_value() {
        let mut store = AgentMemoryStore::new("agent-1".into(), 5);
        // Fill with low-importance memories
        for i in 0..6 {
            store.store(make_memory(
                &format!("low {i}"),
                MemoryType::Semantic,
                0.1,
                &[],
            ));
        }
        // Consolidation should have triggered, removing some
        assert!(store.len() <= 5);
    }

    #[test]
    fn test_consolidation_preserves_important() {
        let mut store = AgentMemoryStore::new("agent-1".into(), 5);
        // Store one high-importance memory
        let id = store.store(make_memory(
            "critical knowledge",
            MemoryType::Semantic,
            1.0,
            &[],
        ));
        // Fill rest with low-importance
        for i in 0..5 {
            store.store(make_memory(
                &format!("filler {i}"),
                MemoryType::Semantic,
                0.05,
                &[],
            ));
        }
        // The important memory should survive
        assert!(store.get(&id).is_some());
    }
}
