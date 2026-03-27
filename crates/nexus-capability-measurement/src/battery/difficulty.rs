//! Difficulty level descriptions and calibration.

use crate::framework::{DifficultyLevel, Vector};

/// Human-readable description of what a difficulty level means for a given vector.
pub fn difficulty_description(vector: Vector, level: DifficultyLevel) -> &'static str {
    match (vector, level) {
        // Reasoning Depth
        (Vector::ReasoningDepth, DifficultyLevel::Level1) => {
            "Single constraint: identify one cause-effect relationship"
        }
        (Vector::ReasoningDepth, DifficultyLevel::Level2) => {
            "Two constraints with conflict: distinguish correlation from causation"
        }
        (Vector::ReasoningDepth, DifficultyLevel::Level3) => {
            "Three cascading constraints: trace multi-hop causal chains"
        }
        (Vector::ReasoningDepth, DifficultyLevel::Level4) => {
            "Four constraints with hidden conflict: identify unstated assumptions"
        }
        (Vector::ReasoningDepth, DifficultyLevel::Level5) => {
            "Underspecified: recognize problem cannot be solved with given information"
        }

        // Planning Coherence
        (Vector::PlanningCoherence, DifficultyLevel::Level1) => {
            "Linear plan: sequence 3-4 steps with simple dependencies"
        }
        (Vector::PlanningCoherence, DifficultyLevel::Level2) => {
            "Branching plan: handle two parallel tracks with merge point"
        }
        (Vector::PlanningCoherence, DifficultyLevel::Level3) => {
            "Multi-phase with rollback: plan must include failure recovery"
        }
        (Vector::PlanningCoherence, DifficultyLevel::Level4) => {
            "Adversarial ordering: steps have non-obvious dependencies"
        }
        (Vector::PlanningCoherence, DifficultyLevel::Level5) => {
            "Underspecified goal: must clarify objectives before planning"
        }

        // Adaptation Under Uncertainty
        (Vector::AdaptationUnderUncertainty, DifficultyLevel::Level1) => {
            "Single invalidation: one assumption changes, revise plan"
        }
        (Vector::AdaptationUnderUncertainty, DifficultyLevel::Level2) => {
            "Cascading failure: initial change propagates through plan"
        }
        (Vector::AdaptationUnderUncertainty, DifficultyLevel::Level3) => {
            "Conflicting information: sources disagree, assess reliability"
        }
        (Vector::AdaptationUnderUncertainty, DifficultyLevel::Level4) => {
            "Mid-execution pivot: fundamental goal changes during execution"
        }
        (Vector::AdaptationUnderUncertainty, DifficultyLevel::Level5) => {
            "Adversarial environment: information is deliberately misleading"
        }

        // Tool Use Integrity
        (Vector::ToolUseIntegrity, DifficultyLevel::Level1) => {
            "Single tool: select and use one tool correctly"
        }
        (Vector::ToolUseIntegrity, DifficultyLevel::Level2) => {
            "Tool sequencing: chain two tools, use output of first as input to second"
        }
        (Vector::ToolUseIntegrity, DifficultyLevel::Level3) => {
            "Output ambiguity: tool returns ambiguous data, agent must recognize limits"
        }
        (Vector::ToolUseIntegrity, DifficultyLevel::Level4) => {
            "Tool limitation: no available tool fully solves the problem"
        }
        (Vector::ToolUseIntegrity, DifficultyLevel::Level5) => {
            "No tool solves it: agent must state the problem is unsolvable with available tools"
        }
    }
}
