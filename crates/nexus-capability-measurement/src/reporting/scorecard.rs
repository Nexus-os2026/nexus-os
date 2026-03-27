//! Per-agent scorecard generation.

use serde::{Deserialize, Serialize};

use crate::framework::*;
use crate::scoring::gaming_detection::GamingFlag;

/// Complete agent scorecard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentScorecard {
    pub agent_id: String,
    pub agent_autonomy_level: u8,
    pub measured_at: u64,
    pub vectors: Vec<VectorScorecard>,
    pub overall: OverallScore,
    pub classification: AgentClassification,
    pub gaming_flags: Vec<GamingFlag>,
    pub audit_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorScorecard {
    pub vector: Vector,
    pub score: f64,
    pub levels: Vec<LevelScorecard>,
    pub articulation_total: f64,
    pub flags: Vec<GamingFlag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelScorecard {
    pub level: DifficultyLevel,
    pub primary_score: f64,
    pub articulation_score: f64,
    pub flag_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallScore {
    /// Weighted average across all four vectors (equal weight per vector).
    pub composite: f64,
    /// Minimum score across all vectors — the capability floor.
    pub floor: f64,
    /// Maximum score across all vectors — the capability ceiling.
    pub ceiling: f64,
    /// Difficulty level where scores consistently drop below 0.5.
    pub effective_ceiling: Option<DifficultyLevel>,
}

impl AgentScorecard {
    pub fn from_session(session: &MeasurementSession) -> Self {
        let vectors: Vec<VectorScorecard> = session
            .vector_results
            .iter()
            .map(|vr| {
                let articulation_total = if vr.level_results.is_empty() {
                    0.0
                } else {
                    vr.level_results
                        .iter()
                        .map(|lr| lr.articulation_score.total)
                        .sum::<f64>()
                        / vr.level_results.len() as f64
                };

                VectorScorecard {
                    vector: vr.vector,
                    score: vr.vector_score,
                    levels: vr
                        .level_results
                        .iter()
                        .map(|lr| LevelScorecard {
                            level: lr.level,
                            primary_score: lr.primary_score.adjusted_score,
                            articulation_score: lr.articulation_score.total,
                            flag_count: lr.gaming_flags.len(),
                        })
                        .collect(),
                    articulation_total,
                    flags: vr.gaming_flags.clone(),
                }
            })
            .collect();

        let scores: Vec<f64> = vectors.iter().map(|v| v.score).collect();
        let composite = if scores.is_empty() {
            0.0
        } else {
            scores.iter().sum::<f64>() / scores.len() as f64
        };
        let floor = scores.iter().cloned().fold(f64::MAX, f64::min);
        let ceiling = scores.iter().cloned().fold(f64::MIN, f64::max);

        let all_flags: Vec<GamingFlag> = session
            .vector_results
            .iter()
            .flat_map(|vr| vr.gaming_flags.clone())
            .collect();

        AgentScorecard {
            agent_id: session.agent_id.clone(),
            agent_autonomy_level: session.agent_autonomy_level,
            measured_at: session.started_at,
            vectors,
            overall: OverallScore {
                composite,
                floor,
                ceiling,
                effective_ceiling: None,
            },
            classification: session
                .cross_vector_analysis
                .as_ref()
                .map(|a| a.overall_classification.clone())
                .unwrap_or(AgentClassification::Anomalous {
                    reason: "No cross-vector analysis".into(),
                }),
            gaming_flags: all_flags,
            audit_hash: session.audit_hash.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring::articulation::empty_articulation;
    use crate::scoring::asymmetric::PrimaryScore;

    #[test]
    fn test_scorecard_from_session() {
        let session = MeasurementSession {
            id: uuid::Uuid::new_v4(),
            agent_id: "test-agent".into(),
            agent_autonomy_level: 3,
            started_at: 1000,
            completed_at: Some(1001),
            vector_results: vec![
                VectorResult {
                    vector: Vector::ReasoningDepth,
                    level_results: vec![LevelResult {
                        level: DifficultyLevel::Level1,
                        problem_id: "p1".into(),
                        problem_version: "v1".into(),
                        agent_response: "test".into(),
                        primary_score: PrimaryScore {
                            raw_score: 0.8,
                            penalties: vec![],
                            adjusted_score: 0.8,
                        },
                        articulation_score: empty_articulation(Vector::ReasoningDepth),
                        gaming_flags: vec![],
                        scorer_agreement: ScorerAgreement::Pending,
                    }],
                    gaming_flags: vec![],
                    vector_score: 0.8,
                },
                VectorResult {
                    vector: Vector::PlanningCoherence,
                    level_results: vec![],
                    gaming_flags: vec![],
                    vector_score: 0.7,
                },
            ],
            cross_vector_analysis: Some(CrossVectorAnalysis {
                capability_profile: CapabilityProfile {
                    reasoning_depth: 0.8,
                    planning_coherence: 0.7,
                    adaptation: 0.0,
                    tool_use: 0.0,
                },
                anomalies: vec![],
                overall_classification: AgentClassification::Balanced {
                    min_score: 0.7,
                    max_score: 0.8,
                },
            }),
            audit_hash: "abc123".into(),
        };

        let card = AgentScorecard::from_session(&session);
        assert_eq!(card.agent_id, "test-agent");
        assert_eq!(card.agent_autonomy_level, 3);
        assert_eq!(card.vectors.len(), 2);
        assert!((card.overall.composite - 0.75).abs() < 1e-9);
        assert!((card.overall.floor - 0.7).abs() < 1e-9);
        assert!((card.overall.ceiling - 0.8).abs() < 1e-9);
    }
}
