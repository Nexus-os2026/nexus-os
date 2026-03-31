//! Bridge between A2A tasks and the Nexus OS cognitive loop.
//!
//! When an external agent sends a task via the A2A protocol, the bridge:
//! 1. Matches the task to the best internal agent based on skill tags
//! 2. Creates a governed `A2ATask` with fuel/capability context
//! 3. Tracks task lifecycle (Submitted → Working → Completed/Failed)
//!
//! The bridge does NOT execute the agent — it prepares the task for the
//! Supervisor to schedule and run.

use crate::server::SkillRegistry;
use crate::types::{
    A2ATask, Artifact, GovernanceContext, MessagePart, MessageRole, TaskMessage, TaskPayload,
    TaskStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Error from bridge operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum BridgeError {
    #[error("no agent found for tags: {tags:?}")]
    NoAgentFound { tags: Vec<String> },

    #[error("task '{id}' not found")]
    TaskNotFound { id: String },

    #[error("invalid state transition: {from:?} -> {to:?}")]
    InvalidTransition { from: TaskStatus, to: TaskStatus },

    #[error("task '{id}' has already completed")]
    AlreadyTerminal { id: String },
}

/// A routed task — an A2A task paired with the agent selected to handle it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutedTask {
    /// The A2A task.
    pub task: A2ATask,
    /// Name of the Nexus agent assigned to this task.
    pub assigned_agent: String,
    /// Match score (number of tag overlaps).
    pub match_score: usize,
}

/// The A2A bridge routes incoming tasks to the correct internal agent.
pub struct A2aBridge {
    /// Active tasks keyed by task ID.
    tasks: HashMap<String, RoutedTask>,
    /// Counter for tasks processed.
    tasks_processed: u64,
}

impl A2aBridge {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            tasks_processed: 0,
        }
    }

    /// Route an incoming task to the best matching agent.
    ///
    /// Scoring: for each registered agent, count how many of the requested
    /// `tags` appear in that agent's skill tags.  The agent with the highest
    /// score wins.  Ties are broken alphabetically for determinism.
    pub fn route_task(
        &mut self,
        registry: &SkillRegistry,
        sender: &str,
        message: &str,
        tags: &[&str],
        fuel_budget: u64,
    ) -> Result<RoutedTask, BridgeError> {
        let mut best: Option<(String, usize)> = None;

        for agent in registry.registered_agents() {
            if let Some(skills) = registry.agent_skills(&agent.name) {
                let score: usize = skills
                    .iter()
                    .flat_map(|s| s.tags.iter())
                    .filter(|t| tags.contains(&t.as_str()))
                    .count();
                if score > 0 {
                    let is_better = match &best {
                        Some((_, best_score)) => {
                            score > *best_score
                                || (score == *best_score && agent.name < best.as_ref().unwrap().0)
                        }
                        None => true,
                    };
                    if is_better {
                        best = Some((agent.name.clone(), score));
                    }
                }
            }
        }

        let (agent_name, score) = best.ok_or_else(|| BridgeError::NoAgentFound {
            tags: tags.iter().map(|t| (*t).to_string()).collect(),
        })?;

        let payload = TaskPayload {
            message: TaskMessage {
                role: MessageRole::User,
                parts: vec![MessagePart::Text {
                    text: message.to_string(),
                }],
                metadata: None,
            },
            metadata: None,
        };

        let mut task = A2ATask::new(sender, &agent_name, payload);
        task.governance = Some(GovernanceContext {
            autonomy_level: 2,
            fuel_budget,
            fuel_consumed: 0,
            required_capabilities: tags.iter().map(|t| (*t).to_string()).collect(),
            hitl_approved: false,
            audit_hash: None,
        });

        let routed = RoutedTask {
            task: task.clone(),
            assigned_agent: agent_name,
            match_score: score,
        };

        self.tasks.insert(task.id.clone(), routed.clone());
        self.tasks_processed += 1;

        Ok(routed)
    }

    /// Get a routed task by ID.
    pub fn get_task(&self, task_id: &str) -> Option<&RoutedTask> {
        self.tasks.get(task_id)
    }

    /// Transition a task to a new status.
    pub fn transition_task(
        &mut self,
        task_id: &str,
        new_status: TaskStatus,
    ) -> Result<(), BridgeError> {
        let routed = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| BridgeError::TaskNotFound {
                id: task_id.to_string(),
            })?;

        let old = routed.task.status;
        if !routed.task.transition_to(new_status) {
            return Err(BridgeError::InvalidTransition {
                from: old,
                to: new_status,
            });
        }

        Ok(())
    }

    /// Complete a task with artifacts.
    pub fn complete_task(
        &mut self,
        task_id: &str,
        artifacts: Vec<Artifact>,
    ) -> Result<(), BridgeError> {
        let routed = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| BridgeError::TaskNotFound {
                id: task_id.to_string(),
            })?;

        if routed.task.status.is_terminal() {
            return Err(BridgeError::AlreadyTerminal {
                id: task_id.to_string(),
            });
        }

        // If still Submitted, transition through Working first
        if routed.task.status == TaskStatus::Submitted {
            routed.task.transition_to(TaskStatus::Working);
        }

        routed.task.artifacts = artifacts;
        routed.task.transition_to(TaskStatus::Completed);

        if let Some(ref mut gov) = routed.task.governance {
            gov.fuel_consumed = gov.fuel_budget / 2; // rough estimate
        }

        Ok(())
    }

    /// List all active (non-terminal) tasks.
    pub fn active_tasks(&self) -> Vec<&RoutedTask> {
        self.tasks
            .values()
            .filter(|rt| !rt.task.status.is_terminal())
            .collect()
    }

    /// Total tasks processed.
    pub fn tasks_processed(&self) -> u64 {
        self.tasks_processed
    }

    /// Total tasks currently tracked.
    pub fn total_tasks(&self) -> usize {
        self.tasks.len()
    }
}

