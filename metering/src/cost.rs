//! Cost estimation engine with configurable rates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::ResourceType;

/// Per-million-token or per-unit cost rates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostRates {
    /// LLM costs per 1M tokens, keyed by `"provider:model:direction"`.
    /// e.g. `"ollama:llama3:input" -> 0.0`
    pub llm_rates: HashMap<String, f64>,
    /// Cost per hour of sandbox compute.
    pub sandbox_compute_per_hour: f64,
    /// Cost per GB per month of storage.
    pub storage_per_gb_month: f64,
    /// Cost per API call (default).
    pub api_call_cost: f64,
    /// Integration-specific per-call costs, keyed by provider.
    pub integration_rates: HashMap<String, f64>,
}

impl Default for CostRates {
    fn default() -> Self {
        let mut llm_rates = HashMap::new();
        // Local (free)
        llm_rates.insert("ollama:*:input".into(), 0.0);
        llm_rates.insert("ollama:*:output".into(), 0.0);
        // OpenAI
        llm_rates.insert("openai:gpt-4o:input".into(), 2.50);
        llm_rates.insert("openai:gpt-4o:output".into(), 10.00);
        // Anthropic
        llm_rates.insert("anthropic:claude-sonnet:input".into(), 3.00);
        llm_rates.insert("anthropic:claude-sonnet:output".into(), 15.00);
        // NVIDIA NIM
        llm_rates.insert("nvidia:nim:input".into(), 0.50);
        llm_rates.insert("nvidia:nim:output".into(), 1.50);

        Self {
            llm_rates,
            sandbox_compute_per_hour: 0.10,
            storage_per_gb_month: 0.023,
            api_call_cost: 0.0001,
            integration_rates: HashMap::new(),
        }
    }
}

impl CostRates {
    /// Estimate cost for a given resource usage.
    pub fn estimate(&self, resource_type: &ResourceType, quantity: f64) -> f64 {
        match resource_type {
            ResourceType::LlmTokensInput { provider, model } => {
                let key = format!("{provider}:{model}:input");
                let wildcard = format!("{provider}:*:input");
                let rate = self
                    .llm_rates
                    .get(&key)
                    .or_else(|| self.llm_rates.get(&wildcard))
                    .copied()
                    .unwrap_or(0.0);
                // Rate is per 1M tokens.
                (quantity / 1_000_000.0) * rate
            }
            ResourceType::LlmTokensOutput { provider, model } => {
                let key = format!("{provider}:{model}:output");
                let wildcard = format!("{provider}:*:output");
                let rate = self
                    .llm_rates
                    .get(&key)
                    .or_else(|| self.llm_rates.get(&wildcard))
                    .copied()
                    .unwrap_or(0.0);
                (quantity / 1_000_000.0) * rate
            }
            ResourceType::SandboxComputeSeconds => {
                (quantity / 3600.0) * self.sandbox_compute_per_hour
            }
            ResourceType::StorageBytes => {
                let gb = quantity / (1024.0 * 1024.0 * 1024.0);
                gb * self.storage_per_gb_month
            }
            ResourceType::ApiCalls => quantity * self.api_call_cost,
            ResourceType::IntegrationCalls { provider } => {
                let rate = self
                    .integration_rates
                    .get(provider)
                    .copied()
                    .unwrap_or(self.api_call_cost);
                quantity * rate
            }
            // Fuel is an internal unit — no direct dollar cost.
            ResourceType::AgentFuelConsumed => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_openai_tokens() {
        let rates = CostRates::default();
        let rt = ResourceType::LlmTokensInput {
            provider: "openai".into(),
            model: "gpt-4o".into(),
        };
        let cost = rates.estimate(&rt, 1_000_000.0);
        assert!((cost - 2.50).abs() < 0.001);
    }

    #[test]
    fn cost_ollama_free() {
        let rates = CostRates::default();
        let rt = ResourceType::LlmTokensOutput {
            provider: "ollama".into(),
            model: "llama3".into(),
        };
        assert_eq!(rates.estimate(&rt, 5_000_000.0), 0.0);
    }

    #[test]
    fn cost_compute_seconds() {
        let rates = CostRates::default();
        // 1 hour = 3600 seconds => $0.10
        let cost = rates.estimate(&ResourceType::SandboxComputeSeconds, 3600.0);
        assert!((cost - 0.10).abs() < 0.001);
    }

    #[test]
    fn cost_storage() {
        let rates = CostRates::default();
        let one_gb = 1024.0 * 1024.0 * 1024.0;
        let cost = rates.estimate(&ResourceType::StorageBytes, one_gb);
        assert!((cost - 0.023).abs() < 0.001);
    }

    #[test]
    fn cost_fuel_is_zero() {
        let rates = CostRates::default();
        assert_eq!(rates.estimate(&ResourceType::AgentFuelConsumed, 999.0), 0.0);
    }

    #[test]
    fn cost_api_calls() {
        let rates = CostRates::default();
        let cost = rates.estimate(&ResourceType::ApiCalls, 10_000.0);
        assert!((cost - 1.0).abs() < 0.001); // 10000 * 0.0001 = 1.0
    }

    #[test]
    fn cost_integration_calls_custom_rate() {
        let mut rates = CostRates::default();
        rates.integration_rates.insert("slack".into(), 0.005);
        let cost = rates.estimate(
            &ResourceType::IntegrationCalls {
                provider: "slack".into(),
            },
            100.0,
        );
        assert!((cost - 0.5).abs() < 0.001);
    }

    #[test]
    fn cost_integration_calls_falls_back_to_api_cost() {
        let rates = CostRates::default();
        let cost = rates.estimate(
            &ResourceType::IntegrationCalls {
                provider: "unknown".into(),
            },
            100.0,
        );
        // Falls back to api_call_cost: 0.0001 * 100 = 0.01
        assert!((cost - 0.01).abs() < 0.001);
    }

    #[test]
    fn cost_unknown_llm_provider_is_zero() {
        let rates = CostRates::default();
        let rt = ResourceType::LlmTokensInput {
            provider: "unknown_provider".into(),
            model: "unknown_model".into(),
        };
        assert_eq!(rates.estimate(&rt, 1_000_000.0), 0.0);
    }

    #[test]
    fn cost_rates_serde_roundtrip() {
        let rates = CostRates::default();
        let json = serde_json::to_string(&rates).unwrap();
        let parsed: CostRates = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.api_call_cost, rates.api_call_cost);
        assert_eq!(parsed.llm_rates.len(), rates.llm_rates.len());
    }
}
