//! Core workspace types and data isolation model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Data isolation strategy for a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataIsolation {
    /// Completely separate database files per workspace.
    Full,
    /// Shared database with row-level isolation via `workspace_id` column.
    Logical,
}

/// Role within a specific workspace (distinct from the global `UserRole`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspaceRole {
    /// Full control over the workspace: members, policies, agents.
    Admin,
    /// Agent deployment, HITL approval, configuration.
    Operator,
    /// Read-only access to dashboards and agent status.
    Viewer,
    /// Read-only access to audit trails and compliance data.
    Auditor,
}

impl WorkspaceRole {
    /// Privilege level within a workspace (higher = more access).
    pub fn privilege_level(&self) -> u8 {
        match self {
            Self::Admin => 100,
            Self::Operator => 75,
            Self::Auditor => 25,
            Self::Viewer => 10,
        }
    }

    /// Check if this role satisfies the required role.
    pub fn satisfies(&self, required: &WorkspaceRole) -> bool {
        self.privilege_level() >= required.privilege_level()
    }
}

impl std::fmt::Display for WorkspaceRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admin => write!(f, "Admin"),
            Self::Operator => write!(f, "Operator"),
            Self::Viewer => write!(f, "Viewer"),
            Self::Auditor => write!(f, "Auditor"),
        }
    }
}

/// A member of a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMember {
    pub user_id: String,
    pub role: WorkspaceRole,
    pub added_at: DateTime<Utc>,
}

/// An isolated workspace within an organization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// Unique workspace identifier.
    pub id: String,
    /// Human-readable workspace name.
    pub name: String,
    /// When the workspace was created.
    pub created_at: DateTime<Utc>,
    /// Workspace members with their roles.
    pub members: Vec<WorkspaceMember>,
    /// Maximum number of agents allowed in this workspace.
    pub agent_limit: u32,
    /// Daily fuel budget (resets at midnight UTC).
    pub fuel_budget_daily: u64,
    /// Maximum autonomy level agents in this workspace can reach (0-5).
    pub max_autonomy_level: u8,
    /// LLM providers this workspace is allowed to use.
    pub allowed_providers: Vec<String>,
    /// Data isolation strategy.
    pub data_isolation: DataIsolation,
}

impl Workspace {
    /// Get the list of admin user IDs.
    pub fn admins(&self) -> Vec<&str> {
        self.members
            .iter()
            .filter(|m| m.role == WorkspaceRole::Admin)
            .map(|m| m.user_id.as_str())
            .collect()
    }

    /// Check if a user is a member of this workspace.
    pub fn has_member(&self, user_id: &str) -> bool {
        self.members.iter().any(|m| m.user_id == user_id)
    }

    /// Get a member's role, if they belong to this workspace.
    pub fn member_role(&self, user_id: &str) -> Option<WorkspaceRole> {
        self.members
            .iter()
            .find(|m| m.user_id == user_id)
            .map(|m| m.role)
    }

    /// Check if the given provider is allowed in this workspace.
    pub fn is_provider_allowed(&self, provider: &str) -> bool {
        self.allowed_providers.is_empty() || self.allowed_providers.iter().any(|p| p == provider)
    }
}

/// Configuration for creating a new workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub name: String,
    pub admin_user_id: String,
    pub agent_limit: Option<u32>,
    pub fuel_budget_daily: Option<u64>,
    pub max_autonomy_level: Option<u8>,
    pub allowed_providers: Option<Vec<String>>,
    pub data_isolation: Option<DataIsolation>,
}

impl WorkspaceConfig {
    /// Build a `Workspace` from this config with sensible defaults.
    pub fn into_workspace(self) -> Workspace {
        let now = Utc::now();
        Workspace {
            id: Uuid::new_v4().to_string(),
            name: self.name,
            created_at: now,
            members: vec![WorkspaceMember {
                user_id: self.admin_user_id,
                role: WorkspaceRole::Admin,
                added_at: now,
            }],
            agent_limit: self.agent_limit.unwrap_or(50),
            fuel_budget_daily: self.fuel_budget_daily.unwrap_or(10_000_000),
            max_autonomy_level: self.max_autonomy_level.unwrap_or(3).min(5),
            allowed_providers: self.allowed_providers.unwrap_or_default(),
            data_isolation: self.data_isolation.unwrap_or(DataIsolation::Logical),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_workspace() -> Workspace {
        WorkspaceConfig {
            name: "Engineering".to_string(),
            admin_user_id: "alice".to_string(),
            agent_limit: Some(10),
            fuel_budget_daily: Some(5_000_000),
            max_autonomy_level: Some(3),
            allowed_providers: Some(vec!["claude".to_string(), "local-slm".to_string()]),
            data_isolation: Some(DataIsolation::Logical),
        }
        .into_workspace()
    }

    #[test]
    fn workspace_creation_defaults() {
        let ws = WorkspaceConfig {
            name: "Default".to_string(),
            admin_user_id: "admin".to_string(),
            agent_limit: None,
            fuel_budget_daily: None,
            max_autonomy_level: None,
            allowed_providers: None,
            data_isolation: None,
        }
        .into_workspace();

        assert_eq!(ws.agent_limit, 50);
        assert_eq!(ws.fuel_budget_daily, 10_000_000);
        assert_eq!(ws.max_autonomy_level, 3);
        assert!(ws.allowed_providers.is_empty());
        assert_eq!(ws.data_isolation, DataIsolation::Logical);
    }

    #[test]
    fn workspace_has_member() {
        let ws = test_workspace();
        assert!(ws.has_member("alice"));
        assert!(!ws.has_member("bob"));
    }

    #[test]
    fn workspace_admins() {
        let ws = test_workspace();
        assert_eq!(ws.admins(), vec!["alice"]);
    }

    #[test]
    fn workspace_member_role() {
        let ws = test_workspace();
        assert_eq!(ws.member_role("alice"), Some(WorkspaceRole::Admin));
        assert_eq!(ws.member_role("unknown"), None);
    }

    #[test]
    fn provider_allowed_empty_means_all() {
        let mut ws = test_workspace();
        ws.allowed_providers = vec![];
        assert!(ws.is_provider_allowed("anything"));
    }

    #[test]
    fn provider_allowed_filters() {
        let ws = test_workspace();
        assert!(ws.is_provider_allowed("claude"));
        assert!(!ws.is_provider_allowed("openai"));
    }

    #[test]
    fn workspace_role_satisfies() {
        assert!(WorkspaceRole::Admin.satisfies(&WorkspaceRole::Operator));
        assert!(WorkspaceRole::Operator.satisfies(&WorkspaceRole::Viewer));
        assert!(!WorkspaceRole::Viewer.satisfies(&WorkspaceRole::Operator));
        assert!(!WorkspaceRole::Auditor.satisfies(&WorkspaceRole::Operator));
    }

    #[test]
    fn max_autonomy_clamped_to_5() {
        let ws = WorkspaceConfig {
            name: "Test".to_string(),
            admin_user_id: "admin".to_string(),
            agent_limit: None,
            fuel_budget_daily: None,
            max_autonomy_level: Some(99),
            allowed_providers: None,
            data_isolation: None,
        }
        .into_workspace();
        assert_eq!(ws.max_autonomy_level, 5);
    }

    #[test]
    fn serde_roundtrip() {
        let ws = test_workspace();
        let json = serde_json::to_string(&ws).unwrap();
        let parsed: Workspace = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "Engineering");
        assert_eq!(parsed.members.len(), 1);
    }
}
