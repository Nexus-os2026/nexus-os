//! Bridge between capability measurement results and Darwin Core evolution.
//!
//! Converts measurement scorecards into fitness signals, re-evaluation triggers,
//! and mutation guidance that the Darwin Core engines can consume.
//!
//! The Darwin Core (AdversarialArena, SwarmCoordinator, PlanEvolutionEngine)
//! lives in `nexus-kernel::cognitive::algorithms`. This bridge produces
//! intermediate types that the kernel can translate into its own operations.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::framework::{AgentClassification, DifficultyLevel, Vector};
use crate::reporting::scorecard::AgentScorecard;
use crate::scoring::gaming_detection::{GamingFlag, GamingFlagSeverity, GamingFlagType};

// ── Bridge Trait ─────────────────────────────────────────────────────────────

/// Bridge between capability measurement and Darwin Core evolution.
pub trait EvolutionFitnessProvider {
    /// Convert a measurement scorecard into a fitness signal
    /// that the AdversarialArena can use for selection pressure.
    fn to_fitness_signal(&self) -> FitnessSignal;

    /// Convert gaming flags into targeted re-evaluation triggers
    /// for the SwarmCoordinator.
    fn to_reevaluation_triggers(&self) -> Vec<ReevaluationTrigger>;

    /// Convert the agent classification into mutation guidance
    /// for the PlanEvolutionEngine.
    fn to_mutation_guidance(&self) -> MutationGuidance;
}

// ── Bridge Types ─────────────────────────────────────────────────────────────

/// Fitness signal for the AdversarialArena.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitnessSignal {
    pub agent_id: String,
    /// Overall fitness 0.0–1.0 (from scorecard composite, after gaming discount).
    pub fitness: f64,
    /// Per-vector fitness breakdown.
    pub vector_fitness: HashMap<Vector, f64>,
    /// No vector scores below this — the capability floor.
    pub capability_floor: f64,
    /// Multiplicative discount from gaming flags (1.0 = no discount).
    pub gaming_discount: f64,
    /// Difficulty level where agent's scores collapse.
    pub effective_ceiling: Option<DifficultyLevel>,
}

