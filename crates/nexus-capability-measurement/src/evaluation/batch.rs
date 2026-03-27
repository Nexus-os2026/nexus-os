//! Batch evaluator — runs the complete test battery against multiple agents
//! and produces the capability boundary map.

use serde::{Deserialize, Serialize};

use crate::battery::test_problem::TestProblem;
use crate::evaluation::agent_adapter::AgentAdapter;
use crate::evaluation::comparator::ResponseComparator;
use crate::evaluation::runner::EvaluationRunner;
use crate::framework::*;
use crate::scoring::gaming_detection::{GamingFlag, GamingFlagSeverity};

// ── Batch Evaluator ──────────────────────────────────────────────────────────

/// Runs the full test battery against a set of agents.
pub struct BatchEvaluator {
    runner: EvaluationRunner,
}

impl BatchEvaluator {
    pub fn new(battery: Vec<TestProblem>, comparator: Option<ResponseComparator>) -> Self {
        let runner = match comparator {
            Some(c) => EvaluationRunner::with_comparator(battery, c),
            None => EvaluationRunner::new(battery),
        };
        Self { runner }
    }

    /// Evaluate a single agent across all vectors and levels.
    pub fn evaluate_agent(&self, adapter: &AgentAdapter) -> Result<MeasurementSession, String> {
        self.runner
            .run_session_with(&adapter.agent_id, adapter.autonomy_level, |problem_text| {
                adapter
                    .evaluate(problem_text)
                    .map_err(crate::evaluation::runner::EvaluationError::InterfaceError)
            })
            .map_err(|e| e.to_string())
    }

    /// Evaluate ALL agents and produce the full capability boundary map.
    #[allow(clippy::type_complexity)]
    pub fn evaluate_all(
        &self,
        adapters: &[AgentAdapter],
        progress_callback: Option<&dyn Fn(usize, usize, &str)>,
    ) -> BatchResult {
        let total = adapters.len();
        let mut sessions = Vec::new();
        let mut failures = Vec::new();

        for (i, adapter) in adapters.iter().enumerate() {
            if let Some(cb) = &progress_callback {
                cb(i + 1, total, &adapter.agent_id);
            }

            match self.evaluate_agent(adapter) {
                Ok(session) => sessions.push(session),
                Err(e) => failures.push((adapter.agent_id.clone(), e)),
            }
        }

        let boundary_map = build_boundary_map(&sessions);
        let census = build_classification_census(&sessions);
        let gaming_report = build_gaming_report(&sessions);
        let calibration = check_calibration(&sessions);

        BatchResult {
            sessions,
            failures,
            boundary_map,
            census,
            gaming_report,
            calibration,
        }
    }
}

// ── Boundary Map ─────────────────────────────────────────────────────────────

const CEILING_THRESHOLD: f64 = 0.5;

/// Find the exact difficulty level where each agent drops below threshold.
pub fn build_boundary_map(sessions: &[MeasurementSession]) -> Vec<AgentBoundary> {
    sessions
        .iter()
        .map(|session| {
            let mut vector_ceilings = Vec::new();

            for vr in &session.vector_results {
                let ceiling = vr
                    .level_results
                    .iter()
                    .find(|lr| lr.primary_score.adjusted_score < CEILING_THRESHOLD)
                    .map(|lr| lr.level);

                let score_at_ceiling = ceiling
                    .map(|_| {
                        vr.level_results
                            .iter()
                            .find(|lr| lr.primary_score.adjusted_score < CEILING_THRESHOLD)
                            .map(|lr| lr.primary_score.adjusted_score)
                            .unwrap_or(0.0)
                    })
                    .unwrap_or(1.0);

                vector_ceilings.push(VectorCeiling {
                    vector: vr.vector,
                    ceiling_level: ceiling,
                    score_at_ceiling,
                });
            }

            let overall_ceiling = vector_ceilings
                .iter()
                .filter_map(|vc| vc.ceiling_level)
                .min();

            let composite = if session.vector_results.is_empty() {
                0.0
            } else {
                session
                    .vector_results
                    .iter()
                    .map(|vr| vr.vector_score)
                    .sum::<f64>()
                    / session.vector_results.len() as f64
            };

            AgentBoundary {
                agent_id: session.agent_id.clone(),
                autonomy_level: session.agent_autonomy_level,
                overall_ceiling,
                vector_ceilings,
                composite_score: composite,
            }
        })
        .collect()
}

// ── Classification Census ────────────────────────────────────────────────────

