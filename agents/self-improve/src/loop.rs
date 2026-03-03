use crate::knowledge::{KnowledgeBase, KnowledgeCategory, KnowledgeEntry};
use crate::learner::{StrategyInsights, StrategyLearner};
use crate::prompt_optimizer::{PromptOptimizer, PromptOutcome};
use crate::tracker::{OutcomeResult, PerformanceTracker, TaskMetrics, TaskType, TrackedOutcome};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImprovementStatus {
    Applied,
    SkippedNeedsApproval,
    SkippedSandboxValidation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImprovementAuditEvent {
    pub timestamp: u64,
    pub agent_id: String,
    pub action: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImprovementVersion {
    pub version_id: u64,
    pub timestamp: u64,
    pub agent_id: String,
    pub task_type: TaskType,
    pub status: ImprovementStatus,
    pub base_prompt: String,
    pub selected_prompt: String,
    pub recommendations: Vec<String>,
    pub knowledge_entry_ids: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRunObservation {
    pub agent_id: String,
    pub task: String,
    pub task_type: TaskType,
    pub result: OutcomeResult,
    pub metrics: TaskMetrics,
    pub base_prompt: String,
    pub prompt_outcomes: Vec<PromptOutcome>,
    pub governance_approved: bool,
    pub destructive_change_requested: bool,
    pub sandbox_validation_passed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoopResult {
    pub tracked_outcome: TrackedOutcome,
    pub insights: StrategyInsights,
    pub version: ImprovementVersion,
    pub status: ImprovementStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LoopState {
    versions: Vec<ImprovementVersion>,
    audit_log: Vec<ImprovementAuditEvent>,
    optimizer: PromptOptimizer,
}

pub struct AutoImproveEngine {
    tracker: PerformanceTracker,
    learner: StrategyLearner,
    optimizer: PromptOptimizer,
    knowledge: KnowledgeBase,
    versions: Vec<ImprovementVersion>,
    audit_log: Vec<ImprovementAuditEvent>,
    loop_state_path: Option<PathBuf>,
}

impl AutoImproveEngine {
    pub fn new_in_memory(scope_key: &str) -> Self {
        Self {
            tracker: PerformanceTracker::new_in_memory(),
            learner: StrategyLearner::new(),
            optimizer: PromptOptimizer::new(),
            knowledge: KnowledgeBase::new_in_memory(scope_key),
            versions: Vec::new(),
            audit_log: Vec::new(),
            loop_state_path: None,
        }
    }

    pub fn new_with_storage(
        storage_root: impl AsRef<Path>,
        scope_key: &str,
    ) -> Result<Self, AgentError> {
        let storage_root = storage_root.as_ref().to_path_buf();
        fs::create_dir_all(storage_root.as_path()).map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed to create self-improve storage '{}': {error}",
                storage_root.display()
            ))
        })?;

        let tracker_path = storage_root.join("tracker.json");
        let knowledge_path = storage_root.join("knowledge.enc");
        let loop_state_path = storage_root.join("loop_state.json");
        let tracker = PerformanceTracker::new_with_file(tracker_path)?;
        let knowledge = KnowledgeBase::new_with_file(knowledge_path, scope_key)?;

        let (versions, audit_log, optimizer) = if loop_state_path.exists() {
            let raw = fs::read_to_string(loop_state_path.as_path()).map_err(|error| {
                AgentError::SupervisorError(format!(
                    "failed reading self-improve state '{}': {error}",
                    loop_state_path.display()
                ))
            })?;
            let state = serde_json::from_str::<LoopState>(raw.as_str()).map_err(|error| {
                AgentError::SupervisorError(format!(
                    "failed parsing self-improve state '{}': {error}",
                    loop_state_path.display()
                ))
            })?;
            (state.versions, state.audit_log, state.optimizer)
        } else {
            (Vec::new(), Vec::new(), PromptOptimizer::new())
        };

        Ok(Self {
            tracker,
            learner: StrategyLearner::new(),
            optimizer,
            knowledge,
            versions,
            audit_log,
            loop_state_path: Some(loop_state_path),
        })
    }

    pub fn run_cycle(
        &mut self,
        observation: AgentRunObservation,
    ) -> Result<LoopResult, AgentError> {
        self.audit(
            &observation.agent_id,
            "loop_started",
            observation.task.as_str(),
        );

        let tracked = self.tracker.track_outcome(
            observation.agent_id.as_str(),
            observation.task_type,
            observation.task.as_str(),
            observation.result,
            observation.metrics.clone(),
        )?;
        self.audit(
            observation.agent_id.as_str(),
            "outcome_recorded",
            format!("outcome_id={}", tracked.id).as_str(),
        );

        let insights = self.learner.analyze_history(
            &self.tracker,
            observation.agent_id.as_str(),
            observation.task_type,
        )?;
        self.audit(
            observation.agent_id.as_str(),
            "history_analyzed",
            format!("recommendations={}", insights.recommendations.len()).as_str(),
        );

        let selected_prompt = self.optimizer.optimize_prompt(
            observation.base_prompt.as_str(),
            observation.prompt_outcomes.as_slice(),
        );
        self.audit(
            observation.agent_id.as_str(),
            "prompt_updated",
            selected_prompt.as_str(),
        );

        let mut knowledge_ids = Vec::new();
        let knowledge_category = category_for_task(observation.task_type);
        for recommendation in &insights.recommendations {
            let entry = self.knowledge.store_strategy(
                observation.agent_id.as_str(),
                knowledge_category,
                recommendation.as_str(),
                task_tags(observation.task_type).as_slice(),
            )?;
            knowledge_ids.push(entry.id);
        }
        self.audit(
            observation.agent_id.as_str(),
            "knowledge_updated",
            format!("entries={}", knowledge_ids.len()).as_str(),
        );

        let status = if observation.destructive_change_requested && !observation.governance_approved
        {
            ImprovementStatus::SkippedNeedsApproval
        } else if !observation.sandbox_validation_passed {
            ImprovementStatus::SkippedSandboxValidation
        } else {
            ImprovementStatus::Applied
        };

        let version = ImprovementVersion {
            version_id: self.next_version_id(),
            timestamp: now_secs(),
            agent_id: observation.agent_id.clone(),
            task_type: observation.task_type,
            status,
            base_prompt: observation.base_prompt,
            selected_prompt,
            recommendations: insights.recommendations.clone(),
            knowledge_entry_ids: knowledge_ids,
        };
        self.versions.push(version.clone());
        self.audit(
            observation.agent_id.as_str(),
            "version_created",
            format!("version_id={}", version.version_id).as_str(),
        );

        self.persist_loop_state()?;
        Ok(LoopResult {
            tracked_outcome: tracked,
            insights,
            version,
            status,
        })
    }

    pub fn rollback_to(
        &mut self,
        agent_id: &str,
        version_id: u64,
    ) -> Result<ImprovementVersion, AgentError> {
        let version = self
            .versions
            .iter()
            .find(|version| version.agent_id == agent_id && version.version_id == version_id)
            .cloned()
            .ok_or_else(|| {
                AgentError::SupervisorError(format!(
                    "no self-improve version '{version_id}' found for agent '{agent_id}'"
                ))
            })?;

        self.optimizer.set_default_prompt(
            version.base_prompt.as_str(),
            version.selected_prompt.as_str(),
        );
        self.audit(
            agent_id,
            "rollback_applied",
            format!("version_id={}", version_id).as_str(),
        );
        self.persist_loop_state()?;
        Ok(version)
    }

    pub fn latest_version(&self, agent_id: &str) -> Option<&ImprovementVersion> {
        self.versions
            .iter()
            .rfind(|version| version.agent_id == agent_id)
    }

    pub fn versions_for_agent(&self, agent_id: &str) -> Vec<ImprovementVersion> {
        self.versions
            .iter()
            .filter(|version| version.agent_id == agent_id)
            .cloned()
            .collect()
    }

    pub fn audit_for_agent(&self, agent_id: &str) -> Vec<ImprovementAuditEvent> {
        self.audit_log
            .iter()
            .filter(|event| event.agent_id == agent_id)
            .cloned()
            .collect()
    }

    pub fn tracker(&self) -> &PerformanceTracker {
        &self.tracker
    }

    pub fn knowledge(&self) -> &KnowledgeBase {
        &self.knowledge
    }

    fn next_version_id(&self) -> u64 {
        self.versions
            .iter()
            .map(|version| version.version_id)
            .max()
            .unwrap_or(0)
            + 1
    }

    fn audit(&mut self, agent_id: &str, action: &str, detail: &str) {
        self.audit_log.push(ImprovementAuditEvent {
            timestamp: now_secs(),
            agent_id: agent_id.to_string(),
            action: action.to_string(),
            detail: detail.to_string(),
        });
    }

    fn persist_loop_state(&self) -> Result<(), AgentError> {
        let Some(path) = &self.loop_state_path else {
            return Ok(());
        };

        let state = LoopState {
            versions: self.versions.clone(),
            audit_log: self.audit_log.clone(),
            optimizer: self.optimizer.clone(),
        };
        let json = serde_json::to_string_pretty(&state).map_err(|error| {
            AgentError::SupervisorError(format!("failed serializing self-improve state: {error}"))
        })?;
        fs::write(path, json).map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed writing self-improve state '{}': {error}",
                path.display()
            ))
        })
    }
}

