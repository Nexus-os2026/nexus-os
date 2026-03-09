use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPlan {
    pub steps: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    Completed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct WorkflowCheckpoint {
    workflow_id: String,
    statuses: BTreeMap<String, StepStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowExecutionReport {
    pub workflow_id: String,
    pub resumed: bool,
    pub completed_steps: Vec<String>,
    pub skipped_steps: Vec<String>,
    pub pending_steps: Vec<String>,
}

pub trait WorkflowStepExecutor {
    fn execute_step(&mut self, step_id: &str) -> Result<(), AgentError>;
}

pub struct ResumableWorkflowEngine {
    checkpoint_root: PathBuf,
    pub audit_trail: AuditTrail,
}

impl ResumableWorkflowEngine {
    pub fn new(checkpoint_root: impl AsRef<Path>) -> Self {
        Self {
            checkpoint_root: checkpoint_root.as_ref().to_path_buf(),
            audit_trail: AuditTrail::new(),
        }
    }

    pub fn execute(
        &mut self,
        workflow_id: &str,
        plan: &WorkflowPlan,
        executor: &mut dyn WorkflowStepExecutor,
    ) -> Result<WorkflowExecutionReport, AgentError> {
        ensure_checkpoint_root(self.checkpoint_root.as_path())?;

        let checkpoint_path = self.checkpoint_path(workflow_id);
        let mut checkpoint = load_checkpoint(checkpoint_path.as_path(), workflow_id, plan)?;
        let resumed = checkpoint_path.exists();

        loop {
            let ready = find_ready_steps(plan, &checkpoint.statuses);

            if ready.is_empty() {
                if has_pending_steps(plan, &checkpoint.statuses) {
                    return Err(AgentError::SupervisorError(
                        "workflow deadlock: unresolved dependencies among pending steps"
                            .to_string(),
                    ));
                }
                break;
            }

            // Execute all currently ready steps; these are dependency-safe to run in parallel.
            // We run deterministically in sorted order for stable replay.
            for step_id in ready {
                self.audit_trail.append_event(
                    uuid::Uuid::nil(),
                    EventType::ToolCall,
                    json!({
                        "event": "workflow_step_start",
                        "workflow_id": workflow_id,
                        "step_id": step_id
                    }),
                )?;

                let execution = executor.execute_step(step_id.as_str());
                if let Err(error) = execution {
                    persist_checkpoint(checkpoint_path.as_path(), &checkpoint)?;
                    self.audit_trail.append_event(
                        uuid::Uuid::nil(),
                        EventType::Error,
                        json!({
                            "event": "workflow_step_error",
                            "workflow_id": workflow_id,
                            "step_id": step_id,
                            "error": error.to_string()
                        }),
                    )?;
                    return Err(error);
                }

                checkpoint
                    .statuses
                    .insert(step_id.clone(), StepStatus::Completed);
                persist_checkpoint(checkpoint_path.as_path(), &checkpoint)?;

                self.audit_trail.append_event(
                    uuid::Uuid::nil(),
                    EventType::ToolCall,
                    json!({
                        "event": "workflow_step_complete",
                        "workflow_id": workflow_id,
                        "step_id": step_id
                    }),
                )?;
            }
        }

        Ok(build_report(workflow_id, resumed, &checkpoint.statuses))
    }

    fn checkpoint_path(&self, workflow_id: &str) -> PathBuf {
        self.checkpoint_root.join(format!(
            "workflow-{}.json",
            sanitize_workflow_id(workflow_id)
        ))
    }
}

fn ensure_checkpoint_root(path: &Path) -> Result<(), AgentError> {
    fs::create_dir_all(path).map_err(|error| {
        AgentError::SupervisorError(format!("failed to create checkpoint root: {error}"))
    })
}

fn load_checkpoint(
    path: &Path,
    workflow_id: &str,
    plan: &WorkflowPlan,
) -> Result<WorkflowCheckpoint, AgentError> {
    let mut checkpoint = if path.exists() {
        let raw = fs::read_to_string(path).map_err(|error| {
            AgentError::SupervisorError(format!("failed reading workflow checkpoint: {error}"))
        })?;

        serde_json::from_str::<WorkflowCheckpoint>(raw.as_str()).map_err(|error| {
            AgentError::SupervisorError(format!("failed parsing workflow checkpoint: {error}"))
        })?
    } else {
        WorkflowCheckpoint {
            workflow_id: workflow_id.to_string(),
            statuses: BTreeMap::new(),
        }
    };

    checkpoint.workflow_id = workflow_id.to_string();
    for step in &plan.steps {
        checkpoint
            .statuses
            .entry(step.id.clone())
            .or_insert(StepStatus::Pending);
    }

    Ok(checkpoint)
}

fn persist_checkpoint(path: &Path, checkpoint: &WorkflowCheckpoint) -> Result<(), AgentError> {
    let serialized = serde_json::to_string_pretty(checkpoint).map_err(|error| {
        AgentError::SupervisorError(format!("failed serializing workflow checkpoint: {error}"))
    })?;

    fs::write(path, serialized).map_err(|error| {
        AgentError::SupervisorError(format!("failed writing workflow checkpoint: {error}"))
    })
}

fn find_ready_steps(plan: &WorkflowPlan, statuses: &BTreeMap<String, StepStatus>) -> Vec<String> {
    let mut ready = Vec::new();

    for step in &plan.steps {
        let status = statuses
            .get(step.id.as_str())
            .copied()
            .unwrap_or(StepStatus::Pending);
        if status != StepStatus::Pending {
            continue;
        }

        let mut dependencies_met = true;
        for dependency in &step.depends_on {
            let dependency_status = statuses
                .get(dependency.as_str())
                .copied()
                .unwrap_or(StepStatus::Pending);
            if dependency_status == StepStatus::Pending {
                dependencies_met = false;
                break;
            }
        }

        if dependencies_met {
            ready.push(step.id.clone());
        }
    }

    ready.sort();
    ready
}

fn has_pending_steps(plan: &WorkflowPlan, statuses: &BTreeMap<String, StepStatus>) -> bool {
    for step in &plan.steps {
        let status = statuses
            .get(step.id.as_str())
            .copied()
            .unwrap_or(StepStatus::Pending);
        if status == StepStatus::Pending {
            return true;
        }
    }
    false
}

fn build_report(
    workflow_id: &str,
    resumed: bool,
    statuses: &BTreeMap<String, StepStatus>,
) -> WorkflowExecutionReport {
    let mut completed_steps = Vec::new();
    let mut skipped_steps = Vec::new();
    let mut pending_steps = Vec::new();

    for (step_id, status) in statuses {
        match status {
            StepStatus::Completed => completed_steps.push(step_id.clone()),
            StepStatus::Skipped => skipped_steps.push(step_id.clone()),
            StepStatus::Pending => pending_steps.push(step_id.clone()),
        }
    }

    WorkflowExecutionReport {
        workflow_id: workflow_id.to_string(),
        resumed,
        completed_steps,
        skipped_steps,
        pending_steps,
    }
}

fn sanitize_workflow_id(workflow_id: &str) -> String {
    let mut out = String::new();
    for ch in workflow_id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "workflow".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::{ResumableWorkflowEngine, WorkflowPlan, WorkflowStep, WorkflowStepExecutor};
    use nexus_kernel::errors::AgentError;
    use std::collections::HashSet;

    struct CrashAfterThreeExecutor {
        executed: Vec<String>,
        crashed: bool,
    }

    impl CrashAfterThreeExecutor {
        fn new() -> Self {
            Self {
                executed: Vec::new(),
                crashed: false,
            }
        }
    }

    impl WorkflowStepExecutor for CrashAfterThreeExecutor {
        fn execute_step(&mut self, step_id: &str) -> Result<(), AgentError> {
            self.executed.push(step_id.to_string());
            if self.executed.len() == 4 && !self.crashed {
                self.crashed = true;
                return Err(AgentError::SupervisorError("simulated crash".to_string()));
            }
            Ok(())
        }
    }

    struct RecordingExecutor {
        executed: Vec<String>,
    }

    impl RecordingExecutor {
        fn new() -> Self {
            Self {
                executed: Vec::new(),
            }
        }
    }

    impl WorkflowStepExecutor for RecordingExecutor {
        fn execute_step(&mut self, step_id: &str) -> Result<(), AgentError> {
            self.executed.push(step_id.to_string());
            Ok(())
        }
    }

    #[test]
    fn test_checkpoint_and_resume() {
        let checkpoint_dir = format!("/tmp/nexus-workflow-checkpoint-{}", uuid::Uuid::new_v4());
        let plan = WorkflowPlan {
            steps: vec![
                WorkflowStep {
                    id: "step-1".to_string(),
                    depends_on: Vec::new(),
                },
                WorkflowStep {
                    id: "step-2".to_string(),
                    depends_on: vec!["step-1".to_string()],
                },
                WorkflowStep {
                    id: "step-3".to_string(),
                    depends_on: vec!["step-2".to_string()],
                },
                WorkflowStep {
                    id: "step-4".to_string(),
                    depends_on: vec!["step-3".to_string()],
                },
                WorkflowStep {
                    id: "step-5".to_string(),
                    depends_on: vec!["step-4".to_string()],
                },
            ],
        };

        let mut engine = ResumableWorkflowEngine::new(checkpoint_dir.as_str());
        let mut first_executor = CrashAfterThreeExecutor::new();
        let first = engine.execute("wf-checkpoint", &plan, &mut first_executor);
        assert!(first.is_err());

        let mut resume_engine = ResumableWorkflowEngine::new(checkpoint_dir.as_str());
        let mut second_executor = RecordingExecutor::new();
        let resumed = resume_engine.execute("wf-checkpoint", &plan, &mut second_executor);
        assert!(resumed.is_ok());

        let expected = ["step-4".to_string(), "step-5".to_string()]
            .into_iter()
            .collect::<HashSet<_>>();
        let actual = second_executor.executed.into_iter().collect::<HashSet<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_dependency_graph() {
        let checkpoint_dir = format!("/tmp/nexus-workflow-deps-{}", uuid::Uuid::new_v4());
        let plan = WorkflowPlan {
            steps: vec![
                WorkflowStep {
                    id: "A".to_string(),
                    depends_on: Vec::new(),
                },
                WorkflowStep {
                    id: "B".to_string(),
                    depends_on: Vec::new(),
                },
                WorkflowStep {
                    id: "C".to_string(),
                    depends_on: vec!["A".to_string(), "B".to_string()],
                },
            ],
        };

        let mut engine = ResumableWorkflowEngine::new(checkpoint_dir.as_str());
        let mut executor = RecordingExecutor::new();
        let result = engine.execute("wf-deps", &plan, &mut executor);
        assert!(result.is_ok());

        let position_a = executor.executed.iter().position(|id| id == "A");
        let position_b = executor.executed.iter().position(|id| id == "B");
        let position_c = executor.executed.iter().position(|id| id == "C");

        assert!(position_a.is_some());
        assert!(position_b.is_some());
        assert!(position_c.is_some());

        if let (Some(a), Some(b), Some(c)) = (position_a, position_b, position_c) {
            assert!(c > a);
            assert!(c > b);
        }
    }
}