pub fn build_classification_census(sessions: &[MeasurementSession]) -> ClassificationCensus {
    let mut census = ClassificationCensus {
        total: sessions.len(),
        ..ClassificationCensus::default()
    };

    for session in sessions {
        if let Some(analysis) = &session.cross_vector_analysis {
            match &analysis.overall_classification {
                AgentClassification::Balanced { .. } => census.balanced += 1,
                AgentClassification::TheoreticalReasoner => census.theoretical_reasoner += 1,
                AgentClassification::ProceduralExecutor => census.procedural_executor += 1,
                AgentClassification::RigidToolUser => census.rigid_tool_user += 1,
                AgentClassification::PatternMatchingCeiling { .. } => census.pattern_matching += 1,
                AgentClassification::Anomalous { .. } => census.anomalous += 1,
            }
        }
    }

    census
}

// ── Gaming Report ────────────────────────────────────────────────────────────

pub fn build_gaming_report(sessions: &[MeasurementSession]) -> GamingReport {
    let mut flags_by_agent = Vec::new();
    let mut red_count = 0;
    let mut orange_count = 0;
    let mut yellow_count = 0;

    for session in sessions {
        let agent_flags: Vec<GamingFlag> = session
            .vector_results
            .iter()
            .flat_map(|vr| vr.gaming_flags.clone())
            .collect();

        for flag in &agent_flags {
            match flag.severity {
                GamingFlagSeverity::Red => red_count += 1,
                GamingFlagSeverity::Orange => orange_count += 1,
                GamingFlagSeverity::Yellow => yellow_count += 1,
            }
        }

        if !agent_flags.is_empty() {
            flags_by_agent.push(AgentGamingFlags {
                agent_id: session.agent_id.clone(),
                flags: agent_flags,
            });
        }
    }

    GamingReport {
        total_flags: red_count + orange_count + yellow_count,
        red_count,
        orange_count,
        yellow_count,
        agents_with_flags: flags_by_agent.len(),
        agents_clean: sessions.len() - flags_by_agent.len(),
        flags_by_agent,
    }
}

// ── Calibration Check ────────────────────────────────────────────────────────

/// Verify the difficulty spectrum is monotonically increasing.
pub fn check_calibration(sessions: &[MeasurementSession]) -> CalibrationReport {
    let mut inversions = Vec::new();
    let vectors = [
        Vector::ReasoningDepth,
        Vector::PlanningCoherence,
        Vector::AdaptationUnderUncertainty,
        Vector::ToolUseIntegrity,
    ];
    let levels = [
        DifficultyLevel::Level1,
        DifficultyLevel::Level2,
        DifficultyLevel::Level3,
        DifficultyLevel::Level4,
        DifficultyLevel::Level5,
    ];

    for vector in &vectors {
        let mut level_averages: Vec<(DifficultyLevel, f64)> = Vec::new();

        for level in &levels {
            let scores: Vec<f64> = sessions
                .iter()
                .flat_map(|s| s.vector_results.iter())
                .filter(|vr| vr.vector == *vector)
                .flat_map(|vr| vr.level_results.iter())
                .filter(|lr| lr.level == *level)
                .map(|lr| lr.primary_score.adjusted_score)
                .collect();

            if !scores.is_empty() {
                let avg = scores.iter().sum::<f64>() / scores.len() as f64;
                level_averages.push((*level, avg));
            }
        }

        for window in level_averages.windows(2) {
            let (lower_level, lower_avg) = window[0];
            let (higher_level, higher_avg) = window[1];

            if higher_avg > lower_avg + 0.05 {
                inversions.push(CalibrationInversion {
                    vector: *vector,
                    lower_level,
                    lower_avg,
                    higher_level,
                    higher_avg,
                });
            }
        }
    }

    CalibrationReport {
        is_calibrated: inversions.is_empty(),
        inversions,
    }
}

