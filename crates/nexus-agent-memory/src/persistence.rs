use crate::store::AgentMemoryStore;
use crate::types::Memory;

/// Persist agent memories to disk as JSON files.
/// One file per agent: `{data_dir}/agent_memory/{agent_id}.json`
pub struct MemoryPersistence;

impl MemoryPersistence {
    pub fn save(store: &AgentMemoryStore, data_dir: &str) -> Result<(), String> {
        let dir = format!("{data_dir}/agent_memory");
        std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir failed: {e}"))?;

        let path = format!("{dir}/{}.json", store.agent_id());
        let json = serde_json::to_string_pretty(store.all())
            .map_err(|e| format!("Serialize failed: {e}"))?;

        std::fs::write(&path, json).map_err(|e| format!("Write failed: {e}"))?;

        tracing::debug!(
            agent = %store.agent_id(),
            memories = store.len(),
            path = %path,
            "Memories saved"
        );
        Ok(())
    }

    pub fn load(agent_id: &str, data_dir: &str, max_memories: usize) -> AgentMemoryStore {
        let path = format!("{data_dir}/agent_memory/{agent_id}.json");
        let mut store = AgentMemoryStore::new(agent_id.into(), max_memories);

        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(memories) = serde_json::from_str::<Vec<Memory>>(&content) {
                for memory in memories {
                    store.store_existing(memory);
                }
                tracing::debug!(agent = %agent_id, memories = store.len(), "Memories loaded from disk");
            }
        }

        store
    }

    pub fn list_agents(data_dir: &str) -> Vec<String> {
        let dir = format!("{data_dir}/agent_memory");
        std::fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        name.strip_suffix(".json").map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn delete(agent_id: &str, data_dir: &str) -> Result<(), String> {
        let path = format!("{data_dir}/agent_memory/{agent_id}.json");
        if std::path::Path::new(&path).exists() {
            std::fs::remove_file(&path).map_err(|e| format!("Delete failed: {e}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Memory, MemoryContent, MemoryMetadata, MemoryType, Valence};

    fn make_memory(summary: &str) -> Memory {
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
            tags: Vec::new(),
        }
    }

    #[test]
    fn test_persistence_save_load() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().to_str().unwrap();

        let mut store = AgentMemoryStore::new("agent-test".into(), 100);
        store.store(make_memory("fact one"));
        store.store(make_memory("fact two"));

        MemoryPersistence::save(&store, data_dir).unwrap();

        let loaded = MemoryPersistence::load("agent-test", data_dir, 100);
        assert_eq!(loaded.len(), 2);

        let agents = MemoryPersistence::list_agents(data_dir);
        assert!(agents.contains(&"agent-test".to_string()));
    }
}
