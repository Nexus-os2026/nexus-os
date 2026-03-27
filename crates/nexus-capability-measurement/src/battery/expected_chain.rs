//! Expert-documented expected reasoning chains, locked before testing.

use serde::{Deserialize, Serialize};

/// Expert-documented expected reasoning for a test problem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedReasoning {
    /// The expected causal chain (Vector 1).
    pub causal_chain: Vec<CausalLink>,
    /// The expected plan with dependencies (Vector 2).
    pub expected_plan: Option<ExpectedPlan>,
    /// The expected adaptation path (Vector 3).
    pub expected_adaptation: Option<ExpectedAdaptation>,
    /// The expected tool use sequence (Vector 4).
    pub expected_tool_use: Option<ExpectedToolUse>,
    /// What the agent MUST identify for full credit.
    pub required_insights: Vec<String>,
    /// What scores zero if the agent does it.
    pub critical_failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalLink {
    pub from: String,
    pub to: String,
    pub relationship: String,
    /// True if agents commonly confuse this with causation.
    pub is_correlation_trap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedPlan {
    pub steps: Vec<PlanStep>,
    /// (prerequisite_step, dependent_step) pairs.
    pub dependency_graph: Vec<(usize, usize)>,
    /// Steps that can execute in parallel.
    pub parallel_groups: Vec<Vec<usize>>,
    pub rollback_paths: Vec<RollbackPath>,
    pub halt_conditions: Vec<HaltCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub index: usize,
    pub description: String,
    pub preconditions: Vec<String>,
    pub postconditions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackPath {
    pub trigger_step: usize,
    pub rollback_steps: Vec<usize>,
    pub safety_justification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HaltCondition {
    pub at_step: usize,
    pub condition: String,
    pub consequence_if_missed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedAdaptation {
    pub what_changed: Vec<String>,
    pub what_invalidated: Vec<String>,
    pub what_preserved: Vec<String>,
    pub minimum_revision: Vec<String>,
    pub cascade_depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedToolUse {
    pub expected_sequence: Vec<ExpectedToolCall>,
    /// What the agent should recognize it CANNOT do.
    pub expected_limitations: Vec<String>,
    /// Information the agent might hallucinate.
    pub fabrication_traps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedToolCall {
    pub tool_name: String,
    pub expected_input: serde_json::Value,
    pub expected_output: serde_json::Value,
    /// Downstream steps that must reference this output.
    pub must_use_output_in: Vec<String>,
    /// Why this tool, not another.
    pub selection_justification: String,
}
