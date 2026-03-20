//! Workspace-scoped governance policies.
//!
//! Policies constrain what agents and users can do within a workspace,
//! layered on top of the global governance rules in the kernel.

use crate::error::TenancyError;
use serde::{Deserialize, Serialize};

/// A workspace-level governance policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePolicy {
    /// Maximum autonomy level (0-5) for agents in this workspace.
    pub max_autonomy_level: u8,
    /// Daily fuel budget (resets at midnight UTC).
    pub fuel_budget_daily: u64,
    /// Maximum number of agents.
    pub agent_limit: u32,
    /// Allowed LLM providers (empty = all).
    pub allowed_providers: Vec<String>,
    /// Whether agents can access external networks.
    pub allow_network_access: bool,
    /// Whether agents can write to the filesystem.
    pub allow_filesystem_write: bool,
    /// Maximum single-action fuel cost (prevents runaway agents).
    pub max_single_action_fuel: u64,
    /// HITL required for autonomy levels at or above this threshold.
    pub hitl_threshold_level: u8,
}

impl Default for WorkspacePolicy {
    fn default() -> Self {
        Self {
            max_autonomy_level: 3,
            fuel_budget_daily: 10_000_000,
            agent_limit: 50,
            allowed_providers: vec![],
            allow_network_access: true,
            allow_filesystem_write: false,
            max_single_action_fuel: 100_000,
            hitl_threshold_level: 2,
        }
    }
}

impl WorkspacePolicy {
    /// Validate that all policy values are within acceptable ranges.
    pub fn validate(&self) -> Result<(), TenancyError> {
        if self.max_autonomy_level > 5 {
            return Err(TenancyError::InvalidPolicy(format!(
                "max_autonomy_level {} exceeds maximum of 5",
                self.max_autonomy_level
            )));
        }
        if self.fuel_budget_daily == 0 {
            return Err(TenancyError::InvalidPolicy(
                "fuel_budget_daily must be > 0".into(),
            ));
        }
        if self.agent_limit == 0 {
            return Err(TenancyError::InvalidPolicy(
                "agent_limit must be > 0".into(),
            ));
        }
        if self.hitl_threshold_level > 5 {
            return Err(TenancyError::InvalidPolicy(format!(
                "hitl_threshold_level {} exceeds maximum of 5",
                self.hitl_threshold_level
            )));
        }
        if self.max_single_action_fuel > self.fuel_budget_daily {
            return Err(TenancyError::InvalidPolicy(
                "max_single_action_fuel cannot exceed fuel_budget_daily".into(),
            ));
        }
        Ok(())
    }

    /// Check if an autonomy level is allowed by this policy.
    pub fn allows_autonomy_level(&self, level: u8) -> bool {
        level <= self.max_autonomy_level
    }

    /// Check if a provider is allowed by this policy.
    pub fn allows_provider(&self, provider: &str) -> bool {
        self.allowed_providers.is_empty() || self.allowed_providers.iter().any(|p| p == provider)
    }

    /// Check if a fuel cost is within the single-action limit.
    pub fn allows_fuel_cost(&self, cost: u64) -> bool {
        cost <= self.max_single_action_fuel
    }

    /// Check if HITL is required for the given autonomy level.
    pub fn requires_hitl(&self, autonomy_level: u8) -> bool {
        autonomy_level >= self.hitl_threshold_level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_valid() {
        let policy = WorkspacePolicy::default();
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn invalid_autonomy_level() {
        let policy = WorkspacePolicy {
            max_autonomy_level: 6,
            ..Default::default()
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn invalid_zero_budget() {
        let policy = WorkspacePolicy {
            fuel_budget_daily: 0,
            ..Default::default()
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn invalid_zero_agent_limit() {
        let policy = WorkspacePolicy {
            agent_limit: 0,
            ..Default::default()
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn invalid_single_action_exceeds_budget() {
        let policy = WorkspacePolicy {
            fuel_budget_daily: 1000,
            max_single_action_fuel: 2000,
            ..Default::default()
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn allows_autonomy_level() {
        let policy = WorkspacePolicy {
            max_autonomy_level: 3,
            ..Default::default()
        };
        assert!(policy.allows_autonomy_level(0));
        assert!(policy.allows_autonomy_level(3));
        assert!(!policy.allows_autonomy_level(4));
    }

    #[test]
    fn allows_provider_empty_means_all() {
        let policy = WorkspacePolicy::default();
        assert!(policy.allows_provider("anything"));
    }

    #[test]
    fn allows_provider_filters() {
        let policy = WorkspacePolicy {
            allowed_providers: vec!["claude".to_string()],
            ..Default::default()
        };
        assert!(policy.allows_provider("claude"));
        assert!(!policy.allows_provider("openai"));
    }

    #[test]
    fn allows_fuel_cost() {
        let policy = WorkspacePolicy {
            max_single_action_fuel: 500,
            ..Default::default()
        };
        assert!(policy.allows_fuel_cost(500));
        assert!(!policy.allows_fuel_cost(501));
    }

    #[test]
    fn requires_hitl() {
        let policy = WorkspacePolicy {
            hitl_threshold_level: 2,
            ..Default::default()
        };
        assert!(!policy.requires_hitl(0));
        assert!(!policy.requires_hitl(1));
        assert!(policy.requires_hitl(2));
        assert!(policy.requires_hitl(5));
    }

    #[test]
    fn serde_roundtrip() {
        let policy = WorkspacePolicy::default();
        let json = serde_json::to_string(&policy).unwrap();
        let parsed: WorkspacePolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_autonomy_level, 3);
        assert_eq!(parsed.fuel_budget_daily, 10_000_000);
    }
}
