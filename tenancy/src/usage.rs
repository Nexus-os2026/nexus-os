//! Workspace usage tracking — fuel consumption, agent counts, and quotas.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Current usage snapshot for a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceUsage {
    /// Workspace ID.
    pub workspace_id: String,
    /// When this snapshot was taken.
    pub captured_at: DateTime<Utc>,
    /// Fuel consumed today (resets at midnight UTC).
    pub fuel_used_today: u64,
    /// Daily fuel budget.
    pub fuel_budget_daily: u64,
    /// Number of agents currently deployed.
    pub agents_deployed: u32,
    /// Maximum agents allowed.
    pub agent_limit: u32,
    /// Number of active workspace members.
    pub member_count: u32,
    /// Total audit trail entries for this workspace.
    pub audit_entries: u64,
    /// LLM requests made today.
    pub llm_requests_today: u64,
    /// Total LLM tokens consumed today (input + output).
    pub llm_tokens_today: u64,
}

impl WorkspaceUsage {
    /// Percentage of daily fuel budget consumed (0.0 to 100.0+).
    pub fn fuel_usage_percent(&self) -> f64 {
        if self.fuel_budget_daily == 0 {
            return 100.0;
        }
        (self.fuel_used_today as f64 / self.fuel_budget_daily as f64) * 100.0
    }

    /// Percentage of agent limit used.
    pub fn agent_usage_percent(&self) -> f64 {
        if self.agent_limit == 0 {
            return 100.0;
        }
        (self.agents_deployed as f64 / self.agent_limit as f64) * 100.0
    }

    /// Whether the workspace has exceeded its daily fuel budget.
    pub fn is_fuel_exhausted(&self) -> bool {
        self.fuel_used_today >= self.fuel_budget_daily
    }

    /// Whether the workspace has reached its agent limit.
    pub fn is_agent_limit_reached(&self) -> bool {
        self.agents_deployed >= self.agent_limit
    }

    /// Remaining fuel for today.
    pub fn fuel_remaining(&self) -> u64 {
        self.fuel_budget_daily.saturating_sub(self.fuel_used_today)
    }
}

/// Per-workspace fuel ledger that tracks daily consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelLedger {
    pub workspace_id: String,
    pub budget_daily: u64,
    pub consumed_today: u64,
    pub last_reset: DateTime<Utc>,
    /// Per-agent fuel consumption within this workspace.
    pub agent_consumption: std::collections::HashMap<String, u64>,
}

impl FuelLedger {
    /// Create a new fuel ledger for a workspace.
    pub fn new(workspace_id: String, budget_daily: u64) -> Self {
        Self {
            workspace_id,
            budget_daily,
            consumed_today: 0,
            last_reset: Utc::now(),
            agent_consumption: std::collections::HashMap::new(),
        }
    }

    /// Check if daily reset is needed (crossed midnight UTC) and reset if so.
    pub fn maybe_reset(&mut self) {
        let now = Utc::now();
        if now.date_naive() != self.last_reset.date_naive() {
            tracing::info!(
                workspace_id = %self.workspace_id,
                consumed = self.consumed_today,
                "Resetting daily fuel ledger"
            );
            self.consumed_today = 0;
            self.agent_consumption.clear();
            self.last_reset = now;
        }
    }

    /// Try to consume fuel. Returns `Ok(())` if budget allows, `Err` if exhausted.
    pub fn try_consume(
        &mut self,
        agent_did: &str,
        amount: u64,
    ) -> Result<(), crate::error::TenancyError> {
        self.maybe_reset();

        if self.consumed_today + amount > self.budget_daily {
            return Err(crate::error::TenancyError::FuelBudgetExhausted {
                workspace_id: self.workspace_id.clone(),
                used: self.consumed_today,
                budget: self.budget_daily,
            });
        }

        self.consumed_today += amount;
        *self
            .agent_consumption
            .entry(agent_did.to_string())
            .or_insert(0) += amount;
        Ok(())
    }

    /// Fuel remaining today.
    pub fn remaining(&self) -> u64 {
        self.budget_daily.saturating_sub(self.consumed_today)
    }

