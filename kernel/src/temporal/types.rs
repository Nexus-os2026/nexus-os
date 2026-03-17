//! Core types for the Temporal Engine — timeline forking, scoring, and decisions.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum TemporalError {
    /// Token budget exhausted before all forks completed.
    BudgetExhausted { used: u64, limit: u64 },
    /// LLM call failed during simulation.
    LlmError(String),
    /// Fork count must be >= 1.
    InvalidForkCount(u32),
    /// Requested fork/decision not found.
    NotFound(String),
    /// Checkpoint operation failed.
    CheckpointError(String),
    /// JSON parsing failed.
    ParseError(String),
    /// General internal error.
    Internal(String),
}

impl fmt::Display for TemporalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BudgetExhausted { used, limit } => {
                write!(f, "token budget exhausted: used {used}, limit {limit}")
            }
            Self::LlmError(msg) => write!(f, "LLM error: {msg}"),
            Self::InvalidForkCount(n) => write!(f, "invalid fork count: {n}"),
            Self::NotFound(id) => write!(f, "not found: {id}"),
            Self::CheckpointError(msg) => write!(f, "checkpoint error: {msg}"),
            Self::ParseError(msg) => write!(f, "parse error: {msg}"),
            Self::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for TemporalError {}

impl From<TemporalError> for crate::errors::AgentError {
    fn from(e: TemporalError) -> Self {
        crate::errors::AgentError::SupervisorError(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Evaluation strategy
// ---------------------------------------------------------------------------

/// How to pick the winning timeline from a set of forks.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum EvalStrategy {
    /// Pick timeline with highest final step score.
    #[default]
    BestFinalScore,
    /// Pick timeline with best average score across all steps.
    BestAverageScore,
    /// Pick timeline with smallest worst-case (min step score is highest).
    LowestRisk,
    /// Present top 3 to user and let them choose.
    UserChoice,
}

// ---------------------------------------------------------------------------
// Fork status
// ---------------------------------------------------------------------------

/// Lifecycle state of a single timeline fork.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ForkStatus {
    /// Fork is currently being simulated.
    Simulating,
    /// Simulation finished.
    Completed,
    /// This fork was selected as the best outcome.
    Selected,
    /// Discarded in favour of a better timeline.
    Pruned,
}

// ---------------------------------------------------------------------------
// Timeline step
// ---------------------------------------------------------------------------

/// A single simulated step within a timeline fork.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineStep {
    pub step_number: u32,
    pub action: String,
    pub simulated_outcome: String,
    pub score: f64,
    /// Predicted side-effects of this step.
    pub side_effects: Vec<String>,
    /// Whether this step can be undone.
    pub reversible: bool,
}

// ---------------------------------------------------------------------------
// Timeline fork
// ---------------------------------------------------------------------------

/// One possible future explored by the temporal engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineFork {
    pub fork_id: String,
    /// `None` for the root timeline.
    pub parent_fork: Option<String>,
    /// Human-readable description of the decision point.
    pub branch_point: String,
    /// The approach this fork chose.
    pub chosen_action: String,
    /// Simulated steps in order.
    pub steps: Vec<TimelineStep>,
    /// Score of the final step (or 0.0 if empty).
    pub final_score: f64,
    /// Worst step score across the timeline.
    pub risk_score: f64,
    pub status: ForkStatus,
}

impl TimelineFork {
    /// Create a new fork with a fresh UUID.
    pub fn new(branch_point: &str, chosen_action: &str) -> Self {
        Self {
            fork_id: Uuid::new_v4().to_string(),
            parent_fork: None,
            branch_point: branch_point.to_string(),
            chosen_action: chosen_action.to_string(),
            steps: Vec::new(),
            final_score: 0.0,
            risk_score: 1.0, // worst-case until proven otherwise
            status: ForkStatus::Simulating,
        }
    }

    /// Recalculate `final_score` and `risk_score` from steps.
    pub fn recalculate_scores(&mut self) {
        if self.steps.is_empty() {
            self.final_score = 0.0;
            self.risk_score = 0.0;
            return;
        }
        self.final_score = self.steps.last().map(|s| s.score).unwrap_or(0.0);
        self.risk_score = self
            .steps
            .iter()
            .map(|s| s.score)
            .fold(f64::INFINITY, f64::min);
    }

    /// Average score across all steps.
    pub fn average_score(&self) -> f64 {
        if self.steps.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.steps.iter().map(|s| s.score).sum();
        sum / self.steps.len() as f64
    }
}

// ---------------------------------------------------------------------------
// Temporal decision
// ---------------------------------------------------------------------------

/// The complete record of a fork-and-evaluate cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalDecision {
    pub decision_id: String,
    pub original_request: String,
    pub branch_point: String,
    pub forks: Vec<TimelineFork>,
    /// Fork ID of the selected timeline (None if `UserChoice` pending).
    pub selected_fork: Option<String>,
    /// Why this timeline was chosen.
    pub reasoning: String,
    pub total_tokens_used: u64,
    pub simulation_time_ms: u64,
}