// ── Result Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub sessions: Vec<MeasurementSession>,
    pub failures: Vec<(String, String)>,
    pub boundary_map: Vec<AgentBoundary>,
    pub census: ClassificationCensus,
    pub gaming_report: GamingReport,
    pub calibration: CalibrationReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBoundary {
    pub agent_id: String,
    pub autonomy_level: u8,
    pub overall_ceiling: Option<DifficultyLevel>,
    pub vector_ceilings: Vec<VectorCeiling>,
    pub composite_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorCeiling {
    pub vector: Vector,
    pub ceiling_level: Option<DifficultyLevel>,
    pub score_at_ceiling: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClassificationCensus {
    pub total: usize,
    pub balanced: usize,
    pub theoretical_reasoner: usize,
    pub procedural_executor: usize,
    pub rigid_tool_user: usize,
    pub pattern_matching: usize,
    pub anomalous: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingReport {
    pub total_flags: usize,
    pub red_count: usize,
    pub orange_count: usize,
    pub yellow_count: usize,
    pub agents_with_flags: usize,
    pub agents_clean: usize,
    pub flags_by_agent: Vec<AgentGamingFlags>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentGamingFlags {
    pub agent_id: String,
    pub flags: Vec<GamingFlag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationReport {
    pub is_calibrated: bool,
    pub inversions: Vec<CalibrationInversion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationInversion {
    pub vector: Vector,
    pub lower_level: DifficultyLevel,
    pub lower_avg: f64,
    pub higher_level: DifficultyLevel,
    pub higher_avg: f64,
}

/// Summary of Darwin Core upload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DarwinUploadSummary {
    pub agents_uploaded: usize,
    pub fitness_signals: usize,
    pub reevaluation_triggers: usize,
    pub mutation_targets: usize,
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battery::expected_chain::ExpectedReasoning;
    use crate::battery::test_problem::{ProblemContext, ScoringRubric, TestProblem};
    use crate::evaluation::agent_adapter::AgentAdapter;
    use crate::scoring::articulation::empty_articulation;
    use crate::scoring::asymmetric::PrimaryScore;
    use crate::scoring::gaming_detection::GamingDetectionRule;

    fn make_problem(vector: Vector, level: DifficultyLevel) -> TestProblem {
        TestProblem {
            id: format!("test-{vector:?}-{level:?}"),
            version: "v1.0.0-locked".into(),
            vector,
            level,
            problem_statement: format!("Test problem for {vector:?} at {level:?}"),
            context: ProblemContext {
                initial_state: serde_json::Value::Null,
                mid_problem_updates: vec![],
                available_tools: vec![],
            },
            expected_reasoning: ExpectedReasoning {
                causal_chain: vec![],
                expected_plan: None,
                expected_adaptation: None,
                expected_tool_use: None,
                required_insights: vec!["test insight".into()],
                critical_failures: vec![],
            },
            scoring_rubric: ScoringRubric {
                full_credit: vec!["correct".into()],
                partial_credit: vec![],
                zero_credit: vec![],
            },
            gaming_detection: vec![],
            locked: true,
            locked_at: Some(0),
        }
    }

    fn make_battery() -> Vec<TestProblem> {
        let mut battery = Vec::new();
        for vector in [
            Vector::ReasoningDepth,
            Vector::PlanningCoherence,
            Vector::AdaptationUnderUncertainty,
            Vector::ToolUseIntegrity,
        ] {
            for level in [
                DifficultyLevel::Level1,
                DifficultyLevel::Level2,
                DifficultyLevel::Level3,
                DifficultyLevel::Level4,
                DifficultyLevel::Level5,
            ] {
                battery.push(make_problem(vector, level));
            }
        }
        battery
    }

    #[test]
    fn test_batch_evaluator_all_vectors() {
        let battery = make_battery();
        let evaluator = BatchEvaluator::new(battery, None);

        let adapter = AgentAdapter::new("strong-agent".into(), 3, |_prompt| {
            Ok("This is a test insight with detailed reasoning.".into())
        });

        let session = evaluator.evaluate_agent(&adapter).unwrap();
        assert_eq!(session.vector_results.len(), 4);
        assert_eq!(session.agent_id, "strong-agent");
        assert_eq!(session.agent_autonomy_level, 3);
    }

    #[test]
    fn test_boundary_map_finds_ceiling() {
        // Create a session where scores decline
        let session = MeasurementSession {
            id: uuid::Uuid::new_v4(),
            agent_id: "ceiling-agent".into(),
            agent_autonomy_level: 2,
            started_at: 0,
            completed_at: Some(0),
            vector_results: vec![VectorResult {
                vector: Vector::ReasoningDepth,
                level_results: vec![
                    LevelResult {
                        level: DifficultyLevel::Level1,
                        problem_id: "p1".into(),
                        problem_version: "v1".into(),
                        agent_response: String::new(),
                        primary_score: PrimaryScore {
                            raw_score: 0.9,
                            penalties: vec![],
                            adjusted_score: 0.9,
                        },
                        articulation_score: empty_articulation(Vector::ReasoningDepth),
                        gaming_flags: vec![],
                        scorer_agreement: ScorerAgreement::Pending,
                    },
                    LevelResult {
                        level: DifficultyLevel::Level2,
                        problem_id: "p2".into(),
                        problem_version: "v1".into(),
                        agent_response: String::new(),
                        primary_score: PrimaryScore {
                            raw_score: 0.6,
                            penalties: vec![],
                            adjusted_score: 0.6,
                        },
                        articulation_score: empty_articulation(Vector::ReasoningDepth),
                        gaming_flags: vec![],
                        scorer_agreement: ScorerAgreement::Pending,
                    },
                    LevelResult {
                        level: DifficultyLevel::Level3,
                        problem_id: "p3".into(),
                        problem_version: "v1".into(),
                        agent_response: String::new(),
                        primary_score: PrimaryScore {
                            raw_score: 0.3,
                            penalties: vec![],
                            adjusted_score: 0.3,
                        },
                        articulation_score: empty_articulation(Vector::ReasoningDepth),
                        gaming_flags: vec![],
                        scorer_agreement: ScorerAgreement::Pending,
                    },
                ],
                gaming_flags: vec![],
                vector_score: 0.5,
            }],
            cross_vector_analysis: None,
            audit_hash: String::new(),
        };

        let map = build_boundary_map(&[session]);
        assert_eq!(map.len(), 1);
        assert_eq!(
            map[0].vector_ceilings[0].ceiling_level,
            Some(DifficultyLevel::Level3)
        );
    }

    #[test]
    fn test_boundary_map_no_ceiling() {
        let session = MeasurementSession {
            id: uuid::Uuid::new_v4(),
            agent_id: "strong".into(),
            agent_autonomy_level: 5,
            started_at: 0,
            completed_at: Some(0),
            vector_results: vec![VectorResult {
                vector: Vector::ReasoningDepth,
                level_results: vec![LevelResult {
                    level: DifficultyLevel::Level1,
                    problem_id: "p1".into(),
                    problem_version: "v1".into(),
                    agent_response: String::new(),
                    primary_score: PrimaryScore {
                        raw_score: 0.9,
                        penalties: vec![],
                        adjusted_score: 0.9,
                    },
                    articulation_score: empty_articulation(Vector::ReasoningDepth),
                    gaming_flags: vec![],
                    scorer_agreement: ScorerAgreement::Pending,
                }],
                gaming_flags: vec![],
                vector_score: 0.9,
            }],
            cross_vector_analysis: None,
            audit_hash: String::new(),
        };

        let map = build_boundary_map(&[session]);
        assert!(
            map[0].vector_ceilings[0].ceiling_level.is_none(),
            "Agent never hits ceiling"
        );
    }

    #[test]
    fn test_classification_census_counts() {
        let sessions: Vec<MeasurementSession> = vec![
            make_session_with_classification(
                "a1",
                AgentClassification::Balanced {
                    min_score: 0.7,
                    max_score: 0.8,
                },
            ),
            make_session_with_classification("a2", AgentClassification::TheoreticalReasoner),
            make_session_with_classification(
                "a3",
                AgentClassification::Balanced {
                    min_score: 0.6,
                    max_score: 0.7,
                },
            ),
        ];

        let census = build_classification_census(&sessions);
        assert_eq!(census.total, 3);
        assert_eq!(census.balanced, 2);
        assert_eq!(census.theoretical_reasoner, 1);
    }

    #[test]
    fn test_gaming_report_severity_counts() {
        use crate::scoring::gaming_detection::GamingFlagType;

        let session = MeasurementSession {
            id: uuid::Uuid::new_v4(),
            agent_id: "flagged".into(),
            agent_autonomy_level: 3,
            started_at: 0,
            completed_at: Some(0),
            vector_results: vec![VectorResult {
                vector: Vector::ReasoningDepth,
                level_results: vec![],
                gaming_flags: vec![
                    GamingFlag {
                        flag_type: GamingFlagType::ConfidentAtLevel5,
                        evidence: "e".into(),
                        severity: GamingFlagSeverity::Red,
                        requires_human_review: true,
                    },
                    GamingFlag {
                        flag_type: GamingFlagType::HighPrimaryZeroArticulation,
                        evidence: "e".into(),
                        severity: GamingFlagSeverity::Orange,
                        requires_human_review: true,
                    },
                    GamingFlag {
                        flag_type: GamingFlagType::TerminologyWithoutCausation,
                        evidence: "e".into(),
                        severity: GamingFlagSeverity::Yellow,
                        requires_human_review: false,
                    },
                ],
                vector_score: 0.5,
            }],
            cross_vector_analysis: None,
            audit_hash: String::new(),
        };

        let report = build_gaming_report(&[session]);
        assert_eq!(report.red_count, 1);
        assert_eq!(report.orange_count, 1);
        assert_eq!(report.yellow_count, 1);
        assert_eq!(report.total_flags, 3);
        assert_eq!(report.agents_with_flags, 1);
    }

    #[test]
    fn test_calibration_clean() {
        // Monotonically decreasing scores = calibrated
        let session = make_session_with_declining_scores("a1");
        let report = check_calibration(&[session]);
        assert!(
            report.is_calibrated,
            "Declining scores should be calibrated"
        );
    }

    #[test]
    fn test_calibration_inversion() {
        // Level 2 scores higher than Level 1 = inversion
        let session = MeasurementSession {
            id: uuid::Uuid::new_v4(),
            agent_id: "inverted".into(),
            agent_autonomy_level: 3,
            started_at: 0,
            completed_at: Some(0),
            vector_results: vec![VectorResult {
                vector: Vector::ReasoningDepth,
                level_results: vec![
                    LevelResult {
                        level: DifficultyLevel::Level1,
                        problem_id: "p1".into(),
                        problem_version: "v1".into(),
                        agent_response: String::new(),
                        primary_score: PrimaryScore {
                            raw_score: 0.3,
                            penalties: vec![],
                            adjusted_score: 0.3,
                        },
                        articulation_score: empty_articulation(Vector::ReasoningDepth),
                        gaming_flags: vec![],
                        scorer_agreement: ScorerAgreement::Pending,
                    },
                    LevelResult {
                        level: DifficultyLevel::Level2,
                        problem_id: "p2".into(),
                        problem_version: "v1".into(),
                        agent_response: String::new(),
                        primary_score: PrimaryScore {
                            raw_score: 0.8,
                            penalties: vec![],
                            adjusted_score: 0.8,
                        },
                        articulation_score: empty_articulation(Vector::ReasoningDepth),
                        gaming_flags: vec![],
                        scorer_agreement: ScorerAgreement::Pending,
                    },
                ],
                gaming_flags: vec![],
                vector_score: 0.5,
            }],
            cross_vector_analysis: None,
            audit_hash: String::new(),
        };

        let report = check_calibration(&[session]);
        assert!(!report.is_calibrated);
        assert!(!report.inversions.is_empty());
    }

    #[test]
    fn test_batch_result_includes_failures() {
        let battery = make_battery();
        let evaluator = BatchEvaluator::new(battery, None);

        let adapters = vec![
            AgentAdapter::new("ok-agent".into(), 3, |_| Ok("test insight".into())),
            AgentAdapter::new("fail-agent".into(), 1, |_| Err("boom".into())),
        ];

        let result = evaluator.evaluate_all(&adapters, None);
        assert_eq!(result.sessions.len(), 1);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].0, "fail-agent");
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    fn make_session_with_classification(
        id: &str,
        class: AgentClassification,
    ) -> MeasurementSession {
        MeasurementSession {
            id: uuid::Uuid::new_v4(),
            agent_id: id.into(),
            agent_autonomy_level: 3,
            started_at: 0,
            completed_at: Some(0),
            vector_results: vec![],
            cross_vector_analysis: Some(CrossVectorAnalysis {
                capability_profile: CapabilityProfile {
                    reasoning_depth: 0.5,
                    planning_coherence: 0.5,
                    adaptation: 0.5,
                    tool_use: 0.5,
                },
                anomalies: vec![],
                overall_classification: class,
            }),
            audit_hash: String::new(),
        }
    }

    fn make_session_with_declining_scores(id: &str) -> MeasurementSession {
        MeasurementSession {
            id: uuid::Uuid::new_v4(),
            agent_id: id.into(),
            agent_autonomy_level: 3,
            started_at: 0,
            completed_at: Some(0),
            vector_results: vec![VectorResult {
                vector: Vector::ReasoningDepth,
                level_results: [0.9, 0.7, 0.5, 0.3, 0.1]
                    .iter()
                    .zip(
                        [
                            DifficultyLevel::Level1,
                            DifficultyLevel::Level2,
                            DifficultyLevel::Level3,
                            DifficultyLevel::Level4,
                            DifficultyLevel::Level5,
                        ]
                        .iter(),
                    )
                    .map(|(score, level)| LevelResult {
                        level: *level,
                        problem_id: format!("p{level:?}"),
                        problem_version: "v1".into(),
                        agent_response: String::new(),
                        primary_score: PrimaryScore {
                            raw_score: *score,
                            penalties: vec![],
                            adjusted_score: *score,
                        },
                        articulation_score: empty_articulation(Vector::ReasoningDepth),
                        gaming_flags: vec![],
                        scorer_agreement: ScorerAgreement::Pending,
                    })
                    .collect(),
                gaming_flags: vec![],
                vector_score: 0.5,
            }],
            cross_vector_analysis: None,
            audit_hash: String::new(),
        }
    }
}