    /// Top N fuel consumers in this workspace.
    pub fn top_consumers(&self, n: usize) -> Vec<(&str, u64)> {
        let mut entries: Vec<_> = self
            .agent_consumption
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(n);
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_percentages() {
        let usage = WorkspaceUsage {
            workspace_id: "ws-1".to_string(),
            captured_at: Utc::now(),
            fuel_used_today: 2_500_000,
            fuel_budget_daily: 10_000_000,
            agents_deployed: 5,
            agent_limit: 10,
            member_count: 3,
            audit_entries: 100,
            llm_requests_today: 50,
            llm_tokens_today: 10_000,
        };

        assert!((usage.fuel_usage_percent() - 25.0).abs() < f64::EPSILON);
        assert!((usage.agent_usage_percent() - 50.0).abs() < f64::EPSILON);
        assert!(!usage.is_fuel_exhausted());
        assert!(!usage.is_agent_limit_reached());
        assert_eq!(usage.fuel_remaining(), 7_500_000);
    }

    #[test]
    fn usage_exhausted() {
        let usage = WorkspaceUsage {
            workspace_id: "ws-1".to_string(),
            captured_at: Utc::now(),
            fuel_used_today: 10_000_000,
            fuel_budget_daily: 10_000_000,
            agents_deployed: 10,
            agent_limit: 10,
            member_count: 1,
            audit_entries: 0,
            llm_requests_today: 0,
            llm_tokens_today: 0,
        };

        assert!(usage.is_fuel_exhausted());
        assert!(usage.is_agent_limit_reached());
        assert_eq!(usage.fuel_remaining(), 0);
    }

    #[test]
    fn fuel_ledger_consume() {
        let mut ledger = FuelLedger::new("ws-1".to_string(), 1000);
        assert!(ledger.try_consume("agent-1", 500).is_ok());
        assert_eq!(ledger.remaining(), 500);
        assert!(ledger.try_consume("agent-2", 300).is_ok());
        assert_eq!(ledger.remaining(), 200);
    }

    #[test]
    fn fuel_ledger_exhausted() {
        let mut ledger = FuelLedger::new("ws-1".to_string(), 1000);
        assert!(ledger.try_consume("agent-1", 600).is_ok());
        let err = ledger.try_consume("agent-1", 500).unwrap_err();
        assert!(
            err.to_string().contains("exhausted"),
            "expected fuel exhausted error, got: {err}"
        );
    }

    #[test]
    fn fuel_ledger_top_consumers() {
        let mut ledger = FuelLedger::new("ws-1".to_string(), 10_000);
        ledger.try_consume("agent-a", 500).unwrap();
        ledger.try_consume("agent-b", 300).unwrap();
        ledger.try_consume("agent-c", 800).unwrap();
        ledger.try_consume("agent-a", 200).unwrap();

        let top = ledger.top_consumers(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, "agent-c"); // 800
        assert_eq!(top[1].0, "agent-a"); // 700
    }

    #[test]
    fn fuel_ledger_tracks_per_agent() {
        let mut ledger = FuelLedger::new("ws-1".to_string(), 10_000);
        ledger.try_consume("agent-1", 100).unwrap();
        ledger.try_consume("agent-1", 200).unwrap();
        ledger.try_consume("agent-2", 50).unwrap();

        assert_eq!(ledger.agent_consumption["agent-1"], 300);
        assert_eq!(ledger.agent_consumption["agent-2"], 50);
    }

    #[test]
    fn serde_roundtrip() {
        let usage = WorkspaceUsage {
            workspace_id: "ws-1".to_string(),
            captured_at: Utc::now(),
            fuel_used_today: 100,
            fuel_budget_daily: 1000,
            agents_deployed: 2,
            agent_limit: 10,
            member_count: 3,
            audit_entries: 50,
            llm_requests_today: 10,
            llm_tokens_today: 5000,
        };
        let json = serde_json::to_string(&usage).unwrap();
        let parsed: WorkspaceUsage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.workspace_id, "ws-1");
        assert_eq!(parsed.fuel_used_today, 100);
    }
}