impl TemporalDecision {
    pub fn new(request: &str, branch_point: &str) -> Self {
        Self {
            decision_id: Uuid::new_v4().to_string(),
            original_request: request.to_string(),
            branch_point: branch_point.to_string(),
            forks: Vec::new(),
            selected_fork: None,
            reasoning: String::new(),
            total_tokens_used: 0,
            simulation_time_ms: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Time-dilated session types
// ---------------------------------------------------------------------------

/// A produced artifact from a dilated work session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub artifact_type: String,
    pub content: String,
    /// Which iteration produced this version.
    pub iteration: u32,
    pub score: f64,
}

/// Record of a time-dilated work session (compressed iteration cycles).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeDilatedSession {
    pub session_id: String,
    pub task: String,
    /// Agent IDs involved.
    pub agent_team: Vec<String>,
    /// Wall-clock seconds spent.
    pub real_time_budget_seconds: u64,
    /// How many create→critique loops ran.
    pub simulated_iterations: u32,
    /// Artifacts produced (best version per name).
    pub artifacts: Vec<Artifact>,
    /// Score at each iteration.
    pub quality_progression: Vec<f64>,
    pub final_score: f64,
}

impl TimeDilatedSession {
    pub fn new(task: &str, agent_team: Vec<String>) -> Self {
        Self {
            session_id: Uuid::new_v4().to_string(),
            task: task.to_string(),
            agent_team,
            real_time_budget_seconds: 0,
            simulated_iterations: 0,
            artifacts: Vec::new(),
            quality_progression: Vec::new(),
            final_score: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Checkpoint types
// ---------------------------------------------------------------------------

/// Snapshot taken before a fork so we can rollback if the chosen timeline fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalCheckpoint {
    pub checkpoint_id: String,
    pub fork_id: String,
    pub timestamp: u64,
    /// Agent consciousness snapshots at fork time.
    pub agent_states: std::collections::HashMap<String, serde_json::Value>,
    pub decision_context: String,
}

impl TemporalCheckpoint {
    pub fn new(fork_id: &str, decision_context: &str) -> Self {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            checkpoint_id: Uuid::new_v4().to_string(),
            fork_id: fork_id.to_string(),
            timestamp: ts,
            agent_states: std::collections::HashMap::new(),
            decision_context: decision_context.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Runtime configuration for the temporal engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalConfig {
    pub max_parallel_forks: u32,
    pub max_depth_per_fork: u32,
    pub fork_budget_tokens: u64,
    pub evaluation_strategy: EvalStrategy,
}

impl Default for TemporalConfig {
    fn default() -> Self {
        Self {
            max_parallel_forks: 5,
            max_depth_per_fork: 10,
            fork_budget_tokens: 50_000,
            evaluation_strategy: EvalStrategy::BestFinalScore,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fork_new_has_uuid() {
        let f = TimelineFork::new("deploy or not", "deploy canary first");
        assert!(!f.fork_id.is_empty());
        assert_eq!(f.status, ForkStatus::Simulating);
        assert_eq!(f.branch_point, "deploy or not");
    }

    #[test]
    fn fork_recalculate_scores() {
        let mut f = TimelineFork::new("bp", "act");
        f.steps.push(TimelineStep {
            step_number: 1,
            action: "a".into(),
            simulated_outcome: "ok".into(),
            score: 7.0,
            side_effects: vec![],
            reversible: true,
        });
        f.steps.push(TimelineStep {
            step_number: 2,
            action: "b".into(),
            simulated_outcome: "good".into(),
            score: 9.0,
            side_effects: vec![],
            reversible: false,
        });
        f.recalculate_scores();
        assert!((f.final_score - 9.0).abs() < f64::EPSILON);
        assert!((f.risk_score - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fork_average_score() {
        let mut f = TimelineFork::new("bp", "act");
        f.steps.push(TimelineStep {
            step_number: 1,
            action: "a".into(),
            simulated_outcome: "ok".into(),
            score: 6.0,
            side_effects: vec![],
            reversible: true,
        });
        f.steps.push(TimelineStep {
            step_number: 2,
            action: "b".into(),
            simulated_outcome: "ok".into(),
            score: 8.0,
            side_effects: vec![],
            reversible: true,
        });
        assert!((f.average_score() - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fork_empty_scores() {
        let mut f = TimelineFork::new("bp", "act");
        f.recalculate_scores();
        assert!((f.final_score).abs() < f64::EPSILON);
        assert!((f.average_score()).abs() < f64::EPSILON);
    }

    #[test]
    fn temporal_decision_new() {
        let d = TemporalDecision::new("design schema", "schema approach");
        assert!(!d.decision_id.is_empty());
        assert!(d.selected_fork.is_none());
        assert!(d.forks.is_empty());
    }

    #[test]
    fn temporal_checkpoint_new() {
        let cp = TemporalCheckpoint::new("fork-1", "pre-deploy");
        assert!(!cp.checkpoint_id.is_empty());
        assert_eq!(cp.fork_id, "fork-1");
        assert!(cp.timestamp > 0);
    }

    #[test]
    fn temporal_config_defaults() {
        let cfg = TemporalConfig::default();
        assert_eq!(cfg.max_parallel_forks, 5);
        assert_eq!(cfg.max_depth_per_fork, 10);
        assert_eq!(cfg.fork_budget_tokens, 50_000);
        assert_eq!(cfg.evaluation_strategy, EvalStrategy::BestFinalScore);
    }

    #[test]
    fn dilated_session_new() {
        let s = TimeDilatedSession::new("write scraper", vec!["a1".into(), "a2".into()]);
        assert!(!s.session_id.is_empty());
        assert_eq!(s.agent_team.len(), 2);
        assert!(s.artifacts.is_empty());
    }

    #[test]
    fn eval_strategy_default() {
        let s = EvalStrategy::default();
        assert_eq!(s, EvalStrategy::BestFinalScore);
    }

    #[test]
    fn error_display() {
        let e = TemporalError::BudgetExhausted {
            used: 100,
            limit: 50,
        };
        assert!(e.to_string().contains("100"));
        assert!(e.to_string().contains("50"));
    }
}
