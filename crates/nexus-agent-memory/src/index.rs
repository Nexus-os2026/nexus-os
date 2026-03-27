use std::collections::HashMap;

use crate::types::Memory;

/// Keyword index for fast memory retrieval.
pub struct MemoryIndex {
    keyword_map: HashMap<String, Vec<String>>,
    domain_map: HashMap<String, Vec<String>>,
    tag_map: HashMap<String, Vec<String>>,
}

impl MemoryIndex {
    pub fn new() -> Self {
        Self {
            keyword_map: HashMap::new(),
            domain_map: HashMap::new(),
            tag_map: HashMap::new(),
        }
    }

    pub fn index(&mut self, memory: &Memory) {
        let keywords = Self::extract_keywords(&memory.content.summary);
        for keyword in keywords {
            self.keyword_map
                .entry(keyword)
                .or_default()
                .push(memory.id.clone());
        }
        if let Some(ref domain) = memory.metadata.domain {
            self.domain_map
                .entry(domain.to_lowercase())
                .or_default()
                .push(memory.id.clone());
        }
        for tag in &memory.tags {
            self.tag_map
                .entry(tag.to_lowercase())
                .or_default()
                .push(memory.id.clone());
        }
    }

    pub fn remove(&mut self, memory_id: &str) {
        for ids in self.keyword_map.values_mut() {
            ids.retain(|id| id != memory_id);
        }
        for ids in self.domain_map.values_mut() {
            ids.retain(|id| id != memory_id);
        }
        for ids in self.tag_map.values_mut() {
            ids.retain(|id| id != memory_id);
        }
    }

    pub fn search_keywords(&self, query: &str) -> Vec<String> {
        let keywords = Self::extract_keywords(query);
        let mut results: HashMap<String, usize> = HashMap::new();

        for keyword in &keywords {
            if let Some(ids) = self.keyword_map.get(keyword) {
                for id in ids {
                    *results.entry(id.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut sorted: Vec<(String, usize)> = results.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.into_iter().map(|(id, _)| id).collect()
    }

    fn extract_keywords(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
            .filter(|w| w.len() >= 3)
            .map(|w| w.to_string())
            .collect()
    }

    pub fn keyword_count(&self) -> usize {
        self.keyword_map.len()
    }

    pub fn domain_count(&self) -> usize {
        self.domain_map.len()
    }

    pub fn tag_count(&self) -> usize {
        self.tag_map.len()
    }
}

impl Default for MemoryIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MemoryContent, MemoryMetadata, MemoryType, Valence};

    fn make_indexed_memory(id: &str, summary: &str, tags: &[&str], domain: Option<&str>) -> Memory {
        Memory {
            id: id.into(),
            agent_id: "a1".into(),
            memory_type: MemoryType::Semantic,
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
                domain: domain.map(|s| s.into()),
                valence: Valence::Neutral,
                confidence: 0.8,
            },
            importance: 0.5,
            access_count: 0,
            last_accessed: 0,
            created_at: 0,
            consolidated: false,
            tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_index_keyword_search() {
        let mut idx = MemoryIndex::new();
        let m1 = make_indexed_memory("m1", "deployed service to production", &[], None);
        let m2 = make_indexed_memory("m2", "database migration completed", &[], None);
        idx.index(&m1);
        idx.index(&m2);

        let results = idx.search_keywords("deployed production");
        assert!(results.contains(&"m1".to_string()));
        assert!(!results.contains(&"m2".to_string()));
    }

    #[test]
    fn test_index_remove() {
        let mut idx = MemoryIndex::new();
        let m1 = make_indexed_memory("m1", "deployed service", &["deploy"], None);
        idx.index(&m1);

        assert!(!idx.search_keywords("deployed").is_empty());
        idx.remove("m1");
        assert!(idx.search_keywords("deployed").is_empty());
    }
}
