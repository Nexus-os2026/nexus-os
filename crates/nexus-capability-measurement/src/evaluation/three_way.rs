//! Three-way comparison: Run 1 (pre-bugfix) vs Run 2 baseline vs Run 2 routed.

use serde::{Deserialize, Serialize};

use crate::evaluation::batch::ClassificationCensus;
use crate::evaluation::validation_run::ValidationRunOutput;
use crate::framework::MeasurementSession;

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreeWayComparison {
    pub run1_baseline: RunSummary,
    pub run2_baseline: RunSummary,
    pub run2_routed: RunSummary,
    pub bugfix_delta: DeltaSummary,
    pub routing_delta: DeltaSummary,
    pub total_delta: DeltaSummary,
    pub agent_details: Vec<AgentThreeWay>,
    pub vector_details: Vec<VectorThreeWay>,
    pub narrative: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    pub label: String,
    pub avg_composite: f64,
    pub agents_evaluated: usize,
    pub census: ClassificationCensus,
    pub calibration_inversions: usize,
    pub gaming_flags: usize,
    pub per_vector: Vec<(String, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaSummary {
    pub label: String,
    pub avg_delta: f64,
    pub pct_improvement: f64,
    pub agents_improved: usize,
    pub agents_unchanged: usize,
    pub agents_degraded: usize,
    pub per_vector: Vec<(String, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentThreeWay {
    pub agent_id: String,
    pub autonomy_level: u8,
    pub run1_baseline: f64,
    pub run2_baseline: f64,
    pub run2_routed: f64,
    pub bugfix_delta: f64,
    pub routing_delta: f64,
    pub total_delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorThreeWay {
    pub vector: String,
    pub run1_avg: f64,
    pub run2_baseline_avg: f64,
    pub run2_routed_avg: f64,
    pub bugfix_delta: f64,
    pub routing_delta: f64,
    pub total_delta: f64,
}

// ── Builder ──────────────────────────────────────────────────────────────────

/// Build a three-way comparison from two validation runs.
pub fn build_three_way(
    run1: &ValidationRunOutput,
    run2: &ValidationRunOutput,
) -> ThreeWayComparison {
    let r1b = extract_summary(
        &run1.ab_result.baseline.sessions,
        "Run 1: Pre-Bug-Fix",
        &run1.ab_result.baseline,
    );
    let r2b = extract_summary(
        &run2.ab_result.baseline.sessions,
        "Run 2: Post-Bug-Fix",
        &run2.ab_result.baseline,
    );
    let r2r = extract_summary(
        &run2.ab_result.routed.sessions,
        "Run 2 + Routing",
        &run2.ab_result.routed,
    );

    let agent_details = build_agent_details(run1, run2);

    let bugfix_delta = compute_delta("Bug Fix", &r1b, &r2b, &agent_details, |a| a.bugfix_delta);
    let routing_delta = compute_delta("Routing", &r2b, &r2r, &agent_details, |a| a.routing_delta);
    let total_delta = compute_delta("Total", &r1b, &r2r, &agent_details, |a| a.total_delta);

    let vector_details = build_vector_details(&r1b, &r2b, &r2r);

    let narrative = format!(
        "Bug fixes improved agent capability by {:.1}%. \
         Predictive routing added {:.1}% on top. \
         Combined improvement: {:.1}% across all {} agents.",
        bugfix_delta.pct_improvement,
        routing_delta.pct_improvement,
        total_delta.pct_improvement,
        r1b.agents_evaluated,
    );

    ThreeWayComparison {
        run1_baseline: r1b,
        run2_baseline: r2b,
        run2_routed: r2r,
        bugfix_delta,
        routing_delta,
        total_delta,
        agent_details,
        vector_details,
        narrative,
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn session_composite(s: &MeasurementSession) -> f64 {
    if s.vector_results.is_empty() {
        return 0.0;
    }
    s.vector_results
        .iter()
        .map(|vr| vr.vector_score)
        .sum::<f64>()
        / s.vector_results.len() as f64
}

fn vector_avg(sessions: &[MeasurementSession], vector_name: &str) -> f64 {
    let scores: Vec<f64> = sessions
        .iter()
        .flat_map(|s| s.vector_results.iter())
        .filter(|vr| format!("{:?}", vr.vector) == vector_name)
        .map(|vr| vr.vector_score)
        .collect();
    if scores.is_empty() {
        0.0
    } else {
        scores.iter().sum::<f64>() / scores.len() as f64
    }
}

fn extract_summary(
    sessions: &[MeasurementSession],
    label: &str,
    batch: &crate::evaluation::batch::BatchResult,
) -> RunSummary {
    let avg = if sessions.is_empty() {
        0.0
    } else {
        sessions.iter().map(session_composite).sum::<f64>() / sessions.len() as f64
    };

    RunSummary {
        label: label.into(),
        avg_composite: avg,
        agents_evaluated: sessions.len(),
        census: batch.census.clone(),
        calibration_inversions: batch.calibration.inversions.len(),
        gaming_flags: batch.gaming_report.total_flags,
        per_vector: vec![
            (
                "ReasoningDepth".into(),
                vector_avg(sessions, "ReasoningDepth"),
            ),
            (
                "PlanningCoherence".into(),
                vector_avg(sessions, "PlanningCoherence"),
            ),
            (
                "Adaptation".into(),
                vector_avg(sessions, "AdaptationUnderUncertainty"),
            ),
            (
                "ToolUseIntegrity".into(),
                vector_avg(sessions, "ToolUseIntegrity"),
            ),
        ],
    }
}

fn compute_delta(
    label: &str,
    from: &RunSummary,
    to: &RunSummary,
    agents: &[AgentThreeWay],
    delta_fn: impl Fn(&AgentThreeWay) -> f64,
) -> DeltaSummary {
    let avg_d = to.avg_composite - from.avg_composite;
    let pct = if from.avg_composite.abs() > 1e-9 {
        (avg_d / from.avg_composite) * 100.0
    } else {
        0.0
    };

    let improved = agents.iter().filter(|a| delta_fn(a) > 0.001).count();
    let degraded = agents.iter().filter(|a| delta_fn(a) < -0.001).count();
    let unchanged = agents.len() - improved - degraded;

    let per_vector: Vec<(String, f64)> = from
        .per_vector
        .iter()
        .zip(to.per_vector.iter())
        .map(|((name, fv), (_, tv))| (name.clone(), tv - fv))
        .collect();

    DeltaSummary {
        label: label.into(),
        avg_delta: avg_d,
        pct_improvement: pct,
        agents_improved: improved,
        agents_unchanged: unchanged,
        agents_degraded: degraded,
        per_vector,
    }
}

fn build_agent_details(
    run1: &ValidationRunOutput,
    run2: &ValidationRunOutput,
) -> Vec<AgentThreeWay> {
    let mut details = Vec::new();

    for r1s in &run1.ab_result.baseline.sessions {
        let r2b = run2
            .ab_result
            .baseline
            .sessions
            .iter()
            .find(|s| s.agent_id == r1s.agent_id);
        let r2r = run2
            .ab_result
            .routed
            .sessions
            .iter()
            .find(|s| s.agent_id == r1s.agent_id);

        let r1_score = session_composite(r1s);
        let r2b_score = r2b.map(session_composite).unwrap_or(0.0);
        let r2r_score = r2r.map(session_composite).unwrap_or(0.0);

        details.push(AgentThreeWay {
            agent_id: r1s.agent_id.clone(),
            autonomy_level: r1s.agent_autonomy_level,
            run1_baseline: r1_score,
            run2_baseline: r2b_score,
            run2_routed: r2r_score,
            bugfix_delta: r2b_score - r1_score,
            routing_delta: r2r_score - r2b_score,
            total_delta: r2r_score - r1_score,
        });
    }

    details.sort_by(|a, b| {
        b.total_delta
            .partial_cmp(&a.total_delta)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    details
}

fn build_vector_details(
    r1: &RunSummary,
    r2b: &RunSummary,
    r2r: &RunSummary,
) -> Vec<VectorThreeWay> {
    r1.per_vector
        .iter()
        .zip(r2b.per_vector.iter())
        .zip(r2r.per_vector.iter())
        .map(|(((name, v1), (_, v2b)), (_, v2r))| VectorThreeWay {
            vector: name.clone(),
            run1_avg: *v1,
            run2_baseline_avg: *v2b,
            run2_routed_avg: *v2r,
            bugfix_delta: v2b - v1,
            routing_delta: v2r - v2b,
            total_delta: v2r - v1,
        })
        .collect()
}

/// Write a markdown report.
pub fn write_report(comparison: &ThreeWayComparison, path: &std::path::Path) -> Result<(), String> {
    let mut md = String::new();
    md.push_str("# Nexus OS — Three-Way Capability Comparison\n\n");
    md.push_str(&format!("## Summary\n\n{}\n\n", comparison.narrative));

    md.push_str("## Scores\n\n");
    md.push_str("| Vector | Run 1 (Pre-Fix) | Run 2 (Post-Fix) | Run 2 + Routing | Bug Fix Δ | Routing Δ | Total Δ |\n");
    md.push_str("|--------|-----------------|------------------|-----------------|-----------|-----------|--------|\n");
    for v in &comparison.vector_details {
        md.push_str(&format!(
            "| {} | {:.3} | {:.3} | {:.3} | {:+.3} | {:+.3} | {:+.3} |\n",
            v.vector,
            v.run1_avg,
            v.run2_baseline_avg,
            v.run2_routed_avg,
            v.bugfix_delta,
            v.routing_delta,
            v.total_delta,
        ));
    }
    md.push_str(&format!(
        "| **Composite** | **{:.3}** | **{:.3}** | **{:.3}** | **{:+.3}** | **{:+.3}** | **{:+.3}** |\n\n",
        comparison.run1_baseline.avg_composite,
        comparison.run2_baseline.avg_composite,
        comparison.run2_routed.avg_composite,
        comparison.bugfix_delta.avg_delta,
        comparison.routing_delta.avg_delta,
        comparison.total_delta.avg_delta,
    ));

    md.push_str("## Agent Details (Top 20 by Total Δ)\n\n");
    md.push_str("| Agent | Level | Run 1 | Run 2 | Routed | Fix Δ | Route Δ | Total Δ |\n");
    md.push_str("|-------|-------|-------|-------|--------|-------|---------|--------|\n");
    for a in comparison.agent_details.iter().take(20) {
        md.push_str(&format!(
            "| {} | L{} | {:.3} | {:.3} | {:.3} | {:+.3} | {:+.3} | {:+.3} |\n",
            a.agent_id,
            a.autonomy_level,
            a.run1_baseline,
            a.run2_baseline,
            a.run2_routed,
            a.bugfix_delta,
            a.routing_delta,
            a.total_delta,
        ));
    }

    md.push_str("\n## Classification Census\n\n");
    md.push_str("| Category | Run 1 | Run 2 | Run 2 + Routing |\n");
    md.push_str("|----------|-------|-------|----------------|\n");
    let c1 = &comparison.run1_baseline.census;
    let c2 = &comparison.run2_baseline.census;
    let c3 = &comparison.run2_routed.census;
    for (label, v1, v2, v3) in [
        ("Balanced", c1.balanced, c2.balanced, c3.balanced),
        (
            "TheoreticalReasoner",
            c1.theoretical_reasoner,
            c2.theoretical_reasoner,
            c3.theoretical_reasoner,
        ),
        (
            "ProceduralExecutor",
            c1.procedural_executor,
            c2.procedural_executor,
            c3.procedural_executor,
        ),
        (
            "RigidToolUser",
            c1.rigid_tool_user,
            c2.rigid_tool_user,
            c3.rigid_tool_user,
        ),
        (
            "PatternMatching",
            c1.pattern_matching,
            c2.pattern_matching,
            c3.pattern_matching,
        ),
        ("Anomalous", c1.anomalous, c2.anomalous, c3.anomalous),
    ] {
        md.push_str(&format!("| {} | {} | {} | {} |\n", label, v1, v2, v3));
    }

    std::fs::write(path, md).map_err(|e| format!("write: {e}"))
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battery::expected_chain::ExpectedReasoning;
    use crate::battery::test_problem::{ProblemContext, ScoringRubric, TestProblem};
    use crate::evaluation::ab_validation::run_ab_validation;
    use crate::evaluation::agent_adapter::AgentAdapter;
    use crate::framework::{DifficultyLevel, Vector};

    fn make_problem(v: Vector, l: DifficultyLevel) -> TestProblem {
        TestProblem {
            id: format!("tw-{v:?}-{l:?}"),
            version: "v1".into(),
            vector: v,
            level: l,
            problem_statement: format!("Three-way test {v:?} {l:?}"),
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

    fn mini_battery() -> Vec<TestProblem> {
        [
            Vector::ReasoningDepth,
            Vector::PlanningCoherence,
            Vector::AdaptationUnderUncertainty,
            Vector::ToolUseIntegrity,
        ]
        .into_iter()
        .map(|v| make_problem(v, DifficultyLevel::Level1))
        .collect()
    }

    fn make_run(label: &str, response: &str) -> ValidationRunOutput {
        let battery = mini_battery();
        let resp = response.to_string();
        let adapters = vec![
            AgentAdapter::new("agent-a".into(), 3, {
                let r = resp.clone();
                move |_| Ok(r.clone())
            }),
            AgentAdapter::new("agent-b".into(), 2, move |_| Ok(resp.clone())),
        ];
        let ab = run_ab_validation(&battery, &adapters, &adapters).unwrap();
        ValidationRunOutput {
            run_label: label.into(),
            config: crate::evaluation::validation_run::ValidationRunConfig::default(),
            ab_result: ab,
            agents_discovered: 2,
            agents_evaluated: 2,
            errors: vec![],
            api_calls: crate::evaluation::validation_run::ApiCallSummary::default(),
            total_duration_secs: 1,
            started_at: 0,
            completed_at: 1,
        }
    }

    #[test]
    fn test_three_way_comparison_structure() {
        let run1 = make_run("run1", "test insight basic");
        let run2 = make_run("run2", "test insight detailed");
        let cmp = build_three_way(&run1, &run2);
        assert_eq!(cmp.run1_baseline.agents_evaluated, 2);
        assert_eq!(cmp.run2_baseline.agents_evaluated, 2);
        assert_eq!(cmp.agent_details.len(), 2);
        assert_eq!(cmp.vector_details.len(), 4);
        assert!(!cmp.narrative.is_empty());
    }

    #[test]
    fn test_bugfix_delta_calculation() {
        let run1 = make_run("run1", "no keywords");
        let run2 = make_run("run2", "test insight detailed");
        let cmp = build_three_way(&run1, &run2);
        // run2 has more keyword matches → higher baseline → positive bugfix delta
        assert!(
            cmp.bugfix_delta.avg_delta >= 0.0,
            "Post-bugfix should be >= pre-bugfix"
        );
    }

    #[test]
    fn test_routing_delta_calculation() {
        let run1 = make_run("run1", "test insight");
        let run2 = make_run("run2", "test insight");
        let cmp = build_three_way(&run1, &run2);
        // Same response for baseline and routed → routing delta ~0
        assert!(cmp.routing_delta.avg_delta.abs() < 0.01);
    }

    #[test]
    fn test_total_delta_is_sum() {
        let run1 = make_run("run1", "no match");
        let run2 = make_run("run2", "test insight detailed");
        let cmp = build_three_way(&run1, &run2);
        let expected = cmp.bugfix_delta.avg_delta + cmp.routing_delta.avg_delta;
        assert!(
            (cmp.total_delta.avg_delta - expected).abs() < 0.01,
            "Total ({:.4}) should ≈ bugfix ({:.4}) + routing ({:.4})",
            cmp.total_delta.avg_delta,
            cmp.bugfix_delta.avg_delta,
            cmp.routing_delta.avg_delta
        );
    }

    #[test]
    fn test_narrative_generation() {
        let run1 = make_run("run1", "test insight");
        let run2 = make_run("run2", "test insight");
        let cmp = build_three_way(&run1, &run2);
        assert!(cmp.narrative.contains('%'));
        assert!(cmp.narrative.contains("agents"));
    }

    #[test]
    fn test_agent_details_sorted_by_total_delta() {
        let run1 = make_run("run1", "test insight");
        let run2 = make_run("run2", "test insight");
        let cmp = build_three_way(&run1, &run2);
        for w in cmp.agent_details.windows(2) {
            assert!(w[0].total_delta >= w[1].total_delta);
        }
    }

    #[test]
    fn test_vector_details_all_four_vectors() {
        let run1 = make_run("run1", "test insight");
        let run2 = make_run("run2", "test insight");
        let cmp = build_three_way(&run1, &run2);
        assert_eq!(cmp.vector_details.len(), 4);
    }
}
