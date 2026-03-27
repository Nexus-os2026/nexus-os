use serde::{Deserialize, Serialize};

use crate::coin::NexusCoin;

/// Compute pricing table — maps model tiers to burn costs.
/// Coins burned here leave the total supply permanently.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputePricingTable {
    /// Price per 1K tokens (input) in micronexus, by model size class
    pub prices: Vec<ModelPrice>,
    /// Minimum burn per request (even if 0 tokens)
    pub minimum_burn: NexusCoin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPrice {
    pub model_id: String,
    pub size_class: String,
    pub is_local: bool,
    /// Micronexus per 1K input tokens
    pub input_cost_per_1k: u64,
    /// Micronexus per 1K output tokens
    pub output_cost_per_1k: u64,
}

impl ComputePricingTable {
    pub fn default_pricing() -> Self {
        Self {
            prices: vec![
                // Local models — near-free (still burns a tiny amount for tracking)
                ModelPrice {
                    model_id: "flash-2b".into(),
                    size_class: "Tiny".into(),
                    is_local: true,
                    input_cost_per_1k: 10, // 0.00001 NXC per 1K tokens
                    output_cost_per_1k: 20,
                },
                ModelPrice {
                    model_id: "ollama-7b".into(),
                    size_class: "Small".into(),
                    is_local: true,
                    input_cost_per_1k: 50, // 0.00005 NXC
                    output_cost_per_1k: 100,
                },
                ModelPrice {
                    model_id: "flash-35b".into(),
                    size_class: "Medium".into(),
                    is_local: true,
                    input_cost_per_1k: 200, // 0.0002 NXC
                    output_cost_per_1k: 400,
                },
                // Cloud models — real cost
                ModelPrice {
                    model_id: "nim-llama-8b".into(),
                    size_class: "Small".into(),
                    is_local: false,
                    input_cost_per_1k: 1_000, // 0.001 NXC
                    output_cost_per_1k: 2_000,
                },
                ModelPrice {
                    model_id: "nim-llama-70b".into(),
                    size_class: "Large".into(),
                    is_local: false,
                    input_cost_per_1k: 5_000, // 0.005 NXC
                    output_cost_per_1k: 10_000,
                },
                ModelPrice {
                    model_id: "anthropic-sonnet".into(),
                    size_class: "Frontier".into(),
                    is_local: false,
                    input_cost_per_1k: 15_000,  // 0.015 NXC
                    output_cost_per_1k: 75_000, // 0.075 NXC
                },
            ],
            minimum_burn: NexusCoin::from_micro(5), // 0.000005 NXC minimum
        }
    }

    /// Calculate the burn cost for a specific LLM call
    pub fn calculate_burn(
        &self,
        model_id: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) -> NexusCoin {
        let price = self
            .prices
            .iter()
            .find(|p| p.model_id == model_id)
            .or_else(|| self.prices.iter().find(|p| model_id.contains(&p.model_id)))
            .unwrap_or_else(|| self.prices.last().unwrap());

        let input_cost = (input_tokens as u128 * price.input_cost_per_1k as u128) / 1000;
        let output_cost = (output_tokens as u128 * price.output_cost_per_1k as u128) / 1000;
        let total = NexusCoin::from_micro((input_cost + output_cost) as u64);

        // Enforce minimum burn
        if total < self.minimum_burn {
            self.minimum_burn
        } else {
            total
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_model_cheap() {
        let pricing = ComputePricingTable::default_pricing();
        let cost = pricing.calculate_burn("flash-2b", 1000, 500);
        // 1000 * 10 / 1000 + 500 * 20 / 1000 = 10 + 10 = 20 micro
        assert_eq!(cost, NexusCoin::from_micro(20));
    }

    #[test]
    fn test_frontier_model_expensive() {
        let pricing = ComputePricingTable::default_pricing();
        let cost = pricing.calculate_burn("anthropic-sonnet", 1000, 1000);
        // 1000 * 15000 / 1000 + 1000 * 75000 / 1000 = 15000 + 75000 = 90000 micro
        assert_eq!(cost, NexusCoin::from_micro(90_000));
        // That's 0.09 NXC — much more than local
        assert!(cost > NexusCoin::from_micro(20));
    }

    #[test]
    fn test_minimum_burn_enforced() {
        let pricing = ComputePricingTable::default_pricing();
        // Zero tokens — should still burn minimum
        let cost = pricing.calculate_burn("flash-2b", 0, 0);
        assert_eq!(cost, NexusCoin::from_micro(5));
    }
}
