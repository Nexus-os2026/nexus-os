//! Collection points — helpers for instrumenting usage at system boundaries.

use crate::cost::CostRates;
use crate::error::MeteringError;
use crate::store::MeteringStore;
use crate::types::{ResourceType, UsageRecord};

/// Convenience wrapper for emitting metered usage records.
pub struct UsageCollector<'a> {
    store: &'a MeteringStore,
    rates: &'a CostRates,
    workspace_id: String,
    user_id: String,
}

impl<'a> UsageCollector<'a> {
    pub fn new(
        store: &'a MeteringStore,
        rates: &'a CostRates,
        workspace_id: impl Into<String>,
        user_id: impl Into<String>,
    ) -> Self {
        Self {
            store,
            rates,
            workspace_id: workspace_id.into(),
            user_id: user_id.into(),
        }
    }

    /// Record LLM token usage (input + output in one call).
    pub fn record_llm_usage(
        &self,
        agent_did: &str,
        provider: &str,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) -> Result<(), MeteringError> {
        if input_tokens > 0 {
            let rt = ResourceType::LlmTokensInput {
                provider: provider.into(),
                model: model.into(),
            };
            let cost = self.rates.estimate(&rt, input_tokens as f64);
            let record = UsageRecord::new(
                &self.workspace_id,
                &self.user_id,
                agent_did,
                rt,
                input_tokens as f64,
            )
            .with_cost(cost);
            self.store.insert_record(&record)?;
        }

        if output_tokens > 0 {
            let rt = ResourceType::LlmTokensOutput {
                provider: provider.into(),
                model: model.into(),
            };
            let cost = self.rates.estimate(&rt, output_tokens as f64);
            let record = UsageRecord::new(
                &self.workspace_id,
                &self.user_id,
                agent_did,
                rt,
                output_tokens as f64,
            )
            .with_cost(cost);
            self.store.insert_record(&record)?;
        }

        Ok(())
    }

    /// Record fuel consumption.
    pub fn record_fuel(&self, agent_did: &str, fuel_units: u64) -> Result<(), MeteringError> {
        let record = UsageRecord::new(
            &self.workspace_id,
            &self.user_id,
            agent_did,
            ResourceType::AgentFuelConsumed,
            fuel_units as f64,
        );
        self.store.insert_record(&record)
    }

    /// Record sandbox compute time.
    pub fn record_compute(&self, agent_did: &str, seconds: f64) -> Result<(), MeteringError> {
        let rt = ResourceType::SandboxComputeSeconds;
        let cost = self.rates.estimate(&rt, seconds);
        let record = UsageRecord::new(&self.workspace_id, &self.user_id, agent_did, rt, seconds)
            .with_cost(cost);
        self.store.insert_record(&record)
    }

    /// Record API calls.
    pub fn record_api_calls(&self, agent_did: &str, count: u64) -> Result<(), MeteringError> {
        let rt = ResourceType::ApiCalls;
        let cost = self.rates.estimate(&rt, count as f64);
        let record = UsageRecord::new(
            &self.workspace_id,
            &self.user_id,
            agent_did,
            rt,
            count as f64,
        )
        .with_cost(cost);
        self.store.insert_record(&record)
    }

    /// Record storage usage.
    pub fn record_storage(&self, agent_did: &str, bytes: u64) -> Result<(), MeteringError> {
        let rt = ResourceType::StorageBytes;
        let cost = self.rates.estimate(&rt, bytes as f64);
        let record = UsageRecord::new(
            &self.workspace_id,
            &self.user_id,
            agent_did,
            rt,
            bytes as f64,
        )
        .with_cost(cost);
        self.store.insert_record(&record)
    }

    /// Record integration/external API calls.
    pub fn record_integration(
        &self,
        agent_did: &str,
        provider: &str,
        count: u64,
    ) -> Result<(), MeteringError> {
        let rt = ResourceType::IntegrationCalls {
            provider: provider.into(),
        };
        let cost = self.rates.estimate(&rt, count as f64);
        let record = UsageRecord::new(
            &self.workspace_id,
            &self.user_id,
            agent_did,
            rt,
            count as f64,
        )
        .with_cost(cost);
        self.store.insert_record(&record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collector_records_llm_usage() {
        let store = MeteringStore::in_memory().unwrap();
        let rates = CostRates::default();
        let collector = UsageCollector::new(&store, &rates, "ws-1", "u-1");

        collector
            .record_llm_usage("agent-1", "openai", "gpt-4o", 5000, 2000)
            .unwrap();

        let records = store
            .query_records("ws-1", "2000-01-01T00:00:00Z", "2100-01-01T00:00:00Z")
            .unwrap();
        assert_eq!(records.len(), 2); // input + output
    }

    #[test]
    fn collector_records_all_types() {
        let store = MeteringStore::in_memory().unwrap();
        let rates = CostRates::default();
        let collector = UsageCollector::new(&store, &rates, "ws-1", "u-1");

        collector.record_fuel("a1", 100).unwrap();
        collector.record_compute("a1", 60.0).unwrap();
        collector.record_api_calls("a1", 5).unwrap();
        collector.record_storage("a1", 1024).unwrap();
        collector.record_integration("a1", "slack", 1).unwrap();

        let records = store
            .query_records("ws-1", "2000-01-01T00:00:00Z", "2100-01-01T00:00:00Z")
            .unwrap();
        assert_eq!(records.len(), 5);
    }
}
