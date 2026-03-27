use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::types::Memory;

/// Memory consolidation strategies.
pub struct MemoryConsolidator;

impl MemoryConsolidator {
    /// Merge similar memories into a single stronger memory.
    pub fn merge_similar(memories: &[Memory], similarity_threshold: f64) -> Vec<MergeCandidate> {
        let mut candidates = Vec::new();

        for i in 0..memories.len() {
            for j in (i + 1)..memories.len() {
                let similarity = Self::compute_similarity(&memories[i], &memories[j]);
                if similarity >= similarity_threshold {
                    candidates.push(MergeCandidate {
                        memory_a: memories[i].id.clone(),
                        memory_b: memories[j].id.clone(),
                        similarity,
                        merged_summary: Self::merge_summaries(
                            &memories[i].content.summary,
                            &memories[j].content.summary,
                        ),
                        merged_importance: (memories[i].importance + memories[j].importance) / 2.0
                            * 1.1,
                    });
                }
            }
        }

        candidates.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates
    }

    /// Identify memories that should be forgotten.
    pub fn identify_forgettable(memories: &[Memory], age_threshold_days: u64) -> Vec<String> {
        let now = epoch_now();
        let threshold_secs = age_threshold_days * 86400;

        memories
            .iter()
            .filter(|m| {
                let age = now.saturating_sub(m.created_at);
                let stale = now.saturating_sub(m.last_accessed);
                age > threshold_secs
                    && m.importance < 0.3
                    && m.access_count < 2
                    && stale > threshold_secs / 2
                    && !m.consolidated
            })
            .map(|m| m.id.clone())
            .collect()
    }

    fn compute_similarity(a: &Memory, b: &Memory) -> f64 {
        let words_a: HashSet<String> = a
            .content
            .summary
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() >= 3)
            .map(|w| w.to_string())
            .collect();

        let words_b: HashSet<String> = b
            .content
            .summary
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() >= 3)
            .map(|w| w.to_string())
            .collect();

        if words_a.is_empty() || words_b.is_empty() {
            return 0.0;
        }

        let intersection = words_a.intersection(&words_b).count();
        let union = words_a.union(&words_b).count();

        intersection as f64 / union as f64
    }

    fn merge_summaries(a: &str, b: &str) -> String {
        if a.len() > b.len() {
            format!("{} (also: {})", a, &b[..b.len().min(100)])
        } else {
            format!("{} (also: {})", b, &a[..a.len().min(100)])
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeCandidate {
    pub memory_a: String,
    pub memory_b: String,
    pub similarity: f64,
    pub merged_summary: String,
    pub merged_importance: f64,
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

    fn mem(id: &str, summary: &str, importance: f64, access_count: u64) -> Memory {
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
                domain: None,
                valence: Valence::Neutral,
                confidence: 0.8,
            },
            importance,
            access_count,
            last_accessed: epoch_now(),
            created_at: epoch_now(),
            consolidated: false,
            tags: Vec::new(),
        }
    }

    #[test]
    fn test_merge_similar() {
        let memories = vec![
            mem(
                "m1",
                "deployed service X to production successfully",
                0.7,
                3,
            ),
            mem(
                "m2",
                "deployed service X to production with rollback",
                0.6,
                2,
            ),
            mem("m3", "database migration completed for service Y", 0.5, 1),
        ];

        let candidates = MemoryConsolidator::merge_similar(&memories, 0.3);
        // m1 and m2 should be similar (share many words)
        assert!(!candidates.is_empty());
        assert!(candidates[0].similarity >= 0.3);
    }

    #[test]
    fn test_forget_old_unused() {
        let now = epoch_now();
        let old_time = now - 100 * 86400; // 100 days ago

        let memories = vec![
            Memory {
                created_at: old_time,
                last_accessed: old_time,
                ..mem("old-unused", "something old", 0.1, 0)
            },
            mem("recent-used", "something recent", 0.9, 10),
        ];

        let forgettable = MemoryConsolidator::identify_forgettable(&memories, 30);
        assert_eq!(forgettable.len(), 1);
        assert_eq!(forgettable[0], "old-unused");
    }
}
