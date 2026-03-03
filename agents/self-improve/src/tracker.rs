use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    Coding,
    Posting,
    Website,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutcomeResult {
    Success,
    Failure,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricKind {
    TestPassRate,
    FixIterations,
    CodeQualityScore,
    EngagementRate,
    ApprovalRate,
    Reach,
    BuildSuccess,
    UserSatisfaction,
    LoadTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendDirection {
    Improving,
    Declining,
    Stable,
    InsufficientData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TaskMetrics {
    pub test_pass_rate: Option<f64>,
    pub fix_iterations: Option<f64>,
    pub code_quality_score: Option<f64>,
    pub engagement_rate: Option<f64>,
    pub approval_rate: Option<f64>,
    pub reach: Option<f64>,
    pub build_success: Option<f64>,
    pub user_satisfaction: Option<f64>,
    pub load_time: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackedOutcome {
    pub id: u64,
    pub timestamp: u64,
    pub agent_id: String,
    pub task_type: TaskType,
    pub task: String,
    pub result: OutcomeResult,
    pub metrics: TaskMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrackerDb {
    outcomes: Vec<TrackedOutcome>,
}

#[derive(Debug, Clone)]
pub struct PerformanceTracker {
    storage_path: Option<PathBuf>,
    outcomes: Vec<TrackedOutcome>,
    next_id: u64,
}

impl Default for PerformanceTracker {
    fn default() -> Self {
        Self::new_in_memory()
    }
}

impl PerformanceTracker {
    pub fn new_in_memory() -> Self {
        Self {
            storage_path: None,
            outcomes: Vec::new(),
            next_id: 1,
        }
    }

    pub fn new_with_file(path: impl AsRef<Path>) -> Result<Self, AgentError> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Ok(Self {
                storage_path: Some(path),
                outcomes: Vec::new(),
                next_id: 1,
            });
        }

        let content = fs::read_to_string(path.as_path()).map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed to read tracker database '{}': {error}",
                path.display()
            ))
        })?;
        let db = serde_json::from_str::<TrackerDb>(content.as_str()).map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed to parse tracker database '{}': {error}",
                path.display()
            ))
        })?;

        let next_id = db
            .outcomes
            .iter()
            .map(|outcome| outcome.id)
            .max()
            .unwrap_or(0)
            + 1;

        Ok(Self {
            storage_path: Some(path),
            outcomes: db.outcomes,
            next_id,
        })
    }

    pub fn track_outcome(
        &mut self,
        agent_id: &str,
        task_type: TaskType,
        task: &str,
        result: OutcomeResult,
        metrics: TaskMetrics,
    ) -> Result<TrackedOutcome, AgentError> {
        let outcome = TrackedOutcome {
            id: self.next_id,
            timestamp: now_secs(),
            agent_id: agent_id.to_string(),
            task_type,
            task: task.to_string(),
            result,
            metrics,
        };
        self.next_id = self.next_id.saturating_add(1);
        self.outcomes.push(outcome.clone());
        self.persist()?;
        Ok(outcome)
    }

    pub fn outcomes_for(&self, agent_id: &str, task_type: TaskType) -> Vec<TrackedOutcome> {
        let mut history = self
            .outcomes
            .iter()
            .filter(|outcome| outcome.agent_id == agent_id && outcome.task_type == task_type)
            .cloned()
            .collect::<Vec<_>>();
        history.sort_by(|left, right| left.id.cmp(&right.id));
        history
    }

    pub fn trend_for(
        &self,
        agent_id: &str,
        task_type: TaskType,
        metric: MetricKind,
    ) -> TrendDirection {
        let history = self.outcomes_for(agent_id, task_type);
        let values = history
            .iter()
            .filter_map(|outcome| metric_value(outcome, metric))
            .collect::<Vec<_>>();
        if values.len() < 4 {
            return TrendDirection::InsufficientData;
        }

        let midpoint = values.len() / 2;
        let first = average(&values[..midpoint]);
        let second = average(&values[midpoint..]);
        let delta = second - first;

        if delta > 0.02 {
            TrendDirection::Improving
        } else if delta < -0.02 {
            TrendDirection::Declining
        } else {
            TrendDirection::Stable
        }
    }

    pub fn all_outcomes(&self) -> &[TrackedOutcome] {
        &self.outcomes
    }

    fn persist(&self) -> Result<(), AgentError> {
        let Some(path) = &self.storage_path else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AgentError::SupervisorError(format!(
                    "failed to create tracker parent '{}': {error}",
                    parent.display()
                ))
            })?;
        }

        let db = TrackerDb {
            outcomes: self.outcomes.clone(),
        };
        let serialized = serde_json::to_string_pretty(&db).map_err(|error| {
            AgentError::SupervisorError(format!("failed serializing tracker db: {error}"))
        })?;
        fs::write(path, serialized).map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed writing tracker database '{}': {error}",
                path.display()
            ))
        })
    }
}

fn metric_value(outcome: &TrackedOutcome, metric: MetricKind) -> Option<f64> {
    match metric {
        MetricKind::TestPassRate => outcome.metrics.test_pass_rate,
        MetricKind::FixIterations => outcome.metrics.fix_iterations.map(|value| -value),
        MetricKind::CodeQualityScore => outcome.metrics.code_quality_score,
        MetricKind::EngagementRate => outcome.metrics.engagement_rate,
        MetricKind::ApprovalRate => outcome.metrics.approval_rate,
        MetricKind::Reach => outcome.metrics.reach,
        MetricKind::BuildSuccess => outcome.metrics.build_success,
        MetricKind::UserSatisfaction => outcome.metrics.user_satisfaction,
        MetricKind::LoadTime => outcome.metrics.load_time.map(|value| -value),
    }
}

fn average(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
