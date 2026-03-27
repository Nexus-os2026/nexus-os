//! Vector 1: Reasoning Depth
//!
//! Measures ability to trace multi-hop causal chains, distinguish correlation
//! from causation, identify hidden constraints, and recognize underspecification.

use crate::framework::DifficultyLevel;

/// Description of what reasoning depth looks like at each level.
pub fn level_description(level: DifficultyLevel) -> &'static str {
    match level {
        DifficultyLevel::Level1 => "Identify a single cause-effect relationship.",
        DifficultyLevel::Level2 => "Distinguish correlation from causation across two factors.",
        DifficultyLevel::Level3 => "Trace a three-hop causal chain with cascading effects.",
        DifficultyLevel::Level4 => {
            "Identify hidden constraints that invalidate surface-level reasoning."
        }
        DifficultyLevel::Level5 => {
            "Recognize that the problem is underspecified and cannot be answered."
        }
    }
}

/// Check if a response demonstrates causal reasoning (not just correlation).
pub fn has_causal_language(response: &str) -> bool {
    let causal_markers = [
        "because",
        "causes",
        "leads to",
        "results in",
        "therefore",
        "consequently",
        "due to",
        "as a result",
    ];
    let resp_lower = response.to_lowercase();
    causal_markers.iter().any(|m| resp_lower.contains(m))
}

/// Check if a response avoids the correlation-as-causation trap.
pub fn avoids_correlation_trap(response: &str) -> bool {
    let hedging = [
        "correlation",
        "does not necessarily",
        "not sufficient to conclude",
        "may not cause",
        "coincidence",
    ];
    let resp_lower = response.to_lowercase();
    hedging.iter().any(|m| resp_lower.contains(m))
}
