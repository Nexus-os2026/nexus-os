//! Core types for outcome evaluation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Outcome Specification ───────────────────────────────────────────────

/// Specification of what a successful outcome looks like.
/// Created BEFORE the agent runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeSpec {
    pub id: Uuid,
    pub task_id: String,
    pub agent_id: String,
    pub goal_description: String,
    pub criteria: Vec<SuccessCriterion>,
    pub constraints: Vec<Constraint>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

/// A single success criterion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessCriterion {
    pub id: Uuid,
    pub description: String,
    pub evaluator: CriterionEvaluator,
    pub required: bool,
    pub weight: f32,
}

/// How to evaluate whether a criterion is met.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CriterionEvaluator {
    ContainsKeywords {
        keywords: Vec<String>,
        match_mode: MatchMode,
    },
    MatchesPattern {
        pattern: String,
    },
    LlmJudge {
        judge_prompt: String,
    },
    FileExists {
        path: String,
        content_contains: Option<Vec<String>>,
    },
    ApiCallMade {
        url_pattern: String,
        expected_status: Option<u16>,
    },
    NumericThreshold {
        field: String,
        operator: ComparisonOp,
        threshold: f64,
    },
    ValidStructure {
        schema: serde_json::Value,
    },
    HumanReview {
        review_instructions: String,
    },
    Custom {
        evaluator_name: String,
        config: serde_json::Value,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MatchMode {
    All,
    Any,
    AtLeast(usize),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComparisonOp {
    GreaterThan,
    GreaterOrEqual,
    LessThan,
    LessOrEqual,
    Equal,
    NotEqual,
}

// ── Constraints ─────────────────────────────────────────────────────────

/// A constraint the agent must NOT violate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    pub id: Uuid,
    pub description: String,
    pub evaluator: ConstraintEvaluator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintEvaluator {
    ForbiddenKeywords {
        keywords: Vec<String>,
    },
    ForbiddenCapabilities {
        capabilities: Vec<String>,
    },
    TimeLimit {
        max_seconds: u64,
    },
    FuelLimit {
        max_fuel: f64,
    },
    ForbiddenPaths {
        paths: Vec<String>,
    },
    Custom {
        evaluator_name: String,
        config: serde_json::Value,
    },
}

// ── Results ─────────────────────────────────────────────────────────────

/// Result of evaluating a single criterion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionResult {
    pub criterion_id: Uuid,
    pub criterion_description: String,
    pub passed: bool,
    pub score: f32,
    pub evidence: String,
    pub evaluated_at: DateTime<Utc>,
}

/// Result of evaluating a single constraint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintResult {
    pub constraint_id: Uuid,
    pub constraint_description: String,
    pub violated: bool,
    pub evidence: String,
    pub evaluated_at: DateTime<Utc>,
}

/// The overall outcome verdict.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OutcomeVerdict {
    Success,
    PartialSuccess,
    Failure,
    PendingReview,
    Inconclusive,
}

impl std::fmt::Display for OutcomeVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "SUCCESS"),
            Self::PartialSuccess => write!(f, "PARTIAL_SUCCESS"),
            Self::Failure => write!(f, "FAILURE"),
            Self::PendingReview => write!(f, "PENDING_REVIEW"),
            Self::Inconclusive => write!(f, "INCONCLUSIVE"),
        }
    }
}

/// Complete outcome assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeAssessment {
    pub id: Uuid,
    pub spec_id: Uuid,
    pub task_id: String,
    pub agent_id: String,
    pub agent_output: String,
    pub criteria_results: Vec<CriterionResult>,
    pub constraint_results: Vec<ConstraintResult>,
    pub verdict: OutcomeVerdict,
    pub score: f32,
    pub summary: String,
    pub evaluated_at: DateTime<Utc>,
    pub evaluation_duration_ms: u64,
    pub audit_hash: String,
}

/// Compliance-ready outcome artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeArtifact {
    pub assessment: OutcomeAssessment,
    pub spec: OutcomeSpec,
    pub action_log: Vec<serde_json::Value>,
    pub memory_entries_created: usize,
    pub rollbacks_performed: usize,
    pub governance_events: Vec<serde_json::Value>,
    pub generated_at: DateTime<Utc>,
    pub artifact_hash: String,
}

// ── Errors ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OutcomeError {
    #[error("outcome spec not found: {0}")]
    SpecNotFound(Uuid),
    #[error("evaluation failed for criterion {criterion_id}: {reason}")]
    EvaluationFailed { criterion_id: Uuid, reason: String },
    #[error("invalid regex pattern: {0}")]
    InvalidPattern(String),
    #[error("LLM judge unavailable: {0}")]
    JudgeUnavailable(String),
    #[error("human review required: {0}")]
    HumanReviewRequired(String),
    #[error("serialization error: {0}")]
    SerializationError(String),
    #[error("field not found in output: {0}")]
    FieldNotFound(String),
}
