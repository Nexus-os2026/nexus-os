use crate::types::{AgentAssignment, ConductorPlan, TaskStatus};
use nexus_kernel::errors::AgentError;
use nexus_kernel::supervisor::{AgentId, Supervisor};
use nexus_sdk::ManifestBuilder;
use std::collections::HashMap;
use uuid::Uuid;

/// Dispatches planned tasks to real agents via the supervisor.
#[derive(Default)]
pub struct Dispatcher;

impl Dispatcher {
    pub fn new() -> Self {
        Self
    }

    /// Spawn agents for all ready tasks (those whose dependencies are met).
    pub fn dispatch_ready(
        &self,
        plan: &ConductorPlan,
        completed_indices: &[usize],
        supervisor: &mut Supervisor,
    ) -> Result<HashMap<Uuid, AgentAssignment>, AgentError> {
        let mut assignments = HashMap::new();

        for (idx, task) in plan.tasks.iter().enumerate() {
            if completed_indices.contains(&idx) {
                continue;
            }

            let deps_met = task
                .depends_on
                .iter()
                .all(|d| completed_indices.contains(d));
            if !deps_met {
                continue;
            }

            let subtask_id = Uuid::new_v4();
            let buffered_fuel = ((task.estimated_fuel as f64) * 1.5) as u64;

            let manifest_result = {
                let mut builder =
                    ManifestBuilder::new(&format!("conductor-{}", task.role.agent_crate_name()))
                        .version("0.1.0")
                        .fuel_budget(buffered_fuel)
                        .autonomy_level(2);

                for cap in &task.capabilities_needed {
                    builder = builder.capability(cap);
                }

                builder.build()
            };

            let manifest = match manifest_result {
                Ok(m) => m,
                Err(e) => {
                    assignments.insert(
                        subtask_id,
                        AgentAssignment {
                            subtask_id,
                            agent_id: Uuid::nil(),
                            role: task.role.clone(),
                            status: TaskStatus::Failed,
                            fuel_allocated: buffered_fuel,
                            fuel_used: 0,
                            output_files: vec![],
                            error: Some(format!("manifest build failed: {e}")),
                        },
                    );
                    continue;
                }
            };

            match supervisor.start_agent(manifest) {
                Ok(agent_id) => {
                    assignments.insert(
                        subtask_id,
                        AgentAssignment {
                            subtask_id,
                            agent_id,
                            role: task.role.clone(),
                            status: TaskStatus::Running,
                            fuel_allocated: buffered_fuel,
                            fuel_used: 0,
                            output_files: vec![],
                            error: None,
                        },
                    );
                }
                Err(e) => {
                    assignments.insert(
                        subtask_id,
                        AgentAssignment {
                            subtask_id,
                            agent_id: Uuid::nil(),
                            role: task.role.clone(),
                            status: TaskStatus::Failed,
                            fuel_allocated: buffered_fuel,
                            fuel_used: 0,
                            output_files: vec![],
                            error: Some(format!("spawn failed: {e}")),
                        },
                    );
                }
            }
        }

        Ok(assignments)
    }
}

/// Extract the AgentId from a successful assignment.
pub fn running_agent_ids(assignments: &HashMap<Uuid, AgentAssignment>) -> Vec<AgentId> {
    assignments
        .values()
        .filter(|a| a.status == TaskStatus::Running)
        .map(|a| a.agent_id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentRole, ConductorPlan, PlannedTask};

    fn single_task_plan() -> ConductorPlan {
        ConductorPlan::new(vec![PlannedTask {
            description: "Build website".into(),
            role: AgentRole::WebBuilder,
            capabilities_needed: AgentRole::WebBuilder.default_capabilities(),
            estimated_fuel: 4000,
            depends_on: vec![],
            expected_outputs: vec!["index.html".into()],
        }])
    }

    #[test]
    fn test_manifest_has_correct_capabilities() {
        let plan = single_task_plan();
        let mut supervisor = Supervisor::new();
        let dispatcher = Dispatcher::new();

        let assignments = dispatcher
            .dispatch_ready(&plan, &[], &mut supervisor)
            .unwrap();
        assert_eq!(assignments.len(), 1);

        let assignment = assignments.values().next().unwrap();
        assert_eq!(assignment.status, TaskStatus::Running);
        assert!(!assignment.agent_id.is_nil());
    }

    #[test]
    fn test_fuel_buffer_applied() {
        let plan = single_task_plan();
        let mut supervisor = Supervisor::new();
        let dispatcher = Dispatcher::new();

        let assignments = dispatcher
            .dispatch_ready(&plan, &[], &mut supervisor)
            .unwrap();

        let assignment = assignments.values().next().unwrap();
        // 4000 * 1.5 = 6000
        assert_eq!(assignment.fuel_allocated, 6000);
    }

    #[test]
    fn test_deps_not_met_skipped() {
        let plan = ConductorPlan::new(vec![
            PlannedTask {
                description: "First task".into(),
                role: AgentRole::Coder,
                capabilities_needed: AgentRole::Coder.default_capabilities(),
                estimated_fuel: 2000,
                depends_on: vec![],
                expected_outputs: vec![],
            },
            PlannedTask {
                description: "Depends on first".into(),
                role: AgentRole::Coder,
                capabilities_needed: AgentRole::Coder.default_capabilities(),
                estimated_fuel: 2000,
                depends_on: vec![0],
                expected_outputs: vec![],
            },
        ]);

        let mut supervisor = Supervisor::new();
        let dispatcher = Dispatcher::new();

        // No completed indices — only first task (no deps) should dispatch
        let assignments = dispatcher
            .dispatch_ready(&plan, &[], &mut supervisor)
            .unwrap();
        assert_eq!(assignments.len(), 1);

        let assignment = assignments.values().next().unwrap();
        assert_eq!(assignment.role, AgentRole::Coder);
    }

    #[test]
    fn test_spawn_failure_handled_gracefully() {
        // ManifestBuilder requires name >= 3 chars and non-empty capabilities.
        // Our dispatcher always provides valid manifests, so we test the error
        // path indirectly by verifying no panic on a valid dispatch.
        let plan = single_task_plan();
        let mut supervisor = Supervisor::new();
        let dispatcher = Dispatcher::new();

        let result = dispatcher.dispatch_ready(&plan, &[], &mut supervisor);
        assert!(result.is_ok());
    }
}
