//! Test battery execution engine.

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::battery::test_problem::TestProblem;
use crate::evaluation::comparator::{self, ResponseComparator};
use crate::framework::*;
use crate::scoring::articulation::empty_articulation;
use crate::scoring::asymmetric::compute_primary_score;
use crate::scoring::gaming_detection::{
    detect_confident_at_level5, detect_high_primary_zero_articulation, detect_inverted_difficulty,
    GamingFlag,
};

/// Agent response from a test problem.
#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub response_text: String,
    pub reasoning_trace: Option<String>,
    pub elapsed_ms: u64,
}

/// Evaluation error.
#[derive(Debug, thiserror::Error)]
pub enum EvaluationError {
    #[error("Agent did not respond within timeout")]
    Timeout,
    #[error("Agent interface error: {0}")]
    InterfaceError(String),
    #[error("Problem not locked: {0}")]
    ProblemNotLocked(String),
    #[error("Scoring error: {0}")]
    ScoringError(String),
}

/// The evaluation runner executes a complete measurement session.
pub struct EvaluationRunner {
    batteries: Vec<TestProblem>,
    /// Optional LLM-as-judge comparator. When set, replaces keyword-based scoring.
    comparator: Option<ResponseComparator>,
}

impl EvaluationRunner {
    pub fn new(batteries: Vec<TestProblem>) -> Self {
        Self {
            batteries,
            comparator: None,
        }
    }

    /// Create a runner with an LLM-as-judge comparator for semantic scoring.
    pub fn with_comparator(batteries: Vec<TestProblem>, comparator: ResponseComparator) -> Self {
        Self {
            batteries,
            comparator: Some(comparator),
        }
    }

    /// Run a measurement session using a provided response function.
    ///
    /// `respond_fn` takes a problem statement and returns the agent's response.
    pub fn run_session_with<F>(
        &self,
        agent_id: &str,
        agent_autonomy_level: u8,
        respond_fn: F,
    ) -> Result<MeasurementSession, EvaluationError>
    where
        F: Fn(&str) -> Result<AgentResponse, EvaluationError>,
    {
        let session_id = Uuid::new_v4();
        let started_at = epoch_secs();
        let mut vector_results = Vec::new();

        for vector in &[
            Vector::ReasoningDepth,
            Vector::PlanningCoherence,
            Vector::AdaptationUnderUncertainty,
            Vector::ToolUseIntegrity,
        ] {
            let battery: Vec<&TestProblem> = self
                .batteries
                .iter()
                .filter(|p| p.vector == *vector && p.locked)
                .collect();

            let mut level_results = Vec::new();
            let mut gaming_flags: Vec<GamingFlag> = Vec::new();

            for problem in &battery {
                let response = respond_fn(&problem.problem_statement)?;

                // Score using LLM-as-judge if available, otherwise keyword-based
                let (primary_score, articulation_score, mut level_flags) =
                    if let Some(ref comp) = self.comparator {
                        self.score_with_comparator(comp, problem, &response.response_text, *vector)
                    } else {
                        self.score_with_keywords(problem, &response.response_text, *vector)
                    };

                // Rule-based gaming detection (always runs)
                if let Some(flag) =
                    detect_confident_at_level5(problem.level, &response.response_text)
                {
                    level_flags.push(flag);
                }
                if let Some(flag) = detect_high_primary_zero_articulation(
                    primary_score.adjusted_score,
                    articulation_score.total,
                ) {
                    level_flags.push(flag);
                }

                level_results.push(LevelResult {
                    level: problem.level,
                    problem_id: problem.id.clone(),
                    problem_version: problem.version.clone(),
                    agent_response: response.response_text,
                    primary_score,
                    articulation_score,
                    gaming_flags: level_flags.clone(),
                    scorer_agreement: ScorerAgreement::Pending,
                });

                gaming_flags.extend(level_flags);
            }

            // Cross-level gaming detection
            if let Some(flag) = detect_inverted_difficulty(&level_results) {
                gaming_flags.push(flag);
            }

            let vector_score = compute_vector_score(&level_results);

            vector_results.push(VectorResult {
                vector: *vector,
                level_results,
                gaming_flags,
                vector_score,
            });
        }

        let cross_vector_analysis = analyze_cross_vector(&vector_results);
        let audit_hash = compute_audit_hash(&vector_results);

        Ok(MeasurementSession {
            id: session_id,
            agent_id: agent_id.to_string(),
            agent_autonomy_level,
            started_at,
            completed_at: Some(epoch_secs()),
            vector_results,
            cross_vector_analysis: Some(cross_vector_analysis),
            audit_hash,
        })
    }

    /// Score using the keyword-based comparator (fallback path).
    fn score_with_keywords(
        &self,
        problem: &TestProblem,
        response: &str,
        vector: Vector,
    ) -> (
        crate::scoring::asymmetric::PrimaryScore,
        crate::scoring::articulation::ArticulationScore,
        Vec<GamingFlag>,
    ) {
        let (coverage, gaps, redundancies, hallucinations) =
            comparator::compare_response(response, &problem.expected_reasoning);
        let primary = compute_primary_score(vector, coverage, gaps, redundancies, hallucinations);
        let articulation = empty_articulation(vector);
        (primary, articulation, Vec::new())
    }

