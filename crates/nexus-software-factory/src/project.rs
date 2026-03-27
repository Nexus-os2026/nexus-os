use serde::{Deserialize, Serialize};

use crate::artifacts::ProjectArtifact;
use crate::pipeline::PipelineStage;
use crate::quality::QualityGateResult;
use crate::roles::TeamMember;

/// A software project managed by the factory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub user_request: String,
    pub team: Vec<TeamMember>,
    pub current_stage: PipelineStage,
    pub artifacts: Vec<ProjectArtifact>,
    pub quality_gates: Vec<QualityGateResult>,
    pub status: ProjectStatus,
    pub collaboration_session: Option<String>,
    pub total_cost: u64,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    pub history: Vec<ProjectEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectStatus {
    Initializing,
    InProgress,
    QualityGateHold {
        stage: PipelineStage,
        reason: String,
    },
    HumanReview {
        stage: PipelineStage,
    },
    Completed,
    Failed {
        stage: PipelineStage,
        reason: String,
    },
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEvent {
    pub timestamp: u64,
    pub stage: PipelineStage,
    pub agent_id: String,
    pub event_type: EventType,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    StageStarted,
    ArtifactProduced,
    QualityGatePassed,
    QualityGateFailed,
    CollaborationStarted,
    CollaborationCompleted,
    HumanReviewRequested,
    HumanReviewCompleted,
    StageCompleted,
    Error,
}

impl Project {
    pub fn new(title: String, user_request: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            user_request,
            team: Vec::new(),
            current_stage: PipelineStage::Requirements,
            artifacts: Vec::new(),
            quality_gates: Vec::new(),
            status: ProjectStatus::Initializing,
            collaboration_session: None,
            total_cost: 0,
            created_at: epoch_now(),
            completed_at: None,
            history: Vec::new(),
        }
    }

    pub fn add_event(
        &mut self,
        stage: PipelineStage,
        agent_id: &str,
        event_type: EventType,
        description: &str,
    ) {
        self.history.push(ProjectEvent {
            timestamp: epoch_now(),
            stage,
            agent_id: agent_id.into(),
            event_type,
            description: description.into(),
        });
    }

    pub fn get_artifact(&self, artifact_type: &str) -> Option<&ProjectArtifact> {
        self.artifacts
            .iter()
            .rev()
            .find(|a| a.artifact_type == artifact_type)
    }

    pub fn duration_secs(&self) -> u64 {
        let end = self.completed_at.unwrap_or_else(epoch_now);
        end.saturating_sub(self.created_at)
    }
}

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
