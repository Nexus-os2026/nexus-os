//! Test problem definitions for the capability battery.

use serde::{Deserialize, Serialize};

use crate::battery::expected_chain::ExpectedReasoning;
use crate::framework::{DifficultyLevel, Vector};
use crate::scoring::gaming_detection::GamingDetectionRule;

/// A single test problem in the battery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestProblem {
    pub id: String,
    /// Immutable once locked.
    pub version: String,
    pub vector: Vector,
    pub level: DifficultyLevel,
    pub problem_statement: String,
    pub context: ProblemContext,
    pub expected_reasoning: ExpectedReasoning,
    pub scoring_rubric: ScoringRubric,
    pub gaming_detection: Vec<GamingDetectionRule>,
    pub locked: bool,
    pub locked_at: Option<u64>,
}

/// Context provided to the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemContext {
    pub initial_state: serde_json::Value,
    /// For adaptation problems: new information mid-problem.
    pub mid_problem_updates: Vec<MidProblemUpdate>,
    /// For tool use problems: available mock tools.
    pub available_tools: Vec<MockTool>,
}

/// New information that arrives during the problem (Vector 3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidProblemUpdate {
    pub arrives_after_step: Option<u32>,
    pub information: String,
    pub source_reliability: SourceReliability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SourceReliability {
    Verified,
    Unverified,
    Suspicious,
    Adversarial,
}

/// Mock tool for tool use integrity testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub mock_responses: Vec<MockResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockResponse {
    pub input_pattern: serde_json::Value,
    pub output: serde_json::Value,
    /// Does the output contain signals the agent should not ignore?
    pub is_ambiguous: bool,
}

/// Rubric for scoring an agent response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringRubric {
    /// Criteria that earn full credit if met.
    pub full_credit: Vec<String>,
    /// Criteria that earn partial credit.
    pub partial_credit: Vec<String>,
    /// Criteria that result in zero score.
    pub zero_credit: Vec<String>,
}

/// Load the test battery from a JSON file path.
pub fn load_battery(path: &str) -> Result<Vec<TestProblem>, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read battery: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse battery: {e}"))
}
