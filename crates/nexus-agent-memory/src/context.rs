use serde::{Deserialize, Serialize};

use crate::store::AgentMemoryStore;
use crate::types::{MemoryQuery, MemoryType};

/// Context builder — selects the most relevant memories for a given task.
pub struct ContextBuilder;

impl ContextBuilder {
    /// Build a context string from relevant memories for a task.
    pub fn build_context(
        store: &mut AgentMemoryStore,
        task_description: &str,
        max_memories: usize,
        max_chars: usize,
    ) -> MemoryContext {
        let query = MemoryQuery {
            query: task_description.into(),
            limit: max_memories * 2,
            ..Default::default()
        };

        let memories = store.query(&query);

        let mut entries = Vec::new();
        let mut total_chars = 0;

        for memory in memories.iter().take(max_memories) {
            let part = format!(
                "[{:?}] {}: {}",
                memory.memory_type,
                memory.tags.join(", "),
                memory.content.summary,
            );

            if total_chars + part.len() > max_chars {
                break;
            }

            total_chars += part.len();
            entries.push(ContextEntry {
                memory_id: memory.id.clone(),
                memory_type: memory.memory_type.clone(),
                summary: memory.content.summary.clone(),
                importance: memory.importance,
            });
        }

        let context_text = if entries.is_empty() {
            String::new()
        } else {
            let parts: Vec<String> = entries
                .iter()
                .map(|e| format!("- [{:?}] {}", e.memory_type, e.summary))
                .collect();
            format!(
                "RELEVANT MEMORIES FROM PAST EXPERIENCE:\n{}\n",
                parts.join("\n")
            )
        };

        MemoryContext {
            entries,
            context_text,
            total_chars,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryContext {
    pub entries: Vec<ContextEntry>,
    pub context_text: String,
    pub total_chars: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEntry {
    pub memory_id: String,
    pub memory_type: MemoryType,
    pub summary: String,
    pub importance: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Memory, MemoryContent, MemoryMetadata, MemoryType, Valence};

    fn make_memory(summary: &str, tags: &[&str]) -> Memory {
        Memory {
            id: String::new(),
            agent_id: String::new(),
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
                domain: None,
                valence: Valence::Neutral,
                confidence: 0.8,
            },
            importance: 0.7,
            access_count: 0,
            last_accessed: 0,
            created_at: 0,
            consolidated: false,
            tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_context_builder() {
        let mut store = AgentMemoryStore::new("agent-1".into(), 100);
        store.store(make_memory(
            "service X requires PostgreSQL 14+",
            &["deploy"],
        ));
        store.store(make_memory("deploy step 1: check migrations", &["deploy"]));

        let ctx = ContextBuilder::build_context(&mut store, "deploy service X", 5, 5000);
        assert!(!ctx.entries.is_empty());
        assert!(ctx.context_text.contains("RELEVANT MEMORIES"));
    }

    #[test]
    fn test_context_respects_char_limit() {
        let mut store = AgentMemoryStore::new("agent-1".into(), 100);
        for i in 0..20 {
            store.store(make_memory(
                &format!("memory number {i} about deploying services to production environments"),
                &["deploy"],
            ));
        }

        let ctx = ContextBuilder::build_context(&mut store, "deploy", 20, 200);
        assert!(ctx.total_chars <= 200);
        assert!(ctx.entries.len() < 20);
    }
}
