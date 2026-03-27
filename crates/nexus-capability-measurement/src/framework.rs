//! Core framework types for the capability measurement system.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::scoring::articulation::ArticulationScore;
use crate::scoring::asymmetric::PrimaryScore;
use crate::scoring::gaming_detection::GamingFlag;

// ── Vectors ──────────────────────────────────────────────────────────────────

/// The four measurement vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Vector {
    ReasoningDepth,
    PlanningCoherence,
    AdaptationUnderUncertainty,
    ToolUseIntegrity,
}

// ── Difficulty ───────────────────────────────────────────────────────────────

/// Difficulty levels 1–5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DifficultyLevel {
    Level1,
    Level2,
    Level3,
    Level4,
    Level5,
}

impl DifficultyLevel {
    /// Weight of this level in the vector score.
    pub fn weight(self) -> f64 {
        match self {
            Self::Level1 => 0.10,
            Self::Level2 => 0.15,
            Self::Level3 => 0.20,
            Self::Level4 => 0.25,
            Self::Level5 => 0.30,
        }
    }

    /// Zero-based index for array access.
    pub fn index(self) -> usize {
        match self {
            Self::Level1 => 0,
            Self::Level2 => 1,
            Self::Level3 => 2,
            Self::Level4 => 3,
            Self::Level5 => 4,
        }
    }
}

// ── Measurement Session ──────────────────────────────────────────────────────

/// A complete capability measurement session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementSession {
    pub id: Uuid,
    pub agent_id: String,
    pub agent_autonomy_level: u8,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub vector_results: Vec<VectorResult>,
    pub cross_vector_analysis: Option<CrossVectorAnalysis>,
    pub audit_hash: String,
}

/// Results for a single vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorResult {
    pub vector: Vector,
    pub level_results: Vec<LevelResult>,
    pub gaming_flags: Vec<GamingFlag>,
    pub vector_score: f64,
}

/// Results for a single difficulty level within a vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelResult {
    pub level: DifficultyLevel,
    pub problem_id: String,
    pub problem_version: String,
    pub agent_response: String,
    pub primary_score: PrimaryScore,
    pub articulation_score: ArticulationScore,
    pub gaming_flags: Vec<GamingFlag>,
    pub scorer_agreement: ScorerAgreement,
}

// ── Cross-Vector Analysis ────────────────────────────────────────────────────

/// Cross-vector analysis reveals capability profile shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossVectorAnalysis {
    pub capability_profile: CapabilityProfile,
    pub anomalies: Vec<String>,
    pub overall_classification: AgentClassification,
}

/// Per-vector scores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityProfile {
    pub reasoning_depth: f64,
    pub planning_coherence: f64,
    pub adaptation: f64,
    pub tool_use: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentClassification {
    /// Strong across all vectors.
    Balanced { min_score: f64, max_score: f64 },
    /// High reasoning, low planning.
    TheoreticalReasoner,
    /// High planning, low reasoning.
    ProceduralExecutor,
    /// High tool use, low adaptation.
    RigidToolUser,
    /// Scores collapse at high difficulty.
    PatternMatchingCeiling { ceiling_level: DifficultyLevel },
    /// Anomalous profile requiring human review.
    Anomalous { reason: String },
}

// ── Scorer Agreement ─────────────────────────────────────────────────────────

/// Two-scorer agreement record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScorerAgreement {
    Pending,
    Agreed { average: f64 },
    Disagreed { scorer_a: f64, scorer_b: f64 },
}

fn epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Create a new measurement session (not yet started).
pub fn new_session(agent_id: &str, agent_autonomy_level: u8) -> MeasurementSession {
    MeasurementSession {
        id: Uuid::new_v4(),
        agent_id: agent_id.to_string(),
        agent_autonomy_level,
        started_at: epoch_secs(),
        completed_at: None,
        vector_results: Vec::new(),
        cross_vector_analysis: None,
        audit_hash: String::new(),
    }
}