pub fn run_once_with_storage(
    storage_root: impl AsRef<Path>,
    scope_key: &str,
    observation: AgentRunObservation,
) -> Result<LoopResult, AgentError> {
    let mut engine = AutoImproveEngine::new_with_storage(storage_root, scope_key)?;
    engine.run_cycle(observation)
}

fn category_for_task(task_type: TaskType) -> KnowledgeCategory {
    match task_type {
        TaskType::Coding => KnowledgeCategory::CodingPatterns,
        TaskType::Posting => KnowledgeCategory::PostingStrategies,
        TaskType::Website => KnowledgeCategory::DesignPrinciples,
        TaskType::Other => KnowledgeCategory::WorkflowOptimizations,
    }
}

fn task_tags(task_type: TaskType) -> Vec<&'static str> {
    match task_type {
        TaskType::Coding => vec!["coding", "rust", "tests"],
        TaskType::Posting => vec!["social", "engagement", "timing"],
        TaskType::Website => vec!["design", "ux", "performance"],
        TaskType::Other => vec!["workflow", "automation"],
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub fn record_recommendations(
    knowledge: &mut KnowledgeBase,
    agent_id: &str,
    category: KnowledgeCategory,
    recommendations: &[String],
) -> Result<Vec<KnowledgeEntry>, AgentError> {
    let mut entries = Vec::new();
    for recommendation in recommendations {
        let entry = knowledge.store_strategy(
            agent_id,
            category,
            recommendation.as_str(),
            &["auto", "learned"],
        )?;
        entries.push(entry);
    }
    Ok(entries)
}