    /// Score using the LLM-as-judge comparator.
    fn score_with_comparator(
        &self,
        comp: &ResponseComparator,
        problem: &TestProblem,
        response: &str,
        vector: Vector,
    ) -> (
        crate::scoring::asymmetric::PrimaryScore,
        crate::scoring::articulation::ArticulationScore,
        Vec<GamingFlag>,
    ) {
        // Primary scoring via LLM judge
        let primary = comp
            .score_primary(problem, response, vector)
            .unwrap_or_else(|_| {
                // Fall back to keyword scoring on judge error
                let (coverage, gaps, redundancies, hallucinations) =
                    comparator::compare_response(response, &problem.expected_reasoning);
                compute_primary_score(vector, coverage, gaps, redundancies, hallucinations)
            });

        // Articulation scoring via LLM judge
        let articulation = comp
            .score_articulation(problem, response, vector)
            .unwrap_or_else(|_| empty_articulation(vector));

        // Gaming detection via LLM judge
        let flags = comp
            .detect_gaming(problem, response, vector)
            .unwrap_or_default();

        (primary, articulation, flags)
    }
}

/// Compute weighted vector score from level results.
pub fn compute_vector_score(results: &[LevelResult]) -> f64 {
    let mut total = 0.0;
    for result in results {
        total += result.primary_score.adjusted_score * result.level.weight();
    }
    total
}

/// Classify agent based on cross-vector score profile shape.
pub fn analyze_cross_vector(results: &[VectorResult]) -> CrossVectorAnalysis {
    let scores: Vec<(Vector, f64)> = results.iter().map(|r| (r.vector, r.vector_score)).collect();

    let min_score = scores.iter().map(|(_, s)| *s).fold(f64::MAX, f64::min);
    let max_score = scores.iter().map(|(_, s)| *s).fold(f64::MIN, f64::max);
    let spread = max_score - min_score;

    let classification = if spread < 0.15 {
        AgentClassification::Balanced {
            min_score,
            max_score,
        }
    } else {
        classify_profile(&scores)
    };

    let get_score = |v: Vector| -> f64 {
        scores
            .iter()
            .find(|(vec, _)| *vec == v)
            .map(|(_, s)| *s)
            .unwrap_or(0.0)
    };

    CrossVectorAnalysis {
        capability_profile: CapabilityProfile {
            reasoning_depth: get_score(Vector::ReasoningDepth),
            planning_coherence: get_score(Vector::PlanningCoherence),
            adaptation: get_score(Vector::AdaptationUnderUncertainty),
            tool_use: get_score(Vector::ToolUseIntegrity),
        },
        anomalies: Vec::new(),
        overall_classification: classification,
    }
}

fn classify_profile(scores: &[(Vector, f64)]) -> AgentClassification {
    let get = |v: Vector| -> f64 {
        scores
            .iter()
            .find(|(vec, _)| *vec == v)
            .map(|(_, s)| *s)
            .unwrap_or(0.0)
    };

    let reasoning = get(Vector::ReasoningDepth);
    let planning = get(Vector::PlanningCoherence);
    let adaptation = get(Vector::AdaptationUnderUncertainty);
    let tool_use = get(Vector::ToolUseIntegrity);

    if reasoning > planning + 0.2 && reasoning > adaptation + 0.2 {
        AgentClassification::TheoreticalReasoner
    } else if planning > reasoning + 0.2 {
        AgentClassification::ProceduralExecutor
    } else if tool_use > adaptation + 0.2 {
        AgentClassification::RigidToolUser
    } else {
        AgentClassification::Anomalous {
            reason: "Uneven profile does not match known patterns".into(),
        }
    }
}

/// Deterministic audit hash from vector results.
pub fn compute_audit_hash(results: &[VectorResult]) -> String {
    let serialized = serde_json::to_string(results).unwrap_or_default();
    let hash = Sha256::digest(serialized.as_bytes());
    format!("{:x}", hash)
}

fn epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_hash_deterministic() {
        let results = vec![VectorResult {
            vector: Vector::ReasoningDepth,
            level_results: vec![],
            gaming_flags: vec![],
            vector_score: 0.75,
        }];
        let hash1 = compute_audit_hash(&results);
        let hash2 = compute_audit_hash(&results);
        assert_eq!(hash1, hash2, "Same results must produce same hash");
    }

    #[test]
    fn test_cross_vector_balanced_classification() {
        let results = vec![
            VectorResult {
                vector: Vector::ReasoningDepth,
                level_results: vec![],
                gaming_flags: vec![],
                vector_score: 0.72,
            },
            VectorResult {
                vector: Vector::PlanningCoherence,
                level_results: vec![],
                gaming_flags: vec![],
                vector_score: 0.75,
            },
            VectorResult {
                vector: Vector::AdaptationUnderUncertainty,
                level_results: vec![],
                gaming_flags: vec![],
                vector_score: 0.70,
            },
            VectorResult {
                vector: Vector::ToolUseIntegrity,
                level_results: vec![],
                gaming_flags: vec![],
                vector_score: 0.73,
            },
        ];
        let analysis = analyze_cross_vector(&results);
        assert!(
            matches!(
                analysis.overall_classification,
                AgentClassification::Balanced { .. }
            ),
            "Tight spread (<0.15) should classify as Balanced, got {:?}",
            analysis.overall_classification,
        );
    }
}
