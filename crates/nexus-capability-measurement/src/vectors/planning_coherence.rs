//! Vector 2: Planning Coherence
//!
//! Measures ability to construct plans with correct dependencies, rollback paths,
//! halt conditions, and parallel execution groups.

use crate::framework::DifficultyLevel;

/// Description of what planning coherence looks like at each level.
pub fn level_description(level: DifficultyLevel) -> &'static str {
    match level {
        DifficultyLevel::Level1 => "Produce a linear 3-4 step plan with simple ordering.",
        DifficultyLevel::Level2 => "Handle two parallel tracks that merge at a single point.",
        DifficultyLevel::Level3 => "Include failure recovery / rollback in a multi-phase plan.",
        DifficultyLevel::Level4 => {
            "Identify non-obvious step ordering with adversarial dependencies."
        }
        DifficultyLevel::Level5 => {
            "Recognize the goal is underspecified and request clarification before planning."
        }
    }
}

/// Check if a plan has explicit dependencies between steps.
pub fn has_explicit_dependencies(response: &str) -> bool {
    let dependency_markers = [
        "depends on",
        "requires",
        "after step",
        "before step",
        "prerequisite",
        "must complete",
        "only if",
    ];
    let resp_lower = response.to_lowercase();
    dependency_markers.iter().any(|m| resp_lower.contains(m))
}

/// Check if a plan includes rollback or failure handling.
pub fn has_rollback_handling(response: &str) -> bool {
    let markers = [
        "rollback",
        "revert",
        "undo",
        "if fails",
        "fallback",
        "recovery",
        "on failure",
    ];
    let resp_lower = response.to_lowercase();
    markers.iter().any(|m| resp_lower.contains(m))
}
