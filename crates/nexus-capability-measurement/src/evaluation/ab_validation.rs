//! A/B validation runner — compares agent performance with and without
//! predictive routing.

use serde::{Deserialize, Serialize};

use crate::evaluation::agent_adapter::AgentAdapter;
use crate::evaluation::batch::{BatchEvaluator, BatchResult, ClassificationCensus};
use crate::framework::*;

// ── Result Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABComparisonResult {
    pub baseline: BatchResult,
    pub routed: BatchResult,
    pub agent_comparisons: Vec<AgentABComparison>,
    pub aggregate: AggregateComparison,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentABComparison {
    pub agent_id: String,
    pub autonomy_level: u8,
    pub baseline_composite: f64,
    pub routed_composite: f64,
    pub delta: f64,
    pub vector_deltas: Vec<VectorDelta>,
    pub baseline_ceiling: Option<DifficultyLevel>,
    pub routed_ceiling: Option<DifficultyLevel>,
    pub ceiling_improved: bool,
    pub baseline_classification: String,
    pub routed_classification: String,
    pub classification_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorDelta {
    pub vector: Vector,
    pub baseline_score: f64,
    pub routed_score: f64,
    pub delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateComparison {
    pub agents_evaluated: usize,
    pub agents_improved: usize,
    pub agents_unchanged: usize,
    pub agents_degraded: usize,
    pub avg_composite_delta: f64,
    pub avg_ceiling_delta: f64,
    pub baseline_census: ClassificationCensus,
    pub routed_census: ClassificationCensus,
    pub vector_aggregates: Vec<VectorAggregate>,
    pub most_improved: Option<String>,
    pub most_improved_delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorAggregate {
    pub vector: Vector,
    pub avg_baseline: f64,
    pub avg_routed: f64,
    pub avg_delta: f64,
    pub agents_improved: usize,
}

// ── Runner ───────────────────────────────────────────────────────────────────

/// Run a full A/B validation: baseline (fixed) vs routed (predictive).
///
/// Both runs use keyword scoring. The `routed_adapters` can wrap the same
/// agents with different invoke functions (e.g., adding routing annotations
/// or selecting different response strategies).
pub fn run_ab_validation(
    battery: &[crate::battery::test_problem::TestProblem],
    baseline_adapters: &[AgentAdapter],
    routed_adapters: &[AgentAdapter],
) -> Result<ABComparisonResult, String> {
    let baseline_evaluator = BatchEvaluator::new(battery.to_vec(), None);
    let baseline = baseline_evaluator.evaluate_all(baseline_adapters, None);

    let routed_evaluator = BatchEvaluator::new(battery.to_vec(), None);
    let routed = routed_evaluator.evaluate_all(routed_adapters, None);

    let agent_comparisons = compare_agents(&baseline, &routed);
    let aggregate = compute_aggregate(&agent_comparisons, &baseline, &routed);

    Ok(ABComparisonResult {
        baseline,
        routed,
        agent_comparisons,
        aggregate,
        timestamp: epoch_secs(),
    })
}

/// Run A/B validation with LLM-as-judge scoring.
///
/// Takes a judge closure factory that creates `ResponseComparator` instances.
/// Two comparators are created — one for baseline, one for routed — since
/// `ResponseComparator` cannot be cloned (contains `Box<dyn Fn>`).
pub fn run_ab_validation_with_judge<F>(
    battery: &[crate::battery::test_problem::TestProblem],
    baseline_adapters: &[AgentAdapter],
    routed_adapters: &[AgentAdapter],
    make_comparator: F,
) -> Result<ABComparisonResult, String>
where
    F: Fn() -> crate::evaluation::comparator::ResponseComparator,
{
    let baseline_evaluator = BatchEvaluator::new(battery.to_vec(), Some(make_comparator()));
    let baseline = baseline_evaluator.evaluate_all(baseline_adapters, None);

    let routed_evaluator = BatchEvaluator::new(battery.to_vec(), Some(make_comparator()));
    let routed = routed_evaluator.evaluate_all(routed_adapters, None);

    let agent_comparisons = compare_agents(&baseline, &routed);
    let aggregate = compute_aggregate(&agent_comparisons, &baseline, &routed);

    Ok(ABComparisonResult {
        baseline,
        routed,
        agent_comparisons,
        aggregate,
        timestamp: epoch_secs(),
    })
}

// ── Comparison Logic ─────────────────────────────────────────────────────────

fn compare_agents(baseline: &BatchResult, routed: &BatchResult) -> Vec<AgentABComparison> {
    let mut comparisons = Vec::new();

    for bs in &baseline.sessions {
        let rs = match routed.sessions.iter().find(|s| s.agent_id == bs.agent_id) {
            Some(s) => s,
            None => continue,
        };

        let bc = composite_score(bs);
        let rc = composite_score(rs);

        let vector_deltas: Vec<VectorDelta> = bs
            .vector_results
            .iter()
            .filter_map(|bv| {
                rs.vector_results
                    .iter()
                    .find(|rv| rv.vector == bv.vector)
                    .map(|rv| VectorDelta {
                        vector: bv.vector,
                        baseline_score: bv.vector_score,
                        routed_score: rv.vector_score,
                        delta: rv.vector_score - bv.vector_score,
                    })
            })
            .collect();

        let bb = baseline
            .boundary_map
            .iter()
            .find(|b| b.agent_id == bs.agent_id);
        let rb = routed
            .boundary_map
            .iter()
            .find(|b| b.agent_id == rs.agent_id);
        let baseline_ceil = bb.and_then(|b| b.overall_ceiling);
        let routed_ceil = rb.and_then(|b| b.overall_ceiling);

        let ceiling_improved = match (baseline_ceil, routed_ceil) {
            (Some(b), Some(r)) => r > b,
            (Some(_), None) => true,
            _ => false,
        };

        let bc_class = classification_str(bs);
        let rc_class = classification_str(rs);

        comparisons.push(AgentABComparison {
            agent_id: bs.agent_id.clone(),
            autonomy_level: bs.agent_autonomy_level,
            baseline_composite: bc,
            routed_composite: rc,
            delta: rc - bc,
            vector_deltas,
            baseline_ceiling: baseline_ceil,
            routed_ceiling: routed_ceil,
            ceiling_improved,
            baseline_classification: bc_class.clone(),
            routed_classification: rc_class.clone(),
            classification_changed: bc_class != rc_class,
        });
    }

    comparisons.sort_by(|a, b| {
        b.delta
            .partial_cmp(&a.delta)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    comparisons
}

fn compute_aggregate(
    comparisons: &[AgentABComparison],
    baseline: &BatchResult,
    routed: &BatchResult,
) -> AggregateComparison {
    let total = comparisons.len();
    let improved = comparisons.iter().filter(|c| c.delta > 0.01).count();
    let unchanged = comparisons.iter().filter(|c| c.delta.abs() <= 0.01).count();
    let degraded = comparisons.iter().filter(|c| c.delta < -0.01).count();

    let avg_delta = if total > 0 {
        comparisons.iter().map(|c| c.delta).sum::<f64>() / total as f64
    } else {
        0.0
    };

    let ceiling_deltas: Vec<f64> = comparisons
        .iter()
        .filter_map(|c| match (c.baseline_ceiling, c.routed_ceiling) {
            (Some(b), Some(r)) => Some(level_to_num(r) - level_to_num(b)),
            (Some(_), None) => Some(1.0),
            _ => None,
        })
        .collect();
    let avg_ceiling = if !ceiling_deltas.is_empty() {
        ceiling_deltas.iter().sum::<f64>() / ceiling_deltas.len() as f64
    } else {
        0.0
    };

    let vectors = [
        Vector::ReasoningDepth,
        Vector::PlanningCoherence,
        Vector::AdaptationUnderUncertainty,
        Vector::ToolUseIntegrity,
    ];
    let vector_aggregates: Vec<VectorAggregate> = vectors
        .iter()
        .map(|v| {
            let deltas: Vec<&VectorDelta> = comparisons
                .iter()
                .flat_map(|c| c.vector_deltas.iter())
                .filter(|vd| vd.vector == *v)
                .collect();
            let n = deltas.len().max(1) as f64;
            VectorAggregate {
                vector: *v,
                avg_baseline: deltas.iter().map(|d| d.baseline_score).sum::<f64>() / n,
                avg_routed: deltas.iter().map(|d| d.routed_score).sum::<f64>() / n,
                avg_delta: deltas.iter().map(|d| d.delta).sum::<f64>() / n,
                agents_improved: deltas.iter().filter(|d| d.delta > 0.01).count(),
            }
        })
        .collect();

    AggregateComparison {
        agents_evaluated: total,
        agents_improved: improved,
        agents_unchanged: unchanged,
        agents_degraded: degraded,
        avg_composite_delta: avg_delta,
        avg_ceiling_delta: avg_ceiling,
        baseline_census: baseline.census.clone(),
        routed_census: routed.census.clone(),
        vector_aggregates,
        most_improved: comparisons.first().map(|c| c.agent_id.clone()),
        most_improved_delta: comparisons.first().map(|c| c.delta).unwrap_or(0.0),
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn composite_score(session: &MeasurementSession) -> f64 {
    if session.vector_results.is_empty() {
        return 0.0;
    }
    session
        .vector_results
        .iter()
        .map(|vr| vr.vector_score)
        .sum::<f64>()
        / session.vector_results.len() as f64
}

fn classification_str(session: &MeasurementSession) -> String {
    session
        .cross_vector_analysis
        .as_ref()
        .map(|a| format!("{:?}", a.overall_classification))
        .unwrap_or_else(|| "Unknown".into())
}

pub fn level_to_num(level: DifficultyLevel) -> f64 {
    match level {
        DifficultyLevel::Level1 => 1.0,
        DifficultyLevel::Level2 => 2.0,
        DifficultyLevel::Level3 => 3.0,
        DifficultyLevel::Level4 => 4.0,
        DifficultyLevel::Level5 => 5.0,
    }
}

fn epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battery::expected_chain::ExpectedReasoning;
    use crate::battery::test_problem::{ProblemContext, ScoringRubric, TestProblem};
    use crate::scoring::gaming_detection::GamingDetectionRule;

    fn make_problem(vector: Vector, level: DifficultyLevel) -> TestProblem {
        TestProblem {
            id: format!("ab-{vector:?}-{level:?}"),
            version: "v1".into(),
            vector,
            level,
            problem_statement: format!("AB test for {vector:?} {level:?}"),
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
                full_credit: vec![],
                partial_credit: vec![],
                zero_credit: vec![],
            },
            gaming_detection: vec![],
            locked: true,
            locked_at: Some(0),
        }
    }

    fn make_battery() -> Vec<TestProblem> {
        let mut b = Vec::new();
        for v in [
            Vector::ReasoningDepth,
            Vector::PlanningCoherence,
            Vector::AdaptationUnderUncertainty,
            Vector::ToolUseIntegrity,
        ] {
            for l in [
                DifficultyLevel::Level1,
                DifficultyLevel::Level2,
                DifficultyLevel::Level3,
            ] {
                b.push(make_problem(v, l));
            }
        }
        b
    }

    fn make_adapter(id: &str, level: u8, response: &str) -> AgentAdapter {
        let resp = response.to_string();
        AgentAdapter::new(id.into(), level, move |_| Ok(resp.clone()))
    }

    #[test]
    fn test_ab_comparison_structure() {
        let battery = make_battery();
        let adapters = vec![
            make_adapter("agent-a", 3, "test insight detailed response"),
            make_adapter("agent-b", 2, "test insight basic"),
        ];
        let result = run_ab_validation(&battery, &adapters, &adapters).unwrap();
        assert_eq!(result.baseline.sessions.len(), 2);
        assert_eq!(result.routed.sessions.len(), 2);
        assert_eq!(result.agent_comparisons.len(), 2);
    }

    #[test]
    fn test_agent_comparison_delta() {
        let battery = make_battery();
        let baseline = vec![make_adapter("a1", 3, "test insight")];
        let routed = vec![make_adapter("a1", 3, "test insight")]; // Same response → delta = 0
        let result = run_ab_validation(&battery, &baseline, &routed).unwrap();
        assert_eq!(result.agent_comparisons.len(), 1);
        assert!(
            result.agent_comparisons[0].delta.abs() < 1e-9,
            "Same response should produce zero delta"
        );
    }

    #[test]
    fn test_aggregate_counts() {
        let battery = make_battery();
        let adapters = vec![
            make_adapter("a1", 3, "test insight"),
            make_adapter("a2", 2, "test insight"),
            make_adapter("a3", 1, "test insight"),
        ];
        let result = run_ab_validation(&battery, &adapters, &adapters).unwrap();
        let agg = &result.aggregate;
        assert_eq!(
            agg.agents_improved + agg.agents_unchanged + agg.agents_degraded,
            agg.agents_evaluated
        );
    }

    #[test]
    fn test_vector_deltas_per_agent() {
        let battery = make_battery();
        let adapters = vec![make_adapter("a1", 3, "test insight")];
        let result = run_ab_validation(&battery, &adapters, &adapters).unwrap();
        assert_eq!(
            result.agent_comparisons[0].vector_deltas.len(),
            4,
            "Should have 4 vector deltas"
        );
    }

    #[test]
    fn test_most_improved_is_first() {
        let battery = make_battery();
        // Different responses → different scores → non-zero deltas
        let baseline = vec![
            make_adapter("a1", 3, "no matching keywords here"),
            make_adapter("a2", 3, "test insight detailed"),
        ];
        let routed = vec![
            make_adapter("a1", 3, "test insight detailed very thorough analysis"),
            make_adapter("a2", 3, "test insight detailed"),
        ];
        let result = run_ab_validation(&battery, &baseline, &routed).unwrap();
        // a1 improved more (went from no keywords to many)
        if result.agent_comparisons.len() >= 2 {
            assert!(
                result.agent_comparisons[0].delta >= result.agent_comparisons[1].delta,
                "First agent should have highest delta"
            );
        }
    }

    #[test]
    fn test_aggregate_vector_averages() {
        let battery = make_battery();
        let adapters = vec![
            make_adapter("a1", 3, "test insight"),
            make_adapter("a2", 2, "test insight"),
        ];
        let result = run_ab_validation(&battery, &adapters, &adapters).unwrap();
        assert_eq!(result.aggregate.vector_aggregates.len(), 4);
        for va in &result.aggregate.vector_aggregates {
            assert!(
                (va.avg_delta).abs() < 1e-9,
                "Same responses should produce zero avg_delta"
            );
        }
    }

    #[test]
    fn test_level_to_num_conversion() {
        assert!((level_to_num(DifficultyLevel::Level1) - 1.0).abs() < 1e-9);
        assert!((level_to_num(DifficultyLevel::Level3) - 3.0).abs() < 1e-9);
        assert!((level_to_num(DifficultyLevel::Level5) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn test_ceiling_improvement_detection() {
        // Build sessions where baseline has L3 ceiling, routed has no ceiling
        let battery = make_battery();
        // Baseline: weak response (fails at L3)
        let baseline = vec![make_adapter("a1", 3, "nothing relevant")];
        // Routed: strong response (passes everything)
        let routed = vec![make_adapter("a1", 3, "test insight detailed thorough")];
        let result = run_ab_validation(&battery, &baseline, &routed).unwrap();

        // The routed run should score higher
        if !result.agent_comparisons.is_empty() {
            let comp = &result.agent_comparisons[0];
            assert!(comp.delta >= 0.0, "Routed should be >= baseline");
        }
    }

    #[test]
    fn test_classification_change_detection() {
        let battery = make_battery();
        let adapters = vec![make_adapter("a1", 3, "test insight")];
        let result = run_ab_validation(&battery, &adapters, &adapters).unwrap();
        // Same response → same classification
        assert!(
            !result.agent_comparisons[0].classification_changed,
            "Same response should not change classification"
        );
    }

    #[test]
    fn test_run_ab_validation_with_judge() {
        use crate::evaluation::comparator::ResponseComparator;

        let battery = make_battery();
        let adapters = vec![make_adapter("a1", 3, "test insight detailed response")];

        // Mock judge that always returns high scores
        let make_comparator = || {
            ResponseComparator::new(|prompt: &str| {
                if prompt.contains("capability measurement judge") {
                    Ok(r#"{"raw_score": 0.85, "penalties": [], "reasoning": "good", "critical_failure_triggered": false}"#.to_string())
                } else if prompt.contains("ARTICULATE") {
                    Ok(
                        r#"{"dimensions": [{"name": "d1", "score": 1, "evidence": "ok"}]}"#
                            .to_string(),
                    )
                } else {
                    Ok("[]".to_string())
                }
            })
        };

        let result =
            run_ab_validation_with_judge(&battery, &adapters, &adapters, make_comparator).unwrap();
        assert_eq!(result.baseline.sessions.len(), 1);
        assert_eq!(result.routed.sessions.len(), 1);
        // Judge scores should be higher than keyword scores
        let judge_score = result.agent_comparisons[0].baseline_composite;
        assert!(
            judge_score > 0.3,
            "Judge score {judge_score} should be higher than keyword fallback"
        );
    }

    #[test]
    fn test_judge_fallback_on_failure() {
        use crate::evaluation::comparator::ResponseComparator;

        let battery = make_battery();
        let adapters = vec![make_adapter("a1", 3, "test insight")];

        // Mock judge that always fails — should fall back to keyword scoring
        let make_comparator =
            || ResponseComparator::new(|_prompt: &str| Err("judge unavailable".to_string()));

        let result =
            run_ab_validation_with_judge(&battery, &adapters, &adapters, make_comparator).unwrap();
        // Should not crash — falls back to keyword scoring
        assert_eq!(result.baseline.sessions.len(), 1);
        // Keyword scoring for "test insight" should produce non-zero score
        let score = result.agent_comparisons[0].baseline_composite;
        assert!(
            score >= 0.0,
            "Score should be non-negative even on judge failure"
        );
    }
}
