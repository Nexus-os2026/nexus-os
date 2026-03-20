//! Workspace manager — CRUD operations with access control enforcement.

use crate::error::TenancyError;
use crate::policy::WorkspacePolicy;
use crate::usage::{FuelLedger, WorkspaceUsage};
use crate::workspace::{Workspace, WorkspaceConfig, WorkspaceMember, WorkspaceRole};
use chrono::Utc;
use std::collections::HashMap;

/// Central manager for all workspaces in a Nexus OS deployment.
#[derive(Debug)]
pub struct WorkspaceManager {
    workspaces: HashMap<String, Workspace>,
    policies: HashMap<String, WorkspacePolicy>,
    fuel_ledgers: HashMap<String, FuelLedger>,
    /// Agents deployed per workspace: workspace_id → set of agent DIDs.
    agents: HashMap<String, Vec<String>>,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        Self {
            workspaces: HashMap::new(),
            policies: HashMap::new(),
            fuel_ledgers: HashMap::new(),
            agents: HashMap::new(),
        }
    }

    // ── Workspace CRUD ─────────────────────────────────────────────────

    /// Create a new workspace from the given configuration.
    pub fn create_workspace(&mut self, config: WorkspaceConfig) -> Result<Workspace, TenancyError> {
        // Check name uniqueness.
        if self.workspaces.values().any(|ws| ws.name == config.name) {
            return Err(TenancyError::WorkspaceAlreadyExists(config.name));
        }

        let ws = config.into_workspace();

        // Initialize default policy.
        let policy = WorkspacePolicy {
            max_autonomy_level: ws.max_autonomy_level,
            fuel_budget_daily: ws.fuel_budget_daily,
            agent_limit: ws.agent_limit,
            allowed_providers: ws.allowed_providers.clone(),
            ..WorkspacePolicy::default()
        };

        // Initialize fuel ledger.
        let ledger = FuelLedger::new(ws.id.clone(), ws.fuel_budget_daily);

        self.policies.insert(ws.id.clone(), policy);
        self.fuel_ledgers.insert(ws.id.clone(), ledger);
        self.agents.insert(ws.id.clone(), Vec::new());
        self.workspaces.insert(ws.id.clone(), ws.clone());

        tracing::info!(workspace_id = %ws.id, name = %ws.name, "Workspace created");
        Ok(ws)
    }

    /// Get a workspace by ID.
    pub fn get_workspace(&self, workspace_id: &str) -> Result<&Workspace, TenancyError> {
        self.workspaces
            .get(workspace_id)
            .ok_or_else(|| TenancyError::WorkspaceNotFound(workspace_id.to_string()))
    }

    /// List all workspaces.
    pub fn list_workspaces(&self) -> Vec<&Workspace> {
        self.workspaces.values().collect()
    }

    /// Delete a workspace. Returns the removed workspace.
    pub fn delete_workspace(&mut self, workspace_id: &str) -> Result<Workspace, TenancyError> {
        let ws = self
            .workspaces
            .remove(workspace_id)
            .ok_or_else(|| TenancyError::WorkspaceNotFound(workspace_id.to_string()))?;

        self.policies.remove(workspace_id);
        self.fuel_ledgers.remove(workspace_id);
        self.agents.remove(workspace_id);

        tracing::info!(workspace_id = %workspace_id, name = %ws.name, "Workspace deleted");
        Ok(ws)
    }

    // ── Member management ──────────────────────────────────────────────

    /// Add a member to a workspace.
    pub fn add_member(
        &mut self,
        workspace_id: &str,
        user_id: &str,
        role: WorkspaceRole,
    ) -> Result<(), TenancyError> {
        let ws = self
            .workspaces
            .get_mut(workspace_id)
            .ok_or_else(|| TenancyError::WorkspaceNotFound(workspace_id.to_string()))?;

        if ws.has_member(user_id) {
            return Err(TenancyError::MemberAlreadyExists {
                workspace_id: workspace_id.to_string(),
                user_id: user_id.to_string(),
            });
        }

        ws.members.push(WorkspaceMember {
            user_id: user_id.to_string(),
            role,
            added_at: Utc::now(),
        });

        tracing::info!(
            workspace_id = %workspace_id,
            user_id = %user_id,
            role = %role,
            "Member added to workspace"
        );
        Ok(())
    }

    /// Remove a member from a workspace. Cannot remove the last admin.
    pub fn remove_member(&mut self, workspace_id: &str, user_id: &str) -> Result<(), TenancyError> {
        let ws = self
            .workspaces
            .get_mut(workspace_id)
            .ok_or_else(|| TenancyError::WorkspaceNotFound(workspace_id.to_string()))?;

        // Find the member.
        let idx = ws
            .members
            .iter()
            .position(|m| m.user_id == user_id)
            .ok_or_else(|| TenancyError::MemberNotFound {
                workspace_id: workspace_id.to_string(),
                user_id: user_id.to_string(),
            })?;

        // Don't remove last admin.
        if ws.members[idx].role == WorkspaceRole::Admin {
            let admin_count = ws
                .members
                .iter()
                .filter(|m| m.role == WorkspaceRole::Admin)
                .count();
            if admin_count <= 1 {
                return Err(TenancyError::LastAdmin(workspace_id.to_string()));
            }
        }

        ws.members.remove(idx);
        tracing::info!(
            workspace_id = %workspace_id,
            user_id = %user_id,
            "Member removed from workspace"
        );
        Ok(())
    }

    /// Update a member's role within a workspace.
    pub fn update_member_role(
        &mut self,
        workspace_id: &str,
        user_id: &str,
        new_role: WorkspaceRole,
    ) -> Result<(), TenancyError> {
        let ws = self
            .workspaces
            .get_mut(workspace_id)
            .ok_or_else(|| TenancyError::WorkspaceNotFound(workspace_id.to_string()))?;

        // Find the member index and current role.
        let idx = ws
            .members
            .iter()
            .position(|m| m.user_id == user_id)
            .ok_or_else(|| TenancyError::MemberNotFound {
                workspace_id: workspace_id.to_string(),
                user_id: user_id.to_string(),
            })?;

        let current_role = ws.members[idx].role;

        // Prevent demoting the last admin.
        if current_role == WorkspaceRole::Admin && new_role != WorkspaceRole::Admin {
            let admin_count = ws
                .members
                .iter()
                .filter(|m| m.role == WorkspaceRole::Admin)
                .count();
            if admin_count <= 1 {
                return Err(TenancyError::LastAdmin(workspace_id.to_string()));
            }
        }

        ws.members[idx].role = new_role;
        Ok(())
    }

    // ── Policy management ──────────────────────────────────────────────

    /// Get the policy for a workspace.
    pub fn get_policy(&self, workspace_id: &str) -> Result<&WorkspacePolicy, TenancyError> {
        self.policies
            .get(workspace_id)
            .ok_or_else(|| TenancyError::WorkspaceNotFound(workspace_id.to_string()))
    }

    /// Update the policy for a workspace.
    pub fn set_policy(
        &mut self,
        workspace_id: &str,
        policy: WorkspacePolicy,
    ) -> Result<(), TenancyError> {
        if !self.workspaces.contains_key(workspace_id) {
            return Err(TenancyError::WorkspaceNotFound(workspace_id.to_string()));
        }
        policy.validate()?;

        // Sync workspace fields with policy.
        if let Some(ws) = self.workspaces.get_mut(workspace_id) {
            ws.max_autonomy_level = policy.max_autonomy_level;
            ws.fuel_budget_daily = policy.fuel_budget_daily;
            ws.agent_limit = policy.agent_limit;
            ws.allowed_providers = policy.allowed_providers.clone();
        }

        // Update fuel ledger budget.
        if let Some(ledger) = self.fuel_ledgers.get_mut(workspace_id) {
            ledger.budget_daily = policy.fuel_budget_daily;
        }

        self.policies.insert(workspace_id.to_string(), policy);
        tracing::info!(workspace_id = %workspace_id, "Workspace policy updated");
        Ok(())
    }

    // ── Agent management ───────────────────────────────────────────────

    /// Register an agent deployment in a workspace.
    pub fn deploy_agent(
        &mut self,
        workspace_id: &str,
        agent_did: &str,
    ) -> Result<(), TenancyError> {
        let ws = self
            .workspaces
            .get(workspace_id)
            .ok_or_else(|| TenancyError::WorkspaceNotFound(workspace_id.to_string()))?;

        let agents = self.agents.entry(workspace_id.to_string()).or_default();
        if agents.len() as u32 >= ws.agent_limit {
            return Err(TenancyError::AgentLimitReached {
                workspace_id: workspace_id.to_string(),
                current: agents.len() as u32,
                limit: ws.agent_limit,
            });
        }

        agents.push(agent_did.to_string());
        Ok(())
    }

    /// Remove an agent deployment from a workspace.
    pub fn undeploy_agent(&mut self, workspace_id: &str, agent_did: &str) {
        if let Some(agents) = self.agents.get_mut(workspace_id) {
            agents.retain(|a| a != agent_did);
        }
    }

    /// List agents deployed in a workspace.
    pub fn list_agents(&self, workspace_id: &str) -> Vec<&str> {
        self.agents
            .get(workspace_id)
            .map(|a| a.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    // ── Fuel ───────────────────────────────────────────────────────────

    /// Try to consume fuel for an agent in a workspace.
    pub fn consume_fuel(
        &mut self,
        workspace_id: &str,
        agent_did: &str,
        amount: u64,
    ) -> Result<(), TenancyError> {
        // Check single-action limit.
        if let Some(policy) = self.policies.get(workspace_id) {
            if !policy.allows_fuel_cost(amount) {
                return Err(TenancyError::FuelBudgetExhausted {
                    workspace_id: workspace_id.to_string(),
                    used: amount,
                    budget: policy.max_single_action_fuel,
                });
            }
        }

        let ledger = self
            .fuel_ledgers
            .get_mut(workspace_id)
            .ok_or_else(|| TenancyError::WorkspaceNotFound(workspace_id.to_string()))?;

        ledger.try_consume(agent_did, amount)
    }

    // ── Usage ──────────────────────────────────────────────────────────

    /// Get a usage snapshot for a workspace.
    pub fn get_usage(&self, workspace_id: &str) -> Result<WorkspaceUsage, TenancyError> {
        let ws = self
            .workspaces
            .get(workspace_id)
            .ok_or_else(|| TenancyError::WorkspaceNotFound(workspace_id.to_string()))?;

        let ledger = self
            .fuel_ledgers
            .get(workspace_id)
            .ok_or_else(|| TenancyError::WorkspaceNotFound(workspace_id.to_string()))?;

        let agents = self.agents.get(workspace_id);

        Ok(WorkspaceUsage {
            workspace_id: workspace_id.to_string(),
            captured_at: Utc::now(),
            fuel_used_today: ledger.consumed_today,
            fuel_budget_daily: ws.fuel_budget_daily,
            agents_deployed: agents.map(|a| a.len() as u32).unwrap_or(0),
            agent_limit: ws.agent_limit,
            member_count: ws.members.len() as u32,
            audit_entries: 0,      // Populated by caller from audit trail.
            llm_requests_today: 0, // Populated by caller from telemetry.
            llm_tokens_today: 0,   // Populated by caller from telemetry.
        })
    }

    // ── Access control ─────────────────────────────────────────────────

    /// Check if a user has at least the given role in a workspace.
    pub fn check_access(
        &self,
        workspace_id: &str,
        user_id: &str,
        required_role: WorkspaceRole,
    ) -> Result<(), TenancyError> {
        let ws = self
            .workspaces
            .get(workspace_id)
            .ok_or_else(|| TenancyError::WorkspaceNotFound(workspace_id.to_string()))?;

        let role = ws.member_role(user_id).ok_or_else(|| {
            TenancyError::AccessDenied(format!(
                "user '{user_id}' is not a member of workspace '{workspace_id}'"
            ))
        })?;

        if !role.satisfies(&required_role) {
            return Err(TenancyError::AccessDenied(format!(
                "user '{user_id}' has role {role} but {required_role} is required"
            )));
        }

        Ok(())
    }

    /// Check if an autonomy level is permitted in a workspace.
    pub fn check_autonomy_level(&self, workspace_id: &str, level: u8) -> Result<(), TenancyError> {
        let ws = self
            .workspaces
            .get(workspace_id)
            .ok_or_else(|| TenancyError::WorkspaceNotFound(workspace_id.to_string()))?;

        if level > ws.max_autonomy_level {
            return Err(TenancyError::AutonomyLevelExceeded {
                workspace_id: workspace_id.to_string(),
                requested: level,
                max: ws.max_autonomy_level,
            });
        }
        Ok(())
    }

    /// Check if an LLM provider is permitted in a workspace.
    pub fn check_provider(&self, workspace_id: &str, provider: &str) -> Result<(), TenancyError> {
        let ws = self
            .workspaces
            .get(workspace_id)
            .ok_or_else(|| TenancyError::WorkspaceNotFound(workspace_id.to_string()))?;

        if !ws.is_provider_allowed(provider) {
            return Err(TenancyError::ProviderNotAllowed {
                workspace_id: workspace_id.to_string(),
                provider: provider.to_string(),
            });
        }
        Ok(())
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eng_config() -> WorkspaceConfig {
        WorkspaceConfig {
            name: "Engineering".to_string(),
            admin_user_id: "alice".to_string(),
            agent_limit: Some(3),
            fuel_budget_daily: Some(10_000),
            max_autonomy_level: Some(3),
            allowed_providers: Some(vec!["claude".to_string()]),
            data_isolation: None,
        }
    }

    fn research_config() -> WorkspaceConfig {
        WorkspaceConfig {
            name: "Research".to_string(),
            admin_user_id: "charlie".to_string(),
            agent_limit: Some(10),
            fuel_budget_daily: Some(50_000),
            max_autonomy_level: Some(5),
            allowed_providers: None,
            data_isolation: None,
        }
    }

    #[test]
    fn create_workspace() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap();
        assert_eq!(ws.name, "Engineering");
        assert_eq!(ws.members.len(), 1);
        assert_eq!(ws.members[0].user_id, "alice");
        assert_eq!(ws.members[0].role, WorkspaceRole::Admin);
    }

    #[test]
    fn duplicate_workspace_name_fails() {
        let mut mgr = WorkspaceManager::new();
        mgr.create_workspace(eng_config()).unwrap();
        let err = mgr.create_workspace(eng_config()).unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn list_workspaces() {
        let mut mgr = WorkspaceManager::new();
        mgr.create_workspace(eng_config()).unwrap();
        mgr.create_workspace(research_config()).unwrap();
        assert_eq!(mgr.list_workspaces().len(), 2);
    }

    #[test]
    fn delete_workspace() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap();
        let deleted = mgr.delete_workspace(&ws.id).unwrap();
        assert_eq!(deleted.name, "Engineering");
        assert!(mgr.get_workspace(&ws.id).is_err());
    }

    #[test]
    fn add_and_remove_member() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap();
        let ws_id = ws.id.clone();

        mgr.add_member(&ws_id, "bob", WorkspaceRole::Operator)
            .unwrap();
        assert_eq!(mgr.get_workspace(&ws_id).unwrap().members.len(), 2);

        mgr.remove_member(&ws_id, "bob").unwrap();
        assert_eq!(mgr.get_workspace(&ws_id).unwrap().members.len(), 1);
    }

    #[test]
    fn cannot_add_duplicate_member() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap();
        let err = mgr
            .add_member(&ws.id, "alice", WorkspaceRole::Viewer)
            .unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn cannot_remove_last_admin() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap();
        let err = mgr.remove_member(&ws.id, "alice").unwrap_err();
        assert!(err.to_string().contains("last admin"));
    }

    #[test]
    fn cannot_demote_last_admin() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap();
        let err = mgr
            .update_member_role(&ws.id, "alice", WorkspaceRole::Viewer)
            .unwrap_err();
        assert!(err.to_string().contains("last admin"));
    }

    #[test]
    fn can_demote_admin_if_another_exists() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap();
        mgr.add_member(&ws.id, "bob", WorkspaceRole::Admin).unwrap();
        mgr.update_member_role(&ws.id, "alice", WorkspaceRole::Viewer)
            .unwrap();
        let ws = mgr.get_workspace(&ws.id).unwrap();
        assert_eq!(ws.member_role("alice"), Some(WorkspaceRole::Viewer));
    }

    #[test]
    fn access_control() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap();
        mgr.add_member(&ws.id, "bob", WorkspaceRole::Viewer)
            .unwrap();

        // Alice (Admin) can do anything.
        assert!(mgr
            .check_access(&ws.id, "alice", WorkspaceRole::Admin)
            .is_ok());

        // Bob (Viewer) cannot operate.
        assert!(mgr
            .check_access(&ws.id, "bob", WorkspaceRole::Operator)
            .is_err());

        // Bob (Viewer) can view.
        assert!(mgr
            .check_access(&ws.id, "bob", WorkspaceRole::Viewer)
            .is_ok());

        // Unknown user denied.
        assert!(mgr
            .check_access(&ws.id, "unknown", WorkspaceRole::Viewer)
            .is_err());
    }

    #[test]
    fn deploy_agent_within_limit() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap();

        mgr.deploy_agent(&ws.id, "agent-1").unwrap();
        mgr.deploy_agent(&ws.id, "agent-2").unwrap();
        mgr.deploy_agent(&ws.id, "agent-3").unwrap();

        // Limit is 3, so the 4th should fail.
        let err = mgr.deploy_agent(&ws.id, "agent-4").unwrap_err();
        assert!(err.to_string().contains("limit reached"));
    }

    #[test]
    fn undeploy_agent() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap();

        mgr.deploy_agent(&ws.id, "agent-1").unwrap();
        assert_eq!(mgr.list_agents(&ws.id).len(), 1);

        mgr.undeploy_agent(&ws.id, "agent-1");
        assert_eq!(mgr.list_agents(&ws.id).len(), 0);
    }

    #[test]
    fn consume_fuel() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap();

        mgr.consume_fuel(&ws.id, "agent-1", 5_000).unwrap();
        let usage = mgr.get_usage(&ws.id).unwrap();
        assert_eq!(usage.fuel_used_today, 5_000);
        assert_eq!(usage.fuel_remaining(), 5_000);
    }

    #[test]
    fn consume_fuel_exhausted() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap();

        mgr.consume_fuel(&ws.id, "agent-1", 8_000).unwrap();
        let err = mgr.consume_fuel(&ws.id, "agent-1", 5_000).unwrap_err();
        assert!(err.to_string().contains("exhausted"));
    }

    #[test]
    fn autonomy_level_check() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap(); // max L3

        assert!(mgr.check_autonomy_level(&ws.id, 3).is_ok());
        assert!(mgr.check_autonomy_level(&ws.id, 4).is_err());
    }

    #[test]
    fn provider_check() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap(); // only "claude"

        assert!(mgr.check_provider(&ws.id, "claude").is_ok());
        assert!(mgr.check_provider(&ws.id, "openai").is_err());
    }

    #[test]
    fn set_policy_updates_workspace() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap();

        let new_policy = WorkspacePolicy {
            max_autonomy_level: 5,
            fuel_budget_daily: 100_000,
            agent_limit: 20,
            allowed_providers: vec![],
            ..WorkspacePolicy::default()
        };
        mgr.set_policy(&ws.id, new_policy).unwrap();

        let ws = mgr.get_workspace(&ws.id).unwrap();
        assert_eq!(ws.max_autonomy_level, 5);
        assert_eq!(ws.fuel_budget_daily, 100_000);
        assert_eq!(ws.agent_limit, 20);
    }

    #[test]
    fn set_invalid_policy_fails() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap();

        let bad_policy = WorkspacePolicy {
            max_autonomy_level: 99,
            ..WorkspacePolicy::default()
        };
        assert!(mgr.set_policy(&ws.id, bad_policy).is_err());
    }

    #[test]
    fn get_usage_snapshot() {
        let mut mgr = WorkspaceManager::new();
        let ws = mgr.create_workspace(eng_config()).unwrap();

        mgr.deploy_agent(&ws.id, "agent-1").unwrap();
        mgr.consume_fuel(&ws.id, "agent-1", 1_000).unwrap();

        let usage = mgr.get_usage(&ws.id).unwrap();
        assert_eq!(usage.fuel_used_today, 1_000);
        assert_eq!(usage.agents_deployed, 1);
        assert_eq!(usage.member_count, 1);
        assert!((usage.fuel_usage_percent() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cross_workspace_isolation() {
        let mut mgr = WorkspaceManager::new();
        let eng = mgr.create_workspace(eng_config()).unwrap();
        let research = mgr.create_workspace(research_config()).unwrap();

        // Deploy agents in different workspaces.
        mgr.deploy_agent(&eng.id, "eng-agent").unwrap();
        mgr.deploy_agent(&research.id, "research-agent").unwrap();

        // Each workspace only sees its own agents.
        assert_eq!(mgr.list_agents(&eng.id), vec!["eng-agent"]);
        assert_eq!(mgr.list_agents(&research.id), vec!["research-agent"]);

        // Consume fuel in engineering; research budget is untouched.
        mgr.consume_fuel(&eng.id, "eng-agent", 5_000).unwrap();
        let eng_usage = mgr.get_usage(&eng.id).unwrap();
        let research_usage = mgr.get_usage(&research.id).unwrap();
        assert_eq!(eng_usage.fuel_used_today, 5_000);
        assert_eq!(research_usage.fuel_used_today, 0);
    }

    #[test]
    fn non_member_access_denied() {
        let mut mgr = WorkspaceManager::new();
        let eng = mgr.create_workspace(eng_config()).unwrap();

        // Charlie is not a member of Engineering.
        let err = mgr
            .check_access(&eng.id, "charlie", WorkspaceRole::Viewer)
            .unwrap_err();
        assert!(err.to_string().contains("not a member"));
    }
}
