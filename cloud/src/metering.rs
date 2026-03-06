//! Usage metering: record, aggregate, and enforce resource consumption limits.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub tenant_id: Uuid,
    pub timestamp: u64,
    pub metric: String,
    pub amount: u64,
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug)]
pub struct MeteringEngine {
    records: Vec<UsageRecord>,
}

impl MeteringEngine {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub fn record(&mut self, tenant_id: Uuid, metric: &str, amount: u64) {
        self.records.push(UsageRecord {
            tenant_id,
            timestamp: unix_now(),
            metric: metric.to_string(),
            amount,
        });
    }

    /// Record with an explicit timestamp (useful for testing).
    pub fn record_at(&mut self, tenant_id: Uuid, metric: &str, amount: u64, timestamp: u64) {
        self.records.push(UsageRecord {
            tenant_id,
            timestamp,
            metric: metric.to_string(),
            amount,
        });
    }

    /// Sum usage for a tenant + metric within the given time range (inclusive).
    pub fn usage_for_period(&self, tenant_id: Uuid, metric: &str, from: u64, to: u64) -> u64 {
        self.records
            .iter()
            .filter(|r| {
                r.tenant_id == tenant_id
                    && r.metric == metric
                    && r.timestamp >= from
                    && r.timestamp <= to
            })
            .map(|r| r.amount)
            .sum()
    }

    /// Check whether total usage for this tenant + metric is within the given limit.
    pub fn is_within_limit(&self, tenant_id: Uuid, metric: &str, limit: u64) -> bool {
        let total: u64 = self
            .records
            .iter()
            .filter(|r| r.tenant_id == tenant_id && r.metric == metric)
            .map(|r| r.amount)
            .sum();
        total <= limit
    }
}

impl Default for MeteringEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_sum_usage() {
        let mut engine = MeteringEngine::new();
        let tenant = Uuid::new_v4();

        engine.record_at(tenant, "fuel_consumed", 100, 1000);
        engine.record_at(tenant, "fuel_consumed", 200, 2000);
        engine.record_at(tenant, "llm_tokens", 50, 1500);

        let fuel = engine.usage_for_period(tenant, "fuel_consumed", 0, u64::MAX);
        assert_eq!(fuel, 300);

        let tokens = engine.usage_for_period(tenant, "llm_tokens", 0, u64::MAX);
        assert_eq!(tokens, 50);
    }

    #[test]
    fn usage_for_period_filters_by_time() {
        let mut engine = MeteringEngine::new();
        let tenant = Uuid::new_v4();

        engine.record_at(tenant, "fuel_consumed", 100, 1000);
        engine.record_at(tenant, "fuel_consumed", 200, 2000);
        engine.record_at(tenant, "fuel_consumed", 300, 3000);

        // Only records between 1500 and 2500
        let usage = engine.usage_for_period(tenant, "fuel_consumed", 1500, 2500);
        assert_eq!(usage, 200);
    }

    #[test]
    fn usage_isolates_tenants() {
        let mut engine = MeteringEngine::new();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();

        engine.record_at(tenant_a, "fuel_consumed", 100, 1000);
        engine.record_at(tenant_b, "fuel_consumed", 999, 1000);

        assert_eq!(
            engine.usage_for_period(tenant_a, "fuel_consumed", 0, u64::MAX),
            100
        );
        assert_eq!(
            engine.usage_for_period(tenant_b, "fuel_consumed", 0, u64::MAX),
            999
        );
    }

    #[test]
    fn is_within_limit_works() {
        let mut engine = MeteringEngine::new();
        let tenant = Uuid::new_v4();

        engine.record_at(tenant, "fuel_consumed", 400, 1000);
        engine.record_at(tenant, "fuel_consumed", 500, 2000);

        // Total = 900
        assert!(engine.is_within_limit(tenant, "fuel_consumed", 1000));
        assert!(engine.is_within_limit(tenant, "fuel_consumed", 900));
        assert!(!engine.is_within_limit(tenant, "fuel_consumed", 899));
    }

    #[test]
    fn is_within_limit_no_records_returns_true() {
        let engine = MeteringEngine::new();
        let tenant = Uuid::new_v4();
        assert!(engine.is_within_limit(tenant, "fuel_consumed", 0));
    }

    #[test]
    fn record_uses_current_timestamp() {
        let mut engine = MeteringEngine::new();
        let tenant = Uuid::new_v4();

        engine.record(tenant, "llm_tokens", 42);

        let total = engine.usage_for_period(tenant, "llm_tokens", 0, u64::MAX);
        assert_eq!(total, 42);
    }
}
