//! Provider routing with strategy-based selection and circuit breaker integration.

use crate::circuit_breaker::ProviderCircuitBreaker;
use crate::providers::{LlmProvider, LlmResponse};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingStrategy {
    Priority,
    RoundRobin,
    LowestLatency,
    CostOptimized,
}

struct ProviderEntry {
    provider: Box<dyn LlmProvider>,
    breaker: ProviderCircuitBreaker,
    avg_latency_ms: f64,
    request_count: u64,
}

pub struct ProviderRouter {
    entries: Vec<ProviderEntry>,
    strategy: RoutingStrategy,
    round_robin_index: usize,
}

impl ProviderRouter {
    pub fn new(strategy: RoutingStrategy) -> Self {
        Self {
            entries: Vec::new(),
            strategy,
            round_robin_index: 0,
        }
    }

    pub fn add_provider(&mut self, provider: Box<dyn LlmProvider>) {
        self.entries.push(ProviderEntry {
            provider,
            breaker: ProviderCircuitBreaker::default(),
            avg_latency_ms: 0.0,
            request_count: 0,
        });
    }

    pub fn add_provider_with_breaker(
        &mut self,
        provider: Box<dyn LlmProvider>,
        breaker: ProviderCircuitBreaker,
    ) {
        self.entries.push(ProviderEntry {
            provider,
            breaker,
            avg_latency_ms: 0.0,
            request_count: 0,
        });
    }

    pub fn strategy(&self) -> RoutingStrategy {
        self.strategy
    }

    pub fn set_strategy(&mut self, strategy: RoutingStrategy) {
        self.strategy = strategy;
    }

    /// Route a request to the best available provider based on the current strategy.
    pub fn route(
        &mut self,
        prompt: &str,
        max_tokens: u32,
        model: &str,
    ) -> Result<LlmResponse, AgentError> {
        let order = self.provider_order();

        if order.is_empty() {
            return Err(AgentError::SupervisorError(
                "AllProvidersUnavailable".to_string(),
            ));
        }

        let mut last_error = None;

        for idx in order {
            if !self.entries[idx].breaker.allow_request() {
                continue;
            }

            let start = Instant::now();
            match self.entries[idx].provider.query(prompt, max_tokens, model) {
                Ok(response) => {
                    let elapsed = start.elapsed();
                    self.entries[idx].breaker.record_success();
                    self.update_latency(idx, elapsed);
                    return Ok(response);
                }
                Err(e) => {
                    self.entries[idx].breaker.record_failure();
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            AgentError::SupervisorError("AllProvidersUnavailable".to_string())
        }))
    }

    fn provider_order(&mut self) -> Vec<usize> {
        match self.strategy {
            RoutingStrategy::Priority => (0..self.entries.len()).collect(),
            RoutingStrategy::RoundRobin => {
                let len = self.entries.len();
                if len == 0 {
                    return vec![];
                }
                let start = self.round_robin_index % len;
                self.round_robin_index = (start + 1) % len;
                let mut order: Vec<usize> = (start..len).collect();
                order.extend(0..start);
                order
            }
            RoutingStrategy::LowestLatency => {
                let mut indices: Vec<usize> = (0..self.entries.len()).collect();
                indices.sort_by(|&a, &b| {
                    self.entries[a]
                        .avg_latency_ms
                        .partial_cmp(&self.entries[b].avg_latency_ms)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                indices
            }
            RoutingStrategy::CostOptimized => {
                let mut indices: Vec<usize> = (0..self.entries.len()).collect();
                indices.sort_by(|&a, &b| {
                    self.entries[a]
                        .provider
                        .cost_per_token()
                        .partial_cmp(&self.entries[b].provider.cost_per_token())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                indices
            }
        }
    }

    fn update_latency(&mut self, idx: usize, elapsed: Duration) {
        let entry = &mut self.entries[idx];
        let ms = elapsed.as_secs_f64() * 1000.0;
        entry.request_count += 1;
        let n = entry.request_count as f64;
        entry.avg_latency_ms = entry.avg_latency_ms * ((n - 1.0) / n) + ms / n;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::LlmResponse;

    struct TestProvider {
        provider_name: String,
        cost: f64,
        should_fail: bool,
    }

    impl TestProvider {
        fn new(name: &str, cost: f64, should_fail: bool) -> Self {
            Self {
                provider_name: name.to_string(),
                cost,
                should_fail,
            }
        }
    }

    impl LlmProvider for TestProvider {
        fn query(
            &self,
            _prompt: &str,
            _max_tokens: u32,
            _model: &str,
        ) -> Result<LlmResponse, AgentError> {
            if self.should_fail {
                Err(AgentError::SupervisorError(format!(
                    "{} failed",
                    self.provider_name
                )))
            } else {
                Ok(LlmResponse {
                    output_text: format!("response from {}", self.provider_name),
                    token_count: 10,
                    model_name: "test".to_string(),
                    tool_calls: vec![],
                })
            }
        }

        fn name(&self) -> &str {
            &self.provider_name
        }

        fn cost_per_token(&self) -> f64 {
            self.cost
        }
    }

    #[test]
    fn router_fallback_on_failure() {
        let mut router = ProviderRouter::new(RoutingStrategy::Priority);
        router.add_provider(Box::new(TestProvider::new("primary", 0.01, true)));
        router.add_provider(Box::new(TestProvider::new("fallback", 0.02, false)));

        let result = router.route("hello", 100, "test");
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert!(resp.output_text.contains("fallback"));
    }

    #[test]
    fn all_providers_unavailable() {
        let mut router = ProviderRouter::new(RoutingStrategy::Priority);
        router.add_provider(Box::new(TestProvider::new("a", 0.01, true)));
        router.add_provider(Box::new(TestProvider::new("b", 0.02, true)));

        let result = router.route("hello", 100, "test");
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("failed"));
    }

    #[test]
    fn cost_optimized_picks_cheapest() {
        let mut router = ProviderRouter::new(RoutingStrategy::CostOptimized);
        router.add_provider(Box::new(TestProvider::new("expensive", 0.10, false)));
        router.add_provider(Box::new(TestProvider::new("cheap", 0.001, false)));
        router.add_provider(Box::new(TestProvider::new("medium", 0.05, false)));

        let result = router.route("hello", 100, "test");
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert!(resp.output_text.contains("cheap"));
    }

    #[test]
    fn circuit_breaker_integration_with_router() {
        let mut router = ProviderRouter::new(RoutingStrategy::Priority);
        router.add_provider(Box::new(TestProvider::new("primary", 0.01, true)));
        router.add_provider(Box::new(TestProvider::new("fallback", 0.02, false)));

        // After enough failures, the primary's circuit should open
        for _ in 0..5 {
            let _ = router.route("hello", 100, "test");
        }

        // Now primary's breaker is open, should go directly to fallback
        let result = router.route("hello", 100, "test");
        assert!(result.is_ok());
        assert!(result.unwrap().output_text.contains("fallback"));
    }

    #[test]
    fn empty_router_returns_error() {
        let mut router = ProviderRouter::new(RoutingStrategy::Priority);
        let result = router.route("hello", 100, "test");
        assert!(result.is_err());
    }
}
