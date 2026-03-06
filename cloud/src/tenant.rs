//! Multi-tenant isolation with plan-based resource limits.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Plan {
    Free,
    Pro,
    Team,
    Enterprise,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_agents: u32,
    pub max_fuel_per_month: u64,
    pub max_llm_tokens_per_month: u64,
    pub max_storage_bytes: u64,
    pub max_concurrent_runs: u32,
}

impl ResourceLimits {
    pub fn for_plan(plan: Plan) -> Self {
        match plan {
            Plan::Free => Self {
                max_agents: 1,
                max_fuel_per_month: 1_000,
                max_llm_tokens_per_month: 10_000,
                max_storage_bytes: 50 * 1024 * 1024, // 50 MB
                max_concurrent_runs: 1,
            },
            Plan::Pro => Self {
                max_agents: 10,
                max_fuel_per_month: 50_000,
                max_llm_tokens_per_month: 500_000,
                max_storage_bytes: 1024 * 1024 * 1024, // 1 GB
                max_concurrent_runs: 5,
            },
            Plan::Team => Self {
                max_agents: 50,
                max_fuel_per_month: 500_000,
                max_llm_tokens_per_month: 5_000_000,
                max_storage_bytes: 10 * 1024 * 1024 * 1024, // 10 GB
                max_concurrent_runs: 20,
            },
            Plan::Enterprise => Self {
                max_agents: u32::MAX,
                max_fuel_per_month: u64::MAX,
                max_llm_tokens_per_month: u64::MAX,
                max_storage_bytes: u64::MAX,
                max_concurrent_runs: u32::MAX,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub plan: Plan,
    pub resource_limits: ResourceLimits,
    pub created_at: u64,
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug)]
pub struct TenantManager {
    tenants: HashMap<Uuid, Tenant>,
}

impl TenantManager {
    pub fn new() -> Self {
        Self {
            tenants: HashMap::new(),
        }
    }

    pub fn create_tenant(&mut self, name: &str, plan: Plan) -> Uuid {
        let id = Uuid::new_v4();
        let tenant = Tenant {
            id,
            name: name.to_string(),
            plan,
            resource_limits: ResourceLimits::for_plan(plan),
            created_at: unix_now(),
        };
        self.tenants.insert(id, tenant);
        id
    }

    pub fn get_tenant(&self, id: Uuid) -> Option<&Tenant> {
        self.tenants.get(&id)
    }

    pub fn list_tenants(&self) -> Vec<&Tenant> {
        self.tenants.values().collect()
    }

    pub fn update_plan(&mut self, id: Uuid, plan: Plan) -> bool {
        if let Some(tenant) = self.tenants.get_mut(&id) {
            tenant.plan = plan;
            tenant.resource_limits = ResourceLimits::for_plan(plan);
            true
        } else {
            false
        }
    }

    pub fn delete_tenant(&mut self, id: Uuid) -> bool {
        self.tenants.remove(&id).is_some()
    }

    /// Check whether `current_usage` is within the tenant's limit for the given resource.
    pub fn check_limit(&self, tenant_id: Uuid, resource: &str, current_usage: u64) -> bool {
        let Some(tenant) = self.tenants.get(&tenant_id) else {
            return false;
        };
        let limit = match resource {
            "agents" => tenant.resource_limits.max_agents as u64,
            "fuel_per_month" => tenant.resource_limits.max_fuel_per_month,
            "llm_tokens_per_month" => tenant.resource_limits.max_llm_tokens_per_month,
            "storage_bytes" => tenant.resource_limits.max_storage_bytes,
            "concurrent_runs" => tenant.resource_limits.max_concurrent_runs as u64,
            _ => return false,
        };
        current_usage <= limit
    }
}

impl Default for TenantManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_get_tenant() {
        let mut mgr = TenantManager::new();
        let id = mgr.create_tenant("Acme Corp", Plan::Pro);

        let tenant = mgr.get_tenant(id).unwrap();
        assert_eq!(tenant.name, "Acme Corp");
        assert_eq!(tenant.plan, Plan::Pro);
        assert_eq!(tenant.resource_limits.max_agents, 10);
        assert_eq!(tenant.resource_limits.max_fuel_per_month, 50_000);
    }

    #[test]
    fn list_tenants_returns_all() {
        let mut mgr = TenantManager::new();
        mgr.create_tenant("Tenant A", Plan::Free);
        mgr.create_tenant("Tenant B", Plan::Team);
        assert_eq!(mgr.list_tenants().len(), 2);
    }

    #[test]
    fn update_plan_adjusts_limits() {
        let mut mgr = TenantManager::new();
        let id = mgr.create_tenant("Startup", Plan::Free);
        assert_eq!(mgr.get_tenant(id).unwrap().resource_limits.max_agents, 1);

        assert!(mgr.update_plan(id, Plan::Enterprise));
        let tenant = mgr.get_tenant(id).unwrap();
        assert_eq!(tenant.plan, Plan::Enterprise);
        assert_eq!(tenant.resource_limits.max_agents, u32::MAX);
        assert_eq!(tenant.resource_limits.max_fuel_per_month, u64::MAX);
    }

    #[test]
    fn update_plan_nonexistent_returns_false() {
        let mut mgr = TenantManager::new();
        assert!(!mgr.update_plan(Uuid::new_v4(), Plan::Pro));
    }

    #[test]
    fn delete_tenant_removes_it() {
        let mut mgr = TenantManager::new();
        let id = mgr.create_tenant("Doomed", Plan::Free);
        assert!(mgr.delete_tenant(id));
        assert!(mgr.get_tenant(id).is_none());
        assert!(!mgr.delete_tenant(id)); // double delete
    }

    #[test]
    fn check_limit_enforces_plan_bounds() {
        let mut mgr = TenantManager::new();
        let id = mgr.create_tenant("Limited", Plan::Free);

        // Free plan: max 1 agent, 1000 fuel
        assert!(mgr.check_limit(id, "agents", 1));
        assert!(!mgr.check_limit(id, "agents", 2));

        assert!(mgr.check_limit(id, "fuel_per_month", 1000));
        assert!(!mgr.check_limit(id, "fuel_per_month", 1001));

        assert!(mgr.check_limit(id, "concurrent_runs", 1));
        assert!(!mgr.check_limit(id, "concurrent_runs", 2));
    }

    #[test]
    fn check_limit_unknown_resource_returns_false() {
        let mut mgr = TenantManager::new();
        let id = mgr.create_tenant("Test", Plan::Enterprise);
        assert!(!mgr.check_limit(id, "nonexistent_resource", 0));
    }

    #[test]
    fn check_limit_nonexistent_tenant_returns_false() {
        let mgr = TenantManager::new();
        assert!(!mgr.check_limit(Uuid::new_v4(), "agents", 0));
    }

    #[test]
    fn plan_based_limits_are_correct() {
        let free = ResourceLimits::for_plan(Plan::Free);
        assert_eq!(free.max_agents, 1);
        assert_eq!(free.max_fuel_per_month, 1_000);
        assert_eq!(free.max_concurrent_runs, 1);

        let pro = ResourceLimits::for_plan(Plan::Pro);
        assert_eq!(pro.max_agents, 10);
        assert_eq!(pro.max_fuel_per_month, 50_000);
        assert_eq!(pro.max_concurrent_runs, 5);

        let team = ResourceLimits::for_plan(Plan::Team);
        assert_eq!(team.max_agents, 50);
        assert_eq!(team.max_fuel_per_month, 500_000);
        assert_eq!(team.max_concurrent_runs, 20);

        let enterprise = ResourceLimits::for_plan(Plan::Enterprise);
        assert_eq!(enterprise.max_agents, u32::MAX);
        assert_eq!(enterprise.max_fuel_per_month, u64::MAX);
    }
}
