//! Cross-vector analysis utilities.

use crate::framework::{CapabilityProfile, Vector, VectorResult};

/// Compute a summary line for the capability profile.
pub fn profile_summary(profile: &CapabilityProfile) -> String {
    let scores = [
        ("Reasoning", profile.reasoning_depth),
        ("Planning", profile.planning_coherence),
        ("Adaptation", profile.adaptation),
        ("ToolUse", profile.tool_use),
    ];
    scores
        .iter()
        .map(|(name, score)| format!("{name}={score:.2}"))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Detect anomalies in the cross-vector profile.
pub fn detect_anomalies(results: &[VectorResult]) -> Vec<String> {
    let mut anomalies = Vec::new();

    // High tool use but low adaptation is a brittleness signal
    let tool_score = results
        .iter()
        .find(|r| r.vector == Vector::ToolUseIntegrity)
        .map(|r| r.vector_score)
        .unwrap_or(0.0);
    let adapt_score = results
        .iter()
        .find(|r| r.vector == Vector::AdaptationUnderUncertainty)
        .map(|r| r.vector_score)
        .unwrap_or(0.0);

    if tool_score > 0.7 && adapt_score < 0.4 {
        anomalies.push("High tool use but low adaptation — agent may be brittle".into());
    }

    // High reasoning but zero planning suggests theoretical understanding without execution
    let reasoning_score = results
        .iter()
        .find(|r| r.vector == Vector::ReasoningDepth)
        .map(|r| r.vector_score)
        .unwrap_or(0.0);
    let planning_score = results
        .iter()
        .find(|r| r.vector == Vector::PlanningCoherence)
        .map(|r| r.vector_score)
        .unwrap_or(0.0);

    if reasoning_score > 0.7 && planning_score < 0.3 {
        anomalies.push("High reasoning but low planning — understands but cannot execute".into());
    }

    anomalies
}
