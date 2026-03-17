//! Core types for the Dream Forge system.

use serde::{Deserialize, Serialize};

/// Categories of dream work an agent can perform overnight.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DreamType {
    /// Replay a past task and find improvements.
    Replay,
    /// Try N variations of an approach, pick best.
    Experiment,
    /// Compress learnings into updated system prompt.
    Consolidate,
    /// Follow curiosity — research a topic proactively.
    Explore,
    /// Predict likely next requests and pre-solve them.
    Precompute,
    /// Genesis — create new agent for detected gap.
    Create,
    /// Find performance improvements in past work.
    Optimize,
}

/// A single dream task waiting in the priority queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamTask {
    pub id: String,
    pub task_type: DreamType,
    /// Priority 0.0–1.0, higher = more important.
    pub priority: f64,
    /// Which agent generated this dream task.
    pub source_agent: String,
    /// Task-specific data (original prompt, errors, context).
    pub context: serde_json::Value,
    pub created_at: u64,
    pub estimated_tokens: u64,
}

impl Default for DreamTask {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            task_type: DreamType::Replay,
            priority: 0.5,
            source_agent: String::new(),
            context: serde_json::Value::Null,
            created_at: crate::consciousness::state::now_secs(),
            estimated_tokens: 500,
        }
    }
}

/// The result of a completed dream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamResult {
    pub task_id: String,
    pub dream_type: DreamType,
    pub agent_id: String,
    pub started_at: u64,
    pub completed_at: u64,
    pub tokens_used: u64,
    pub outcome: DreamOutcome,
}

/// What a dream produced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DreamOutcome {
    Improvement {
        description: String,
        before_score: f64,
        after_score: f64,
        /// File path if something was created.
        artifact: Option<String>,
    },
    Discovery {
        description: String,
        relevance: f64,
        /// Agent IDs who received this knowledge.
        shared_with: Vec<String>,
    },
    Creation {
        new_agent_id: String,
        reason: String,
        test_score: f64,
    },
    Precomputed {
        predicted_request: String,
        prepared_response: String,
        confidence: f64,
    },
    NoResult {
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dream_task_default_has_uuid() {
        let t = DreamTask::default();
        assert!(!t.id.is_empty());
        assert_eq!(t.task_type, DreamType::Replay);
    }

    #[test]
    fn dream_type_serialization_roundtrip() {
        let types = vec![
            DreamType::Replay,
            DreamType::Experiment,
            DreamType::Consolidate,
            DreamType::Explore,
            DreamType::Precompute,
            DreamType::Create,
            DreamType::Optimize,
        ];
        for dt in types {
            let json = serde_json::to_string(&dt).unwrap();
            let back: DreamType = serde_json::from_str(&json).unwrap();
            assert_eq!(dt, back);
        }
    }

    #[test]
    fn dream_outcome_serialize() {
        let outcome = DreamOutcome::Improvement {
            description: "Better code review".into(),
            before_score: 0.6,
            after_score: 0.9,
            artifact: Some("/tmp/improved.rs".into()),
        };
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("Improvement"));
    }

    #[test]
    fn dream_result_serialize() {
        let result = DreamResult {
            task_id: "t1".into(),
            dream_type: DreamType::Experiment,
            agent_id: "a1".into(),
            started_at: 1000,
            completed_at: 2000,
            tokens_used: 500,
            outcome: DreamOutcome::NoResult {
                reason: "test".into(),
            },
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: DreamResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.task_id, "t1");
    }
}
