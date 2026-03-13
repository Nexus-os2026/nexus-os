use crate::audit::{AuditTrail, EventType};
use crate::autonomy::{AutonomyGuard, AutonomyLevel};
use crate::consent::{
    ApprovalQueue, ApprovalRequest, ConsentPolicyEngine, ConsentRuntime, GovernedOperation,
};
use crate::errors::AgentError;
use crate::lifecycle::{transition_state, AgentState};
use crate::orchestration::messaging::{AgentId, TeamId, TeamMessage, TeamMessageBus};
use crate::orchestration::roles::AgentRole;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamAgent {
    pub agent_id: AgentId,
    pub role: AgentRole,
    pub state: AgentState,
    pub capabilities: Vec<String>,
    pub fuel_allocation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Team {
    pub team_id: TeamId,
    pub agents: Vec<TeamAgent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAssignment {
    pub team_id: TeamId,
    pub role: AgentRole,
    pub agent_id: AgentId,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamTaskPlan {
    pub team_id: TeamId,
    pub task: String,
    pub assignments: Vec<TaskAssignment>,
    pub message_trace: Vec<TeamMessage>,
}

#[derive(Debug, Default)]
pub struct Orchestrator {
    teams: HashMap<TeamId, Team>,
    bus: TeamMessageBus,
    audit_trail: AuditTrail,
    autonomy_guard: AutonomyGuard,
    consent_runtime: ConsentRuntime,
}

impl Orchestrator {
    pub fn new() -> Self {
        Self::with_autonomy_level(AutonomyLevel::L0)
    }

    pub fn with_autonomy_level(level: AutonomyLevel) -> Self {
        Self {
            teams: HashMap::new(),
            bus: TeamMessageBus::new(),
            audit_trail: AuditTrail::new(),
            autonomy_guard: AutonomyGuard::new(level),
            consent_runtime: ConsentRuntime::new(
                ConsentPolicyEngine::default(),
                ApprovalQueue::in_memory(),
                "orchestrator".to_string(),
            ),
        }
    }

    pub fn create_team(&mut self, roles: &[AgentRole]) -> Result<TeamId, AgentError> {
        let payload = serde_json::to_vec(
            &roles
                .iter()
                .map(|role| role.canonical_rank())
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default();
        self.consent_runtime.enforce_operation(
            GovernedOperation::MultiAgentOrchestrate,
            Uuid::nil(),
            payload.as_slice(),
            &mut self.audit_trail,
        )?;
        self.autonomy_guard
            .require_multi_agent(Uuid::nil(), &mut self.audit_trail)?;
        if roles.is_empty() {
            return Err(AgentError::SupervisorError(
                "team must include at least one role".to_string(),
            ));
        }

        let mut unique_roles = roles.to_vec();
        unique_roles.sort_by_key(|role| role.canonical_rank());
        unique_roles.dedup();

        let team_id = Uuid::new_v4();
        let mut agents = Vec::new();

        for role in unique_roles {
            let profile = role.default_profile();
            let agent_id = Uuid::new_v4();
            let mut state = AgentState::Created;
            state = transition_state(state, AgentState::Starting)?;
            state = transition_state(state, AgentState::Running)?;

            agents.push(TeamAgent {
                agent_id,
                role,
                state,
                capabilities: profile.default_capabilities,
                fuel_allocation: profile.default_fuel_allocation,
            });
        }

        let team = Team { team_id, agents };

        self.audit_trail.append_event(
            team_id,
            EventType::StateChange,
            json!({
                "event": "team_created",
                "agent_count": team.agents.len(),
            }),
        )?;

        self.teams.insert(team_id, team);
        Ok(team_id)
    }

    pub fn assign_task(&mut self, team_id: TeamId, task: &str) -> Result<TeamTaskPlan, AgentError> {
        let team =
            self.teams.get(&team_id).cloned().ok_or_else(|| {
                AgentError::SupervisorError(format!("team '{team_id}' not found"))
            })?;

        let mut ordered_agents = team.agents.clone();
        ordered_agents.sort_by_key(|agent| agent.role.canonical_rank());

        let mut assignments = Vec::new();
        let mut message_trace = Vec::new();

        let mut previous_agent = Uuid::nil();
        for agent in ordered_agents {
            let description = build_role_task(agent.role, task);
            let assignment = TaskAssignment {
                team_id,
                role: agent.role,
                agent_id: agent.agent_id,
                description,
            };

            let message = self.bus.send(
                team_id,
                previous_agent,
                agent.agent_id,
                assignment.description.as_str(),
            )?;
            message_trace.push(message);
            previous_agent = agent.agent_id;

            assignments.push(assignment);
        }

        self.audit_trail.append_event(
            team_id,
            EventType::ToolCall,
            json!({
                "event": "team_task_assigned",
                "task": task,
                "assignment_count": assignments.len(),
            }),
        )?;

        Ok(TeamTaskPlan {
            team_id,
            task: task.to_string(),
            assignments,
            message_trace,
        })
    }

    pub fn team(&self, team_id: TeamId) -> Option<&Team> {
        self.teams.get(&team_id)
    }

    pub fn bus(&self) -> &TeamMessageBus {
        &self.bus
    }

    pub fn audit_trail(&self) -> &AuditTrail {
        &self.audit_trail
    }

    pub fn autonomy_guard(&self) -> &AutonomyGuard {
        &self.autonomy_guard
    }

    pub fn approve_consent(
        &mut self,
        request_id: &str,
        approver_id: &str,
    ) -> Result<(), AgentError> {
        self.consent_runtime
            .approve(request_id, approver_id, &mut self.audit_trail)?;
        Ok(())
    }

    pub fn pending_consent_requests(&self) -> Vec<ApprovalRequest> {
        self.consent_runtime.pending_requests()
    }
}

fn build_role_task(role: AgentRole, task: &str) -> String {
    match role {
        AgentRole::Researcher => format!("research task for: {task}"),
        AgentRole::Writer => format!("write draft for: {task}"),
        AgentRole::Reviewer => format!("review draft for: {task}"),
        AgentRole::Publisher => format!("publish approved output for: {task}"),
        AgentRole::Analyst => format!("analyze performance for: {task}"),
    }
}

pub fn canonical_merge_messages(mut messages: Vec<TeamMessage>) -> Vec<TeamMessage> {
    messages.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then_with(|| left.message_id.cmp(&right.message_id))
    });
    messages
}

#[cfg(test)]
mod tests {
    use super::Orchestrator;
    use crate::autonomy::AutonomyLevel;
    use crate::errors::AgentError;
    use crate::orchestration::roles::AgentRole;

    fn setup_orchestrator() -> Orchestrator {
        let mut orchestrator = Orchestrator::with_autonomy_level(AutonomyLevel::L2);
        // Configure allowed approvers for operations used in tests
        let policy = orchestrator.consent_runtime.policy_engine_mut();
        policy.set_policy(
            crate::consent::GovernedOperation::MultiAgentOrchestrate,
            crate::consent::HitlTier::Tier2,
            vec!["approver.a".to_string()],
        );
        orchestrator
    }

    #[test]
    fn test_team_creation() {
        let mut orchestrator = setup_orchestrator();
        let initial = orchestrator.create_team(&[
            AgentRole::Researcher,
            AgentRole::Writer,
            AgentRole::Publisher,
        ]);
        let request_id = match initial {
            Err(AgentError::ApprovalRequired { request_id }) => request_id,
            other => panic!("expected approval required, got: {other:?}"),
        };
        orchestrator
            .approve_consent(request_id.as_str(), "approver.a")
            .expect("approval should succeed");

        let team_id = orchestrator.create_team(&[
            AgentRole::Researcher,
            AgentRole::Writer,
            AgentRole::Publisher,
        ]);
        assert!(team_id.is_ok());

        if let Ok(team_id) = team_id {
            let team = orchestrator.team(team_id);
            assert!(team.is_some());
            if let Some(team) = team {
                assert_eq!(team.agents.len(), 3);
                assert!(team
                    .agents
                    .iter()
                    .all(|agent| agent.state == crate::lifecycle::AgentState::Running));
            }
        }
    }

    #[test]
    fn test_task_distribution() {
        let mut orchestrator = setup_orchestrator();
        let initial = orchestrator.create_team(&[
            AgentRole::Researcher,
            AgentRole::Writer,
            AgentRole::Publisher,
        ]);
        let request_id = match initial {
            Err(AgentError::ApprovalRequired { request_id }) => request_id,
            other => panic!("expected approval required, got: {other:?}"),
        };
        orchestrator
            .approve_consent(request_id.as_str(), "approver.a")
            .expect("approval should succeed");

        let team_id = orchestrator.create_team(&[
            AgentRole::Researcher,
            AgentRole::Writer,
            AgentRole::Publisher,
        ]);
        assert!(team_id.is_ok());

        if let Ok(team_id) = team_id {
            let plan = orchestrator.assign_task(team_id, "write blog post about Rust");
            assert!(plan.is_ok());

            if let Ok(plan) = plan {
                assert_eq!(plan.assignments.len(), 3);
                assert_eq!(plan.assignments[0].role, AgentRole::Researcher);
                assert_eq!(plan.assignments[1].role, AgentRole::Writer);
                assert_eq!(plan.assignments[2].role, AgentRole::Publisher);

                assert!(plan.assignments[0].description.contains("research"));
                assert!(plan.assignments[1].description.contains("write"));
                assert!(plan.assignments[2].description.contains("publish"));

                assert_eq!(plan.message_trace.len(), 3);
                assert!(plan.message_trace[0].sequence < plan.message_trace[1].sequence);
                assert!(plan.message_trace[1].sequence < plan.message_trace[2].sequence);
            }
        }
    }
}
