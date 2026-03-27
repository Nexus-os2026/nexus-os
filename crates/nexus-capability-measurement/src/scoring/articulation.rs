//! Articulation scoring — separate track from primary score.
//!
//! Tests whether the agent can EXPLAIN its reasoning, not just produce correct
//! output. Each dimension is scored binary (0 or 1), max 3 per vector.

use serde::{Deserialize, Serialize};

use crate::framework::Vector;

/// Articulation score for a single level result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticulationScore {
    pub vector: Vector,
    pub dimensions: Vec<ArticulationDimension>,
    /// Sum of dimension scores, max 3.0.
    pub total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticulationDimension {
    pub name: String,
    /// 0.0 or 1.0 (binary).
    pub score: f64,
    /// What in the agent's response supports this score.
    pub evidence: String,
}

/// Vector-specific articulation dimension names.
pub fn articulation_dimensions(vector: Vector) -> Vec<String> {
    match vector {
        Vector::ReasoningDepth => vec![
            "semantic_correctness".into(),
            "completeness".into(),
            "precision".into(),
        ],
        Vector::PlanningCoherence => vec![
            "dependency_correctness".into(),
            "completeness".into(),
            "ordering_justification".into(),
        ],
        Vector::AdaptationUnderUncertainty => vec![
            "revision_precision".into(),
            "cascade_awareness".into(),
            "epistemic_honesty".into(),
        ],
        Vector::ToolUseIntegrity => vec![
            "selection_justification".into(),
            "output_fidelity".into(),
            "limitation_transparency".into(),
        ],
    }
}

/// Create an articulation score with all dimensions at zero (to be filled by scorer).
pub fn empty_articulation(vector: Vector) -> ArticulationScore {
    let dims = articulation_dimensions(vector)
        .into_iter()
        .map(|name| ArticulationDimension {
            name,
            score: 0.0,
            evidence: String::new(),
        })
        .collect();
    ArticulationScore {
        vector,
        dimensions: dims,
        total: 0.0,
    }
}

/// Compute articulation total from dimension scores.
pub fn compute_articulation(
    vector: Vector,
    dimension_scores: &[(String, f64, String)],
) -> ArticulationScore {
    let dimensions: Vec<ArticulationDimension> = dimension_scores
        .iter()
        .map(|(name, score, evidence)| ArticulationDimension {
            name: name.clone(),
            score: if *score >= 0.5 { 1.0 } else { 0.0 },
            evidence: evidence.clone(),
        })
        .collect();
    let total = dimensions.iter().map(|d| d.score).sum();
    ArticulationScore {
        vector,
        dimensions,
        total,
    }
}
