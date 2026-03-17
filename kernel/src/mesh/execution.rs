//! Distributed execution — split work across mesh instances.

use super::MeshError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Status of a task or sub-task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Assigned,
    Running,
    Complete,
    Failed,
}

/// A unit of work assigned to a specific peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: Uuid,
    pub description: String,
    pub assigned_peer: Uuid,
    pub status: TaskStatus,
    pub result: Option<serde_json::Value>,
}

/// A composite task broken into sub-tasks distributed across the mesh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedTask {
    pub id: Uuid,
    pub description: String,
    pub sub_tasks: Vec<SubTask>,
    pub coordinator_peer: Uuid,
    pub status: TaskStatus,
}

/// Coordinates distributed task execution across mesh peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedExecutor {
    local_peer_id: Uuid,
    tasks: HashMap<Uuid, DistributedTask>,
}

impl DistributedExecutor {
    /// Create a new executor for the local peer.
    pub fn new(local_peer_id: Uuid) -> Self {
        Self {
            local_peer_id,
            tasks: HashMap::new(),
        }
    }

    /// Create and register a distributed task, assigning sub-tasks to the given
    /// peer-description pairs.
    pub fn distribute_task(
        &mut self,
        description: &str,
        assignments: Vec<(Uuid, String)>,
    ) -> Result<DistributedTask, MeshError> {
        if assignments.is_empty() {
            return Err(MeshError::TaskFailed(
                "no sub-task assignments provided".into(),
            ));
        }

        let sub_tasks: Vec<SubTask> = assignments
            .into_iter()
            .map(|(peer_id, desc)| SubTask {
                id: Uuid::new_v4(),
                description: desc,
                assigned_peer: peer_id,
                status: TaskStatus::Assigned,
                result: None,
            })
            .collect();

        let task = DistributedTask {
            id: Uuid::new_v4(),
            description: description.to_string(),
            sub_tasks,
            coordinator_peer: self.local_peer_id,
            status: TaskStatus::Running,
        };

        self.tasks.insert(task.id, task.clone());
        Ok(task)
    }

    /// Record the result for a sub-task.
    pub fn collect_results(
        &mut self,
        task_id: &Uuid,
        sub_task_id: &Uuid,
        result: serde_json::Value,
    ) -> Result<(), MeshError> {
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| MeshError::TaskFailed(format!("task {} not found", task_id)))?;

        let sub = task
            .sub_tasks
            .iter_mut()
            .find(|s| s.id == *sub_task_id)
            .ok_or_else(|| MeshError::TaskFailed(format!("sub-task {} not found", sub_task_id)))?;

        sub.result = Some(result);
        sub.status = TaskStatus::Complete;

        // Check if all sub-tasks are done
        let all_done = task
            .sub_tasks
            .iter()
            .all(|s| s.status == TaskStatus::Complete || s.status == TaskStatus::Failed);
        if all_done {
            let any_failed = task
                .sub_tasks
                .iter()
                .any(|s| s.status == TaskStatus::Failed);
            task.status = if any_failed {
                TaskStatus::Failed
            } else {
                TaskStatus::Complete
            };
        }

        Ok(())
    }

    /// Merge the results of all completed sub-tasks into a single JSON array.
    pub fn merge_results(&self, task_id: &Uuid) -> Result<serde_json::Value, MeshError> {
        let task = self
            .tasks
            .get(task_id)
            .ok_or_else(|| MeshError::TaskFailed(format!("task {} not found", task_id)))?;

        let results: Vec<serde_json::Value> = task
            .sub_tasks
            .iter()
            .filter_map(|s| s.result.clone())
            .collect();

        Ok(serde_json::json!({
            "task_id": task.id.to_string(),
            "description": task.description,
            "status": task.status,
            "results": results,
            "total_sub_tasks": task.sub_tasks.len(),
            "completed": task.sub_tasks.iter().filter(|s| s.status == TaskStatus::Complete).count(),
        }))
    }

    /// Retrieve a task by id.
    pub fn get_task(&self, task_id: &Uuid) -> Option<&DistributedTask> {
        self.tasks.get(task_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distribute_and_collect() {
        let local = Uuid::new_v4();
        let peer_a = Uuid::new_v4();
        let peer_b = Uuid::new_v4();

        let mut exec = DistributedExecutor::new(local);

        let task = exec
            .distribute_task(
                "analyze dataset",
                vec![
                    (peer_a, "partition A".into()),
                    (peer_b, "partition B".into()),
                ],
            )
            .unwrap();

        assert_eq!(task.sub_tasks.len(), 2);
        assert_eq!(task.status, TaskStatus::Running);

        let sub_a = task.sub_tasks[0].id;
        let sub_b = task.sub_tasks[1].id;

        exec.collect_results(&task.id, &sub_a, serde_json::json!({"count": 42}))
            .unwrap();
        // Still running — sub_b not done
        assert_eq!(exec.get_task(&task.id).unwrap().status, TaskStatus::Running);

        exec.collect_results(&task.id, &sub_b, serde_json::json!({"count": 58}))
            .unwrap();
        assert_eq!(
            exec.get_task(&task.id).unwrap().status,
            TaskStatus::Complete
        );
    }

    #[test]
    fn merge_results_produces_summary() {
        let local = Uuid::new_v4();
        let peer = Uuid::new_v4();
        let mut exec = DistributedExecutor::new(local);

        let task = exec
            .distribute_task("job", vec![(peer, "work".into())])
            .unwrap();
        let sub_id = task.sub_tasks[0].id;

        exec.collect_results(&task.id, &sub_id, serde_json::json!("done"))
            .unwrap();

        let merged = exec.merge_results(&task.id).unwrap();
        assert_eq!(merged["completed"], 1);
        assert_eq!(merged["results"][0], "done");
    }

    #[test]
    fn empty_assignments_rejected() {
        let local = Uuid::new_v4();
        let mut exec = DistributedExecutor::new(local);
        assert!(exec.distribute_task("noop", vec![]).is_err());
    }

    #[test]
    fn collect_unknown_task_fails() {
        let local = Uuid::new_v4();
        let mut exec = DistributedExecutor::new(local);
        let result = exec.collect_results(&Uuid::new_v4(), &Uuid::new_v4(), serde_json::json!(1));
        assert!(result.is_err());
    }

    #[test]
    fn collect_unknown_subtask_fails() {
        let local = Uuid::new_v4();
        let peer = Uuid::new_v4();
        let mut exec = DistributedExecutor::new(local);
        let task = exec.distribute_task("t", vec![(peer, "s".into())]).unwrap();
        let result = exec.collect_results(&task.id, &Uuid::new_v4(), serde_json::json!(1));
        assert!(result.is_err());
    }

    #[test]
    fn distributed_task_serde_roundtrip() {
        let task = DistributedTask {
            id: Uuid::new_v4(),
            description: "test".into(),
            sub_tasks: vec![SubTask {
                id: Uuid::new_v4(),
                description: "sub".into(),
                assigned_peer: Uuid::new_v4(),
                status: TaskStatus::Pending,
                result: None,
            }],
            coordinator_peer: Uuid::new_v4(),
            status: TaskStatus::Pending,
        };
        let json = serde_json::to_string(&task).unwrap();
        let back: DistributedTask = serde_json::from_str(&json).unwrap();
        assert_eq!(back.description, "test");
    }
}
