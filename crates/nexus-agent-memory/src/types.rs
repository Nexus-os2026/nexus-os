use serde::{Deserialize, Serialize};

/// A single memory entry — the atomic unit of agent memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub agent_id: String,
    pub memory_type: MemoryType,
    pub content: MemoryContent,
    pub metadata: MemoryMetadata,
    /// Importance score (0.0-1.0) — higher = more likely to be retained.
    pub importance: f64,
    /// How often this memory has been retrieved.
    pub access_count: u64,
    /// Last accessed timestamp (epoch seconds).
    pub last_accessed: u64,
    /// Created timestamp (epoch seconds).
    pub created_at: u64,
    /// Whether this memory has been consolidated (merged/compressed).
    pub consolidated: bool,
    /// Tags for retrieval.
    pub tags: Vec<String>,
}

/// The four types of agent memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryType {
    /// Specific events and experiences.
    Episodic,
    /// General knowledge and facts.
    Semantic,
    /// How to do things (learned procedures).
    Procedural,
    /// Knowledge about other agents and entities.
    Relational,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Episodic => write!(f, "Episodic"),
            Self::Semantic => write!(f, "Semantic"),
            Self::Procedural => write!(f, "Procedural"),
            Self::Relational => write!(f, "Relational"),
        }
    }
}

/// The actual content of the memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryContent {
    /// Natural language summary.
    pub summary: String,
    /// Structured data (if applicable).
    pub data: Option<serde_json::Value>,
    /// The raw experience that created this memory (if retained).
    pub raw_input: Option<String>,
    /// The outcome/result associated with this memory.
    pub outcome: Option<String>,
}

/// Metadata about the memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetadata {
    /// What task/context created this memory.
    pub source_task: Option<String>,
    /// Quality score of the task that created this memory.
    pub task_quality: Option<f64>,
    /// Related memory IDs.
    pub related_memories: Vec<String>,
    /// The domain/topic this memory belongs to.
    pub domain: Option<String>,
    /// Emotional valence.
    pub valence: Valence,
    /// How confident the agent is in this memory.
    pub confidence: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Valence {
    Positive,
    Neutral,
    Negative,
}

/// A memory query — what the agent is looking for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryQuery {
    pub query: String,
    pub memory_type: Option<MemoryType>,
    pub tags: Option<Vec<String>>,
    pub domain: Option<String>,
    pub min_importance: Option<f64>,
    pub after: Option<u64>,
    pub before: Option<u64>,
    pub limit: usize,
}

impl Default for MemoryQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            memory_type: None,
            tags: None,
            domain: None,
            min_importance: None,
            after: None,
            before: None,
            limit: 10,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_type_display() {
        assert_eq!(MemoryType::Episodic.to_string(), "Episodic");
        assert_eq!(MemoryType::Semantic.to_string(), "Semantic");
        assert_eq!(MemoryType::Procedural.to_string(), "Procedural");
        assert_eq!(MemoryType::Relational.to_string(), "Relational");
    }

    #[test]
    fn test_default_query() {
        let q = MemoryQuery::default();
        assert_eq!(q.limit, 10);
        assert!(q.query.is_empty());
        assert!(q.memory_type.is_none());
    }
}
