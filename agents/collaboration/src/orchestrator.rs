//! Task decomposition and capability-based agent assignment.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubTaskStatus {
    Pending,
    Assigned,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: Uuid,
    pub description: String,
    pub required_capabilities: Vec<String>,
    pub estimated_fuel: u64,
    pub status: SubTaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub description: String,
    pub subtasks: Vec<SubTask>,
    pub assignments: HashMap<Uuid, Uuid>, // subtask_id -> agent_id
}

impl Task {
    pub fn get_subtask(&self, subtask_id: Uuid) -> Option<&SubTask> {
        self.subtasks.iter().find(|s| s.id == subtask_id)
    }

    fn get_subtask_mut(&mut self, subtask_id: Uuid) -> Option<&mut SubTask> {
        self.subtasks.iter_mut().find(|s| s.id == subtask_id)
    }
}

#[derive(Debug)]
pub struct Orchestrator {
    agents: HashMap<Uuid, Vec<String>>, // agent_id -> capabilities
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    pub fn register_agent(&mut self, id: Uuid, capabilities: Vec<String>) {
        self.agents.insert(id, capabilities);
    }

    /// Create a Task from a description and a list of (description, required_caps, estimated_fuel).
    pub fn decompose(
        &self,
        description: &str,
        subtask_specs: Vec<(&str, Vec<String>, u64)>,
    ) -> Task {
        let subtasks = subtask_specs
            .into_iter()
            .map(|(desc, caps, fuel)| SubTask {
                id: Uuid::new_v4(),
                description: desc.to_string(),
                required_capabilities: caps,
                estimated_fuel: fuel,
                status: SubTaskStatus::Pending,
            })
            .collect();

        Task {
            id: Uuid::new_v4(),
            description: description.to_string(),
            subtasks,
            assignments: HashMap::new(),
        }
    }

    /// Assign subtasks to agents by matching required capabilities.
    /// Each subtask is assigned to the first agent whose capabilities
    /// are a superset of the subtask's required capabilities.
    pub fn assign(&self, task: &mut Task) -> usize {
        let mut assigned = 0;

        for subtask in &mut task.subtasks {
            if subtask.status != SubTaskStatus::Pending {
                continue;
            }

            for (agent_id, agent_caps) in &self.agents {
                let has_all = subtask
                    .required_capabilities
                    .iter()
                    .all(|req| agent_caps.contains(req));

                if has_all {
                    task.assignments.insert(subtask.id, *agent_id);
                    subtask.status = SubTaskStatus::Assigned;
                    assigned += 1;
                    break;
                }
            }
        }

        assigned
    }

    pub fn complete_subtask(&self, task: &mut Task, subtask_id: Uuid) -> bool {
        if let Some(subtask) = task.get_subtask_mut(subtask_id) {
            subtask.status = SubTaskStatus::Completed;
            true
        } else {
            false
        }
    }

    pub fn fail_subtask(&self, task: &mut Task, subtask_id: Uuid) -> bool {
        if let Some(subtask) = task.get_subtask_mut(subtask_id) {
            subtask.status = SubTaskStatus::Failed;
            true
        } else {
            false
        }
    }

    pub fn is_complete(&self, task: &Task) -> bool {
        !task.subtasks.is_empty()
            && task
                .subtasks
                .iter()
                .all(|s| s.status == SubTaskStatus::Completed)
    }
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_decompose() {
        let mut orch = Orchestrator::new();
        let coder = Uuid::new_v4();
        orch.register_agent(coder, vec!["code".to_string(), "test".to_string()]);

        let task = orch.decompose(
            "Build a feature",
            vec![
                ("Write code", vec!["code".to_string()], 50),
                ("Write tests", vec!["test".to_string()], 30),
            ],
        );

        assert_eq!(task.description, "Build a feature");
        assert_eq!(task.subtasks.len(), 2);
        assert_eq!(task.subtasks[0].status, SubTaskStatus::Pending);
        assert_eq!(task.subtasks[1].estimated_fuel, 30);
    }