/// Trigger for SwarmCoordinator to re-evaluate specific capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReevaluationTrigger {
    pub agent_id: String,
    /// Which vector to re-evaluate.
    pub vector: Option<Vector>,
    /// Which difficulty level showed the anomaly.
    pub level: Option<DifficultyLevel>,
    /// Why re-evaluation is needed.
    pub reason: ReevaluationReason,
    /// Priority — Red flags get immediate re-evaluation.
    pub priority: TriggerPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReevaluationReason {
    InvertedDifficulty,
    ArticulationGap,
    ConfidentUnderspecification,
    OutputFidelityFailure,
    GamingFlagTriggered(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerPriority {
    Normal,
    High,
    Critical,
}

/// Guidance for PlanEvolutionEngine on how to mutate this agent's genome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationGuidance {
    pub agent_id: String,
    pub classification: AgentClassification,
    /// Specific capability gaps to target with mutations.
    pub mutation_targets: Vec<MutationTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationTarget {
    pub vector: Vector,
    pub current_score: f64,
    pub target_score: f64,
    pub suggested_mutation: SuggestedMutation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestedMutation {
    AddPlanningScaffolding,
    DeepenReasoning,
    AddAdaptationExposure,
    StrengthenOutputGrounding,
    ExpandProblemDiversity,
    AddAdversarialTraining,
}

// ── Feedback Result ──────────────────────────────────────────────────────────

/// Summary of what was submitted to the Darwin Core.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackResult {
    pub agent_id: String,
    pub fitness: f64,
    pub gaming_discount: f64,
    pub trigger_count: usize,
    pub critical_triggers: usize,
    pub mutation_target_count: usize,
    pub classification: AgentClassification,
}

// ── Implementation ───────────────────────────────────────────────────────────

impl EvolutionFitnessProvider for AgentScorecard {
    fn to_fitness_signal(&self) -> FitnessSignal {
        let mut vector_fitness = HashMap::new();
        for vs in &self.vectors {
            vector_fitness.insert(vs.vector, vs.score);
        }

        // Gaming discount: multiply per-flag severity penalties
        let gaming_discount = compute_gaming_discount(&self.gaming_flags);

        let fitness = (self.overall.composite * gaming_discount).clamp(0.0, 1.0);

        FitnessSignal {
            agent_id: self.agent_id.clone(),
            fitness,
            vector_fitness,
            capability_floor: self.overall.floor,
            gaming_discount,
            effective_ceiling: self.overall.effective_ceiling,
        }
    }

    fn to_reevaluation_triggers(&self) -> Vec<ReevaluationTrigger> {
        let mut triggers = Vec::new();

        for flag in &self.gaming_flags {
            let priority = match flag.severity {
                GamingFlagSeverity::Yellow => TriggerPriority::Normal,
                GamingFlagSeverity::Orange => TriggerPriority::High,
                GamingFlagSeverity::Red => TriggerPriority::Critical,
            };

            let reason = match &flag.flag_type {
                GamingFlagType::InvertedDifficultySpectrum => {
                    ReevaluationReason::InvertedDifficulty
                }
                GamingFlagType::HighPrimaryZeroArticulation => ReevaluationReason::ArticulationGap,
                GamingFlagType::ConfidentAtLevel5 => {
                    ReevaluationReason::ConfidentUnderspecification
                }
                GamingFlagType::OutputDoesntMatchToolReturn => {
                    ReevaluationReason::OutputFidelityFailure
                }
                other => ReevaluationReason::GamingFlagTriggered(format!("{other:?}")),
            };

            triggers.push(ReevaluationTrigger {
                agent_id: self.agent_id.clone(),
                vector: None,
                level: None,
                reason,
                priority,
            });
        }

        triggers
    }

    fn to_mutation_guidance(&self) -> MutationGuidance {
        let mut targets = Vec::new();

        match &self.classification {
            AgentClassification::TheoreticalReasoner => {
                if let Some(vs) = self
                    .vectors
                    .iter()
                    .find(|v| v.vector == Vector::PlanningCoherence)
                {
                    targets.push(MutationTarget {
                        vector: Vector::PlanningCoherence,
                        current_score: vs.score,
                        target_score: (vs.score + 0.15).min(1.0),
                        suggested_mutation: SuggestedMutation::AddPlanningScaffolding,
                    });
                }
            }
            AgentClassification::ProceduralExecutor => {
                if let Some(vs) = self
                    .vectors
                    .iter()
                    .find(|v| v.vector == Vector::ReasoningDepth)
                {
                    targets.push(MutationTarget {
                        vector: Vector::ReasoningDepth,
                        current_score: vs.score,
                        target_score: (vs.score + 0.15).min(1.0),
                        suggested_mutation: SuggestedMutation::DeepenReasoning,
                    });
                }
            }
            AgentClassification::RigidToolUser => {
                if let Some(vs) = self
                    .vectors
                    .iter()
                    .find(|v| v.vector == Vector::AdaptationUnderUncertainty)
                {
                    targets.push(MutationTarget {
                        vector: Vector::AdaptationUnderUncertainty,
                        current_score: vs.score,
                        target_score: (vs.score + 0.15).min(1.0),
                        suggested_mutation: SuggestedMutation::AddAdaptationExposure,
                    });
                }
            }
            AgentClassification::PatternMatchingCeiling { .. } => {
                // Target the weakest vector with diversity training
                if let Some(weakest) = self.vectors.iter().min_by(|a, b| {
                    a.score
                        .partial_cmp(&b.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }) {
                    targets.push(MutationTarget {
                        vector: weakest.vector,
                        current_score: weakest.score,
                        target_score: (weakest.score + 0.15).min(1.0),
                        suggested_mutation: SuggestedMutation::ExpandProblemDiversity,
                    });
                }
            }
            AgentClassification::Balanced { .. } => {
                // Target the lowest-scoring vector
                if let Some(weakest) = self.vectors.iter().min_by(|a, b| {
                    a.score
                        .partial_cmp(&b.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }) {
                    let mutation = match weakest.vector {
                        Vector::ReasoningDepth => SuggestedMutation::DeepenReasoning,
                        Vector::PlanningCoherence => SuggestedMutation::AddPlanningScaffolding,
                        Vector::AdaptationUnderUncertainty => {
                            SuggestedMutation::AddAdaptationExposure
                        }
                        Vector::ToolUseIntegrity => SuggestedMutation::StrengthenOutputGrounding,
                    };
                    targets.push(MutationTarget {
                        vector: weakest.vector,
                        current_score: weakest.score,
                        target_score: (weakest.score + 0.15).min(1.0),
                        suggested_mutation: mutation,
                    });
                }
            }
            AgentClassification::Anomalous { .. } => {
                // No automated mutation — requires human review
            }
        }

        MutationGuidance {
            agent_id: self.agent_id.clone(),
            classification: self.classification.clone(),
            mutation_targets: targets,
        }
    }
}

// ── Feedback Loop ────────────────────────────────────────────────────────────

/// Run measurement → scoring → evolution feedback for an agent.
/// Returns a summary of what was produced.
pub fn run_measurement_feedback(scorecard: &AgentScorecard) -> FeedbackResult {
    let fitness_signal = scorecard.to_fitness_signal();
    let triggers = scorecard.to_reevaluation_triggers();
    let guidance = scorecard.to_mutation_guidance();

    let critical_triggers = triggers
        .iter()
        .filter(|t| t.priority == TriggerPriority::Critical)
        .count();

    FeedbackResult {
        agent_id: scorecard.agent_id.clone(),
        fitness: fitness_signal.fitness,
        gaming_discount: fitness_signal.gaming_discount,
        trigger_count: triggers.len(),
        critical_triggers,
        mutation_target_count: guidance.mutation_targets.len(),
        classification: guidance.classification,
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Compute gaming discount: multiplicative penalty per flag severity.
fn compute_gaming_discount(flags: &[GamingFlag]) -> f64 {
    let mut discount = 1.0;
    for flag in flags {
        match flag.severity {
            GamingFlagSeverity::Yellow => discount *= 0.95,
            GamingFlagSeverity::Orange => discount *= 0.85,
            GamingFlagSeverity::Red => discount *= 0.50,
        }
    }
    discount
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::*;
    use crate::reporting::scorecard::{OverallScore, VectorScorecard};

    fn make_scorecard(
        agent_id: &str,
        classification: AgentClassification,
        vector_scores: &[(Vector, f64)],
        flags: Vec<GamingFlag>,
    ) -> AgentScorecard {
        let vectors: Vec<VectorScorecard> = vector_scores
            .iter()
            .map(|(v, s)| VectorScorecard {
                vector: *v,
                score: *s,
                levels: vec![],
                articulation_total: 0.0,
                flags: vec![],
            })
            .collect();

        let scores: Vec<f64> = vectors.iter().map(|v| v.score).collect();
        let composite = scores.iter().sum::<f64>() / scores.len().max(1) as f64;
        let floor = scores.iter().cloned().fold(f64::MAX, f64::min);
        let ceiling = scores.iter().cloned().fold(f64::MIN, f64::max);

        AgentScorecard {
            agent_id: agent_id.to_string(),
            agent_autonomy_level: 3,
            measured_at: 0,
            vectors,
            overall: OverallScore {
                composite,
                floor,
                ceiling,
                effective_ceiling: None,
            },
            classification,
            gaming_flags: flags,
            audit_hash: String::new(),
        }
    }

    #[test]
    fn test_fitness_signal_from_balanced_agent() {
        let card = make_scorecard(
            "agent-balanced",
            AgentClassification::Balanced {
                min_score: 0.70,
                max_score: 0.80,
            },
            &[
                (Vector::ReasoningDepth, 0.75),
                (Vector::PlanningCoherence, 0.80),
                (Vector::AdaptationUnderUncertainty, 0.70),
                (Vector::ToolUseIntegrity, 0.75),
            ],
            vec![],
        );

        let signal = card.to_fitness_signal();
        assert_eq!(signal.agent_id, "agent-balanced");
        assert!(
            (signal.gaming_discount - 1.0).abs() < 1e-9,
            "No flags → no discount"
        );
        assert!(
            (signal.fitness - 0.75).abs() < 1e-9,
            "Fitness should equal composite (no discount)"
        );
        assert_eq!(signal.vector_fitness.len(), 4);
    }

    #[test]
    fn test_fitness_signal_gaming_discount() {
        let flags = vec![
            GamingFlag {
                flag_type: GamingFlagType::ConfidentAtLevel5,
                evidence: "test".into(),
                severity: GamingFlagSeverity::Red,
                requires_human_review: true,
            },
            GamingFlag {
                flag_type: GamingFlagType::HighPrimaryZeroArticulation,
                evidence: "test".into(),
                severity: GamingFlagSeverity::Orange,
                requires_human_review: true,
            },
        ];

        let card = make_scorecard(
            "agent-flagged",
            AgentClassification::Anomalous {
                reason: "test".into(),
            },
            &[(Vector::ReasoningDepth, 0.90)],
            flags,
        );

        let signal = card.to_fitness_signal();
        // Red = 0.50 × Orange = 0.85 → 0.425
        let expected_discount = 0.50 * 0.85;
        assert!(
            (signal.gaming_discount - expected_discount).abs() < 1e-9,
            "Expected discount {expected_discount}, got {}",
            signal.gaming_discount
        );
        assert!(
            signal.fitness < 0.90,
            "Fitness must be discounted from raw 0.90"
        );
    }

    #[test]
    fn test_reevaluation_triggers_from_red_flags() {
        let flags = vec![GamingFlag {
            flag_type: GamingFlagType::ConfidentAtLevel5,
            evidence: "confident on underspecified".into(),
            severity: GamingFlagSeverity::Red,
            requires_human_review: true,
        }];

        let card = make_scorecard(
            "agent-red",
            AgentClassification::Anomalous {
                reason: "test".into(),
            },
            &[(Vector::ReasoningDepth, 0.5)],
            flags,
        );

        let triggers = card.to_reevaluation_triggers();
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].priority, TriggerPriority::Critical);
        assert!(matches!(
            triggers[0].reason,
            ReevaluationReason::ConfidentUnderspecification
        ));
    }

    #[test]
    fn test_mutation_guidance_theoretical_reasoner() {
        let card = make_scorecard(
            "agent-theorist",
            AgentClassification::TheoreticalReasoner,
            &[
                (Vector::ReasoningDepth, 0.85),
                (Vector::PlanningCoherence, 0.40),
            ],
            vec![],
        );

        let guidance = card.to_mutation_guidance();
        assert_eq!(guidance.mutation_targets.len(), 1);
        assert_eq!(
            guidance.mutation_targets[0].vector,
            Vector::PlanningCoherence
        );
        assert!(matches!(
            guidance.mutation_targets[0].suggested_mutation,
            SuggestedMutation::AddPlanningScaffolding
        ));
    }

    #[test]
    fn test_mutation_guidance_rigid_tool_user() {
        let card = make_scorecard(
            "agent-rigid",
            AgentClassification::RigidToolUser,
            &[
                (Vector::ToolUseIntegrity, 0.80),
                (Vector::AdaptationUnderUncertainty, 0.30),
            ],
            vec![],
        );

        let guidance = card.to_mutation_guidance();
        assert_eq!(guidance.mutation_targets.len(), 1);
        assert_eq!(
            guidance.mutation_targets[0].vector,
            Vector::AdaptationUnderUncertainty
        );
        assert!(matches!(
            guidance.mutation_targets[0].suggested_mutation,
            SuggestedMutation::AddAdaptationExposure
        ));
    }

    #[test]
    fn test_anomalous_agent_no_automated_mutation() {
        let card = make_scorecard(
            "agent-anomalous",
            AgentClassification::Anomalous {
                reason: "requires human review".into(),
            },
            &[(Vector::ReasoningDepth, 0.5)],
            vec![],
        );

        let guidance = card.to_mutation_guidance();
        assert!(
            guidance.mutation_targets.is_empty(),
            "Anomalous agents should get no automated mutations"
        );
    }

    #[test]
    fn test_pattern_matching_ceiling_targets_diversity() {
        let card = make_scorecard(
            "agent-ceiling",
            AgentClassification::PatternMatchingCeiling {
                ceiling_level: DifficultyLevel::Level3,
            },
            &[
                (Vector::ReasoningDepth, 0.60),
                (Vector::PlanningCoherence, 0.55),
                (Vector::AdaptationUnderUncertainty, 0.35),
                (Vector::ToolUseIntegrity, 0.50),
            ],
            vec![],
        );

        let guidance = card.to_mutation_guidance();
        assert_eq!(guidance.mutation_targets.len(), 1);
        // Should target the weakest vector (Adaptation at 0.35)
        assert_eq!(
            guidance.mutation_targets[0].vector,
            Vector::AdaptationUnderUncertainty
        );
        assert!(matches!(
            guidance.mutation_targets[0].suggested_mutation,
            SuggestedMutation::ExpandProblemDiversity
        ));
    }

    #[test]
    fn test_feedback_result_summary() {
        let flags = vec![
            GamingFlag {
                flag_type: GamingFlagType::InvertedDifficultySpectrum,
                evidence: "test".into(),
                severity: GamingFlagSeverity::Orange,
                requires_human_review: true,
            },
            GamingFlag {
                flag_type: GamingFlagType::ConfidentAtLevel5,
                evidence: "test".into(),
                severity: GamingFlagSeverity::Red,
                requires_human_review: true,
            },
        ];

        let card = make_scorecard(
            "agent-feedback",
            AgentClassification::Balanced {
                min_score: 0.5,
                max_score: 0.7,
            },
            &[
                (Vector::ReasoningDepth, 0.60),
                (Vector::PlanningCoherence, 0.70),
            ],
            flags,
        );

        let result = run_measurement_feedback(&card);
        assert_eq!(result.agent_id, "agent-feedback");
        assert_eq!(result.trigger_count, 2);
        assert_eq!(result.critical_triggers, 1); // One Red flag
        assert!(result.gaming_discount < 1.0);
    }
}
