use nexus_factory::intent::TaskType;
use nexus_kernel::supervisor::AgentId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Incoming user request to the conductor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRequest {
    pub id: Uuid,
    pub prompt: String,
    pub output_dir: String,
    pub constraints: HashMap<String, String>,
}

impl UserRequest {
    pub fn new(prompt: impl Into<String>, output_dir: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            prompt: prompt.into(),
            output_dir: output_dir.into(),
            constraints: HashMap::new(),
        }
    }
}

/// Maps factory TaskType to a conductor-level role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRole {
    WebBuilder,
    Coder,
    Designer,
    Fixer,
    General,
}

impl AgentRole {
    pub fn from_task_type(t: &TaskType) -> Self {
        match t {
            TaskType::WebBuild | TaskType::CloneSite => AgentRole::WebBuilder,
            TaskType::CodeGen => AgentRole::Coder,
            TaskType::DesignGen => AgentRole::Designer,
            TaskType::FixProject => AgentRole::Fixer,
            _ => AgentRole::General,
        }
    }

    pub fn default_capabilities(&self) -> Vec<String> {
        match self {
            AgentRole::WebBuilder => vec![
                "llm.query".into(),
                "fs.read".into(),
                "fs.write".into(),
                "web.search".into(),
                "web.read".into(),
            ],
            AgentRole::Coder => vec![
                "llm.query".into(),
                "fs.read".into(),
                "fs.write".into(),
                "process.exec".into(),
            ],
            AgentRole::Designer => vec![
                "llm.query".into(),
                "fs.read".into(),
                "fs.write".into(),
            ],
            AgentRole::Fixer => vec![
                "llm.query".into(),
                "fs.read".into(),
                "fs.write".into(),
                "process.exec".into(),
            ],
            AgentRole::General => vec![
                "llm.query".into(),
                "fs.read".into(),
                "fs.write".into(),
            ],
        }
    }

    pub fn agent_crate_name(&self) -> &'static str {
        match self {
            AgentRole::WebBuilder => "web-builder",
            AgentRole::Coder => "coder",
            AgentRole::Designer => "designer",
            AgentRole::Fixer => "fixer",
            AgentRole::General => "general",
        }
    }
}

/// A single subtask in the conductor plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTask {
    pub description: String,
    pub role: AgentRole,
    pub capabilities_needed: Vec<String>,
    pub estimated_fuel: u64,
    pub depends_on: Vec<usize>,
    pub expected_outputs: Vec<String>,
}

/// The full execution plan produced by the planner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConductorPlan {
    pub id: Uuid,
    pub tasks: Vec<PlannedTask>,
}

impl ConductorPlan {
    pub fn new(tasks: Vec<PlannedTask>) -> Self {
        Self {
            id: Uuid::new_v4(),
            tasks,
        }
    }
}

/// Tracks a dispatched agent assignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAssignment {
    pub subtask_id: Uuid,
    pub agent_id: AgentId,
    pub role: AgentRole,
    pub status: TaskStatus,
    pub fuel_allocated: u64,
    pub fuel_used: u64,
    pub output_files: Vec<String>,
    pub error: Option<String>,
}

/// Status of a single task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Final result returned by the conductor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConductorResult {
    pub request_id: Uuid,
    pub plan_id: Uuid,
    pub status: ConductorStatus,
    pub output_dir: String,
    pub output_files: Vec<String>,
    pub agents_used: usize,
    pub total_fuel_used: u64,
    pub duration_secs: f64,
    pub summary: String,
    /// Time machine checkpoint ID — undo the entire conductor run in one action.
    pub checkpoint_id: Option<String>,
}

/// Overall conductor outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConductorStatus {
    Success,
    PartialSuccess,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_factory::intent::TaskType;

    #[test]
    fn test_role_from_task_type() {
        assert_eq!(
            AgentRole::from_task_type(&TaskType::WebBuild),
            AgentRole::WebBuilder
        );
        assert_eq!(
            AgentRole::from_task_type(&TaskType::CodeGen),
            AgentRole::Coder
        );
        assert_eq!(
            AgentRole::from_task_type(&TaskType::DesignGen),
            AgentRole::Designer
        );
        assert_eq!(
            AgentRole::from_task_type(&TaskType::FixProject),
            AgentRole::Fixer
        );
        assert_eq!(
            AgentRole::from_task_type(&TaskType::Research),
            AgentRole::General
        );
        assert_eq!(
            AgentRole::from_task_type(&TaskType::CloneSite),
            AgentRole::WebBuilder
        );
    }

    #[test]
    fn test_capabilities_not_empty() {
        let roles = [
            AgentRole::WebBuilder,
            AgentRole::Coder,
            AgentRole::Designer,
            AgentRole::Fixer,
            AgentRole::General,
        ];
        for role in &roles {
            assert!(!role.default_capabilities().is_empty());
        }
    }

    #[test]
    fn test_agent_crate_names() {
        assert_eq!(AgentRole::WebBuilder.agent_crate_name(), "web-builder");
        assert_eq!(AgentRole::Coder.agent_crate_name(), "coder");
        assert_eq!(AgentRole::Designer.agent_crate_name(), "designer");
        assert_eq!(AgentRole::Fixer.agent_crate_name(), "fixer");
        assert_eq!(AgentRole::General.agent_crate_name(), "general");
    }

    #[test]
    fn test_user_request_new() {
        let req = UserRequest::new("build a website", "/tmp/out");
        assert_eq!(req.prompt, "build a website");
        assert_eq!(req.output_dir, "/tmp/out");
        assert!(req.constraints.is_empty());
    }
}