    #[test]
    fn assign_matches_by_capability() {
        let mut orch = Orchestrator::new();
        let coder = Uuid::new_v4();
        let designer = Uuid::new_v4();
        orch.register_agent(coder, vec!["code".to_string(), "test".to_string()]);
        orch.register_agent(designer, vec!["design".to_string(), "css".to_string()]);

        let mut task = orch.decompose(
            "Full stack",
            vec![
                ("Backend", vec!["code".to_string()], 50),
                ("UI Design", vec!["design".to_string(), "css".to_string()], 40),
            ],
        );

        let assigned = orch.assign(&mut task);
        assert_eq!(assigned, 2);

        // Backend assigned to coder
        let backend_id = task.subtasks[0].id;
        assert_eq!(task.assignments[&backend_id], coder);
        assert_eq!(task.subtasks[0].status, SubTaskStatus::Assigned);

        // UI assigned to designer
        let ui_id = task.subtasks[1].id;
        assert_eq!(task.assignments[&ui_id], designer);
        assert_eq!(task.subtasks[1].status, SubTaskStatus::Assigned);
    }

    #[test]
    fn unmatched_subtask_stays_pending() {
        let mut orch = Orchestrator::new();
        let coder = Uuid::new_v4();
        orch.register_agent(coder, vec!["code".to_string()]);

        let mut task = orch.decompose(
            "Needs design",
            vec![("UI Design", vec!["design".to_string()], 40)],
        );

        let assigned = orch.assign(&mut task);
        assert_eq!(assigned, 0);
        assert_eq!(task.subtasks[0].status, SubTaskStatus::Pending);
        assert!(task.assignments.is_empty());
    }

    #[test]
    fn complete_and_fail_subtasks() {
        let orch = Orchestrator::new();
        let mut task = orch.decompose(
            "Test lifecycle",
            vec![
                ("Step A", vec![], 10),
                ("Step B", vec![], 10),
                ("Step C", vec![], 10),
            ],
        );

        let a_id = task.subtasks[0].id;
        let b_id = task.subtasks[1].id;
        let c_id = task.subtasks[2].id;

        assert!(!orch.is_complete(&task));

        assert!(orch.complete_subtask(&mut task, a_id));
        assert_eq!(task.subtasks[0].status, SubTaskStatus::Completed);

        assert!(orch.fail_subtask(&mut task, b_id));
        assert_eq!(task.subtasks[1].status, SubTaskStatus::Failed);

        // Not complete — b is failed, c is pending
        assert!(!orch.is_complete(&task));

        assert!(orch.complete_subtask(&mut task, b_id)); // override to completed
        assert!(orch.complete_subtask(&mut task, c_id));
        assert!(orch.is_complete(&task));
    }

    #[test]
    fn complete_nonexistent_subtask_returns_false() {
        let orch = Orchestrator::new();
        let mut task = orch.decompose("Empty", vec![]);
        assert!(!orch.complete_subtask(&mut task, Uuid::new_v4()));
        assert!(!orch.fail_subtask(&mut task, Uuid::new_v4()));
    }

    #[test]
    fn is_complete_false_for_empty_task() {
        let orch = Orchestrator::new();
        let task = orch.decompose("No subtasks", vec![]);
        assert!(!orch.is_complete(&task));
    }

    #[test]
    fn multi_capability_requirement() {
        let mut orch = Orchestrator::new();
        let generalist = Uuid::new_v4();
        let specialist = Uuid::new_v4();
        orch.register_agent(generalist, vec!["code".to_string(), "test".to_string(), "deploy".to_string()]);
        orch.register_agent(specialist, vec!["code".to_string()]);

        let mut task = orch.decompose(
            "Complex",
            vec![("Full pipeline", vec!["code".to_string(), "test".to_string(), "deploy".to_string()], 100)],
        );

        let assigned = orch.assign(&mut task);
        assert_eq!(assigned, 1);
        // Should be assigned to generalist (has all 3 caps), not specialist
        let sub_id = task.subtasks[0].id;
        assert_eq!(task.assignments[&sub_id], generalist);
    }
}
