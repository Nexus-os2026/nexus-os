//! Aggregation engine — builds UsageReports from raw records.

use chrono::{Duration, Utc};
use std::collections::HashMap;

use crate::cost::CostRates;
use crate::error::MeteringError;
use crate::store::MeteringStore;
use crate::types::{
    AgentUsageSummary, CostLineItem, GroupBy, ResourceType, TimePeriod, UsageRecord, UsageReport,
    UsageTrend,
};

/// Builds aggregated reports from the metering store.
pub struct MeteringAggregator<'a> {
    store: &'a MeteringStore,
    rates: &'a CostRates,
}

impl<'a> MeteringAggregator<'a> {
    pub fn new(store: &'a MeteringStore, rates: &'a CostRates) -> Self {
        Self { store, rates }
    }

    /// Build a report for a workspace over a time period.
    pub fn workspace_report(
        &self,
        workspace_id: &str,
        period: &TimePeriod,
    ) -> Result<UsageReport, MeteringError> {
        let (start, end) = period_bounds(period);
        let records = self.store.query_records(workspace_id, &start, &end)?;

        let report = self.build_report(period.clone(), Some(workspace_id.to_string()), &records)?;
        Ok(report)
    }

    /// Build grouped reports (one per distinct group key).
    pub fn grouped_reports(
        &self,
        workspace_id: &str,
        period: &TimePeriod,
        group_by: &GroupBy,
    ) -> Result<Vec<UsageReport>, MeteringError> {
        let (start, end) = period_bounds(period);
        let records = self.store.query_records(workspace_id, &start, &end)?;

        // Partition by group key.
        let mut groups: HashMap<String, Vec<&UsageRecord>> = HashMap::new();
        for record in &records {
            let key = match group_by {
                GroupBy::Workspace => record.workspace_id.clone(),
                GroupBy::User => record.user_id.clone(),
                GroupBy::Agent => record.agent_did.clone(),
                GroupBy::Provider => extract_provider(&record.resource_type),
                GroupBy::ResourceType => record.resource_type.category().to_string(),
            };
            groups.entry(key).or_default().push(record);
        }

        let mut reports = Vec::new();
        for (key, group_records) in groups {
            let owned: Vec<UsageRecord> = group_records.into_iter().cloned().collect();
            let mut report = self.build_report(period.clone(), Some(key.clone()), &owned)?;
            report.group_key = Some(key);
            reports.push(report);
        }

        // Sort by cost descending.
        reports.sort_by(|a, b| {
            let a_cost: f64 = a.cost_breakdown.iter().map(|c| c.total_cost).sum();
            let b_cost: f64 = b.cost_breakdown.iter().map(|c| c.total_cost).sum();
            b_cost
                .partial_cmp(&a_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(reports)
    }

    fn build_report(
        &self,
        period: TimePeriod,
        group_key: Option<String>,
        records: &[UsageRecord],
    ) -> Result<UsageReport, MeteringError> {
        let mut total_llm_tokens: u64 = 0;
        let mut total_fuel: u64 = 0;
        let mut total_compute: f64 = 0.0;
        let mut total_api: u64 = 0;
        let mut total_storage: u64 = 0;

        // Cost by category.
        let mut category_costs: HashMap<String, (f64, f64)> = HashMap::new(); // (quantity, cost)

        // Agent stats.
        let mut agent_stats: HashMap<String, (u64, f64)> = HashMap::new(); // (count, cost)

        for record in records {
            let cost = record
                .cost_estimate_usd
                .unwrap_or_else(|| self.rates.estimate(&record.resource_type, record.quantity));

            match &record.resource_type {
                ResourceType::LlmTokensInput { .. } | ResourceType::LlmTokensOutput { .. } => {
                    total_llm_tokens += record.quantity as u64;
                }
                ResourceType::AgentFuelConsumed => {
                    total_fuel += record.quantity as u64;
                }
                ResourceType::SandboxComputeSeconds => {
                    total_compute += record.quantity;
                }
                ResourceType::ApiCalls | ResourceType::IntegrationCalls { .. } => {
                    total_api += record.quantity as u64;
                }
                ResourceType::StorageBytes => {
                    total_storage += record.quantity as u64;
                }
            }

            let cat = record.resource_type.category().to_string();
            let entry = category_costs.entry(cat).or_insert((0.0, 0.0));
            entry.0 += record.quantity;
            entry.1 += cost;

            let agent_entry = agent_stats
                .entry(record.agent_did.clone())
                .or_insert((0, 0.0));
            agent_entry.0 += 1;
            agent_entry.1 += cost;
        }

        // Build cost breakdown.
        let cost_breakdown: Vec<CostLineItem> = category_costs
            .into_iter()
            .map(|(cat, (qty, cost))| {
                let unit_cost = if qty > 0.0 { cost / qty } else { 0.0 };
                CostLineItem {
                    category: cat,
                    quantity: qty,
                    unit_cost,
                    total_cost: cost,
                }
            })
            .collect();

        // Build top agents (top 10).
        let mut top_agents: Vec<AgentUsageSummary> = agent_stats
            .into_iter()
            .map(|(did, (count, cost))| AgentUsageSummary {
                agent_did: did,
                total_records: count,
                total_cost: cost,
            })
            .collect();
        top_agents.sort_by(|a, b| {
            b.total_cost
                .partial_cmp(&a.total_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        top_agents.truncate(10);

        let current_cost: f64 = cost_breakdown.iter().map(|c| c.total_cost).sum();

        // Trend: compare with previous period of equal length.
        let trend = self.compute_trend(&period, group_key.as_deref(), current_cost);

        Ok(UsageReport {
            period,
            group_key,
            total_llm_tokens,
            total_fuel_consumed: total_fuel,
            total_compute_seconds: total_compute,
            total_api_calls: total_api,
            total_storage_bytes: total_storage,
            cost_breakdown,
            top_agents,
            trend,
        })
    }

    fn compute_trend(
        &self,
        period: &TimePeriod,
        workspace_id: Option<&str>,
        current_cost: f64,
    ) -> UsageTrend {
        let ws = workspace_id.unwrap_or("*");
        let (start, end) = period_bounds(period);

        // Compute previous period bounds.
        let prev_cost = if let (Ok(s), Ok(e)) = (
            chrono::DateTime::parse_from_rfc3339(&start),
            chrono::DateTime::parse_from_rfc3339(&end),
        ) {
            let duration = e - s;
            let prev_end = s;
            let prev_start = prev_end - duration;
            self.store
                .sum_cost(ws, &prev_start.to_rfc3339(), &prev_end.to_rfc3339())
                .unwrap_or(0.0)
        } else {
            0.0
        };

        let change_percent = if prev_cost > 0.0 {
            ((current_cost - prev_cost) / prev_cost) * 100.0
        } else if current_cost > 0.0 {
            100.0
        } else {
            0.0
        };

        UsageTrend {
            previous_period_cost: prev_cost,
            current_period_cost: current_cost,
            change_percent,
        }
    }
}

/// Export records as CSV.
pub fn export_csv(records: &[UsageRecord]) -> String {
    let mut out = String::from("id,timestamp,workspace_id,user_id,agent_did,resource_type,quantity,unit,cost_estimate_usd\n");

    for r in records {
        let rt = serde_json::to_string(&r.resource_type).unwrap_or_default();
        let cost = r
            .cost_estimate_usd
            .map(|c| format!("{c:.6}"))
            .unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            r.id,
            r.timestamp.to_rfc3339(),
            r.workspace_id,
            r.user_id,
            r.agent_did,
            rt.replace(',', ";"),
            r.quantity,
            r.unit,
            cost,
        ));
    }
    out
}

fn extract_provider(rt: &ResourceType) -> String {
    match rt {
        ResourceType::LlmTokensInput { provider, .. }
        | ResourceType::LlmTokensOutput { provider, .. } => provider.clone(),
        ResourceType::IntegrationCalls { provider } => provider.clone(),
        other => other.category().to_string(),
    }
}

pub fn period_bounds(period: &TimePeriod) -> (String, String) {
    let now = Utc::now();
    match period {
        TimePeriod::Hour => {
            let start = now - Duration::hours(1);
            (start.to_rfc3339(), now.to_rfc3339())
        }
        TimePeriod::Day => {
            let start = now - Duration::days(1);
            (start.to_rfc3339(), now.to_rfc3339())
        }
        TimePeriod::Week => {
            let start = now - Duration::weeks(1);
            (start.to_rfc3339(), now.to_rfc3339())
        }
        TimePeriod::Month => {
            let start = now - Duration::days(30);
            (start.to_rfc3339(), now.to_rfc3339())
        }
        TimePeriod::Custom { start, end } => (start.to_rfc3339(), end.to_rfc3339()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MeteringStore;

    #[test]
    fn csv_export_format() {
        let records = vec![
            UsageRecord::new("ws-1", "u1", "a1", ResourceType::ApiCalls, 100.0).with_cost(0.01),
        ];
        let csv = export_csv(&records);
        assert!(csv.starts_with("id,timestamp,"));
        assert!(csv.contains("ws-1"));
        assert!(csv.contains("0.010000"));
    }

    #[test]
    fn aggregation_report() {
        let store = MeteringStore::in_memory().unwrap();
        let rates = CostRates::default();

        // Insert some records.
        for _ in 0..3 {
            let r = UsageRecord::new("ws-1", "u1", "agent-a", ResourceType::ApiCalls, 10.0)
                .with_cost(0.001);
            store.insert_record(&r).unwrap();
        }

        let r = UsageRecord::new(
            "ws-1",
            "u1",
            "agent-b",
            ResourceType::LlmTokensInput {
                provider: "openai".into(),
                model: "gpt-4o".into(),
            },
            1000.0,
        )
        .with_cost(0.0025);
        store.insert_record(&r).unwrap();

        let agg = MeteringAggregator::new(&store, &rates);
        let period = TimePeriod::Custom {
            start: chrono::Utc::now() - Duration::hours(1),
            end: chrono::Utc::now() + Duration::hours(1),
        };

        let report = agg.workspace_report("ws-1", &period).unwrap();
        assert_eq!(report.total_api_calls, 30);
        assert_eq!(report.total_llm_tokens, 1000);
        assert_eq!(report.top_agents.len(), 2);
    }

    #[test]
    fn grouped_reports_by_agent() {
        let store = MeteringStore::in_memory().unwrap();
        let rates = CostRates::default();

        for agent in &["a1", "a2", "a3"] {
            let r =
                UsageRecord::new("ws-1", "u1", *agent, ResourceType::ApiCalls, 5.0).with_cost(0.5);
            store.insert_record(&r).unwrap();
        }

        let agg = MeteringAggregator::new(&store, &rates);
        let period = TimePeriod::Custom {
            start: chrono::Utc::now() - Duration::hours(1),
            end: chrono::Utc::now() + Duration::hours(1),
        };

        let reports = agg
            .grouped_reports("ws-1", &period, &GroupBy::Agent)
            .unwrap();
        assert_eq!(reports.len(), 3);
    }
}
