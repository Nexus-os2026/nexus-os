//! `nexus-tenancy` — Workspace-based multi-tenancy for Nexus OS.
//!
//! Provides isolated workspaces within an organization, each with its own:
//! - Members and role-based access control
//! - Agent deployments with capacity limits
//! - Fuel budgets with daily reset and per-agent tracking
//! - Governance policies (autonomy level caps, provider restrictions, HITL thresholds)
//! - Data isolation (full or logical)
//!
//! # Architecture
//!
//! ```text
//! Organization
//! ├── Workspace: Engineering
//! │   ├── Users: [alice (Admin), bob (Operator)]
//! │   ├── Agents: [coder, reviewer]
//! │   ├── Fuel Budget: 10M/day
//! │   └── Policies: L3 max autonomy, HITL ≥ L2
//! ├── Workspace: Research
//! │   ├── Users: [charlie (Admin), diana (Viewer)]
//! │   ├── Agents: [researcher, analyst]
//! │   ├── Fuel Budget: 50M/day
//! │   └── Policies: L5 max autonomy
//! └── Admin Workspace
//!     └── Cross-workspace audit (read-only)
//! ```
//!
//! # Usage
//!
//! ```rust
//! use nexus_tenancy::{WorkspaceManager, WorkspaceConfig, WorkspaceRole};
//!
//! # fn main() -> Result<(), nexus_tenancy::TenancyError> {
//! let mut mgr = WorkspaceManager::new();
//!
//! // Create a workspace
//! let ws = mgr.create_workspace(WorkspaceConfig {
//!     name: "Engineering".into(),
//!     admin_user_id: "alice".into(),
//!     agent_limit: Some(10),
//!     fuel_budget_daily: Some(10_000_000),
//!     max_autonomy_level: Some(3),
//!     allowed_providers: Some(vec!["claude".into()]),
//!     data_isolation: None,
//! })?;
//!
//! // Add a member
//! mgr.add_member(&ws.id, "bob", WorkspaceRole::Operator)?;
//!
//! // Deploy an agent
//! mgr.deploy_agent(&ws.id, "did:key:z6MkCoder")?;
//!
//! // Consume fuel
//! mgr.consume_fuel(&ws.id, "did:key:z6MkCoder", 1000)?;
//!
//! // Check usage
//! let usage = mgr.get_usage(&ws.id)?;
//! assert_eq!(usage.fuel_used_today, 1000);
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod manager;
pub mod policy;
pub mod usage;
pub mod workspace;

// Re-exports for convenience.
pub use error::TenancyError;
pub use manager::WorkspaceManager;
pub use policy::WorkspacePolicy;
pub use usage::{FuelLedger, WorkspaceUsage};
pub use workspace::{DataIsolation, Workspace, WorkspaceConfig, WorkspaceMember, WorkspaceRole};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_to_end_workspace_lifecycle() {
        let mut mgr = WorkspaceManager::new();

        // Create workspace.
        let ws = mgr
            .create_workspace(WorkspaceConfig {
                name: "Test Team".to_string(),
                admin_user_id: "admin@test.com".to_string(),
                agent_limit: Some(5),
                fuel_budget_daily: Some(50_000),
                max_autonomy_level: Some(3),
                allowed_providers: Some(vec!["claude".to_string()]),
                data_isolation: Some(DataIsolation::Logical),
            })
            .unwrap();

        // Add members.
        mgr.add_member(&ws.id, "dev@test.com", WorkspaceRole::Operator)
            .unwrap();
        mgr.add_member(&ws.id, "auditor@test.com", WorkspaceRole::Auditor)
            .unwrap();

        // Access checks.
        assert!(mgr
            .check_access(&ws.id, "admin@test.com", WorkspaceRole::Admin)
            .is_ok());
        assert!(mgr
            .check_access(&ws.id, "dev@test.com", WorkspaceRole::Operator)
            .is_ok());
        assert!(mgr
            .check_access(&ws.id, "dev@test.com", WorkspaceRole::Admin)
            .is_err());

        // Deploy agents.
        mgr.deploy_agent(&ws.id, "did:key:z6MkAgent1").unwrap();
        mgr.deploy_agent(&ws.id, "did:key:z6MkAgent2").unwrap();

        // Governance checks.
        assert!(mgr.check_autonomy_level(&ws.id, 3).is_ok());
        assert!(mgr.check_autonomy_level(&ws.id, 4).is_err());
        assert!(mgr.check_provider(&ws.id, "claude").is_ok());
        assert!(mgr.check_provider(&ws.id, "openai").is_err());

        // Fuel consumption.
        mgr.consume_fuel(&ws.id, "did:key:z6MkAgent1", 10_000)
            .unwrap();
        mgr.consume_fuel(&ws.id, "did:key:z6MkAgent2", 5_000)
            .unwrap();

        // Usage report.
        let usage = mgr.get_usage(&ws.id).unwrap();
        assert_eq!(usage.fuel_used_today, 15_000);
        assert_eq!(usage.agents_deployed, 2);
        assert_eq!(usage.member_count, 3);
        assert!((usage.fuel_usage_percent() - 30.0).abs() < f64::EPSILON);

        // Cleanup.
        mgr.undeploy_agent(&ws.id, "did:key:z6MkAgent1");
        mgr.remove_member(&ws.id, "auditor@test.com").unwrap();

        let ws = mgr.get_workspace(&ws.id).unwrap();
        assert_eq!(ws.members.len(), 2);
    }

    #[test]
    fn multi_workspace_isolation() {
        let mut mgr = WorkspaceManager::new();

        let ws_a = mgr
            .create_workspace(WorkspaceConfig {
                name: "Team A".to_string(),
                admin_user_id: "alice".to_string(),
                agent_limit: Some(5),
                fuel_budget_daily: Some(10_000),
                max_autonomy_level: Some(2),
                allowed_providers: None,
                data_isolation: None,
            })
            .unwrap();

        let ws_b = mgr
            .create_workspace(WorkspaceConfig {
                name: "Team B".to_string(),
                admin_user_id: "bob".to_string(),
                agent_limit: Some(5),
                fuel_budget_daily: Some(20_000),
                max_autonomy_level: Some(5),
                allowed_providers: None,
                data_isolation: None,
            })
            .unwrap();

        // Alice cannot access Team B.
        assert!(mgr
            .check_access(&ws_b.id, "alice", WorkspaceRole::Viewer)
            .is_err());

        // Bob cannot access Team A.
        assert!(mgr
            .check_access(&ws_a.id, "bob", WorkspaceRole::Viewer)
            .is_err());

        // Independent fuel budgets.
        mgr.consume_fuel(&ws_a.id, "agent-a", 5_000).unwrap();
        let usage_a = mgr.get_usage(&ws_a.id).unwrap();
        let usage_b = mgr.get_usage(&ws_b.id).unwrap();
        assert_eq!(usage_a.fuel_used_today, 5_000);
        assert_eq!(usage_b.fuel_used_today, 0);

        // Independent autonomy caps.
        assert!(mgr.check_autonomy_level(&ws_a.id, 3).is_err()); // max 2
        assert!(mgr.check_autonomy_level(&ws_b.id, 5).is_ok()); // max 5
    }
}