impl Default for A2aBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::SkillRegistry;
    use crate::types::AgentSkill;

    fn setup_registry() -> SkillRegistry {
        let mut reg = SkillRegistry::new("nexus", "http://localhost");
        reg.register_skills(
            "coder-agent",
            vec![AgentSkill {
                id: "code-gen".to_string(),
                name: "Code Generation".to_string(),
                description: None,
                tags: vec![
                    "code".to_string(),
                    "generation".to_string(),
                    "programming".to_string(),
                ],
                input_modes: vec!["text/plain".to_string()],
                output_modes: vec!["text/plain".to_string()],
            }],
        );
        reg.register_skills(
            "web-agent",
            vec![AgentSkill {
                id: "web-search".to_string(),
                name: "Web Search".to_string(),
                description: None,
                tags: vec!["web".to_string(), "search".to_string()],
                input_modes: vec!["text/plain".to_string()],
                output_modes: vec!["application/json".to_string()],
            }],
        );
        reg
    }

    #[test]
    fn route_task_to_best_agent() {
        let reg = setup_registry();
        let mut bridge = A2aBridge::new();

        let routed = bridge
            .route_task(
                &reg,
                "external-agent",
                "Write a function",
                &["code", "generation"],
                5000,
            )
            .unwrap();

        assert_eq!(routed.assigned_agent, "coder-agent");
        assert_eq!(routed.match_score, 2);
        assert_eq!(routed.task.status, TaskStatus::Submitted);
        assert_eq!(routed.task.sender, "external-agent");
    }

    #[test]
    fn route_task_no_match() {
        let reg = setup_registry();
        let mut bridge = A2aBridge::new();

        let result = bridge.route_task(&reg, "caller", "do finance", &["finance"], 1000);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no agent found"));
    }

    #[test]
    fn task_lifecycle_through_bridge() {
        let reg = setup_registry();
        let mut bridge = A2aBridge::new();

        let routed = bridge
            .route_task(&reg, "caller", "search web", &["web"], 3000)
            .unwrap();
        let task_id = routed.task.id.clone();

        // Submitted → Working
        bridge
            .transition_task(&task_id, TaskStatus::Working)
            .unwrap();
        assert_eq!(
            bridge.get_task(&task_id).unwrap().task.status,
            TaskStatus::Working
        );

        // Working → Completed
        bridge
            .transition_task(&task_id, TaskStatus::Completed)
            .unwrap();
        assert_eq!(
            bridge.get_task(&task_id).unwrap().task.status,
            TaskStatus::Completed
        );
    }

    #[test]
    fn complete_task_with_artifacts() {
        let reg = setup_registry();
        let mut bridge = A2aBridge::new();

        let routed = bridge
            .route_task(&reg, "caller", "generate code", &["code"], 5000)
            .unwrap();
        let task_id = routed.task.id.clone();

        let artifacts = vec![Artifact {
            name: Some("output.py".to_string()),
            description: Some("Generated code".to_string()),
            parts: vec![MessagePart::Text {
                text: "def hello(): pass".to_string(),
            }],
            index: Some(0),
            last_chunk: Some(true),
            metadata: None,
        }];

        bridge.complete_task(&task_id, artifacts).unwrap();
        let task = &bridge.get_task(&task_id).unwrap().task;
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.artifacts.len(), 1);
        assert!(task.governance.as_ref().unwrap().fuel_consumed > 0);
    }

    #[test]
    fn cannot_complete_terminal_task() {
        let reg = setup_registry();
        let mut bridge = A2aBridge::new();

        let routed = bridge
            .route_task(&reg, "caller", "do work", &["code"], 1000)
            .unwrap();
        let task_id = routed.task.id.clone();

        bridge.complete_task(&task_id, vec![]).unwrap();
        let result = bridge.complete_task(&task_id, vec![]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("already completed"));
    }

    #[test]
    fn active_tasks_filters_terminal() {
        let reg = setup_registry();
        let mut bridge = A2aBridge::new();

        let r1 = bridge
            .route_task(&reg, "caller", "task 1", &["code"], 1000)
            .unwrap();
        let r2 = bridge
            .route_task(&reg, "caller", "task 2", &["web"], 1000)
            .unwrap();

        assert_eq!(bridge.active_tasks().len(), 2);

        bridge.complete_task(&r1.task.id, vec![]).unwrap();
        assert_eq!(bridge.active_tasks().len(), 1);
        assert_eq!(bridge.active_tasks()[0].task.id, r2.task.id);
    }

    #[test]
    fn tasks_processed_counter() {
        let reg = setup_registry();
        let mut bridge = A2aBridge::new();

        bridge.route_task(&reg, "c", "t1", &["code"], 100).unwrap();
        bridge.route_task(&reg, "c", "t2", &["web"], 100).unwrap();
        assert_eq!(bridge.tasks_processed(), 2);
        assert_eq!(bridge.total_tasks(), 2);
    }

    #[test]
    fn invalid_transition_rejected() {
        let reg = setup_registry();
        let mut bridge = A2aBridge::new();

        let routed = bridge
            .route_task(&reg, "caller", "task", &["code"], 1000)
            .unwrap();
        let task_id = routed.task.id.clone();

        // Submitted → Completed is not allowed (must go through Working)
        let result = bridge.transition_task(&task_id, TaskStatus::Completed);
        assert!(result.is_err());
    }

    #[test]
    fn task_not_found_error() {
        let mut bridge = A2aBridge::new();
        let result = bridge.transition_task("nonexistent", TaskStatus::Working);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn governance_context_attached() {
        let reg = setup_registry();
        let mut bridge = A2aBridge::new();

        let routed = bridge
            .route_task(&reg, "external", "work", &["code"], 8000)
            .unwrap();

        let gov = routed.task.governance.as_ref().unwrap();
        assert_eq!(gov.fuel_budget, 8000);
        assert_eq!(gov.fuel_consumed, 0);
        assert!(!gov.hitl_approved);
        assert_eq!(gov.required_capabilities, vec!["code"]);
    }
}
