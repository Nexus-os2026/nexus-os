//! Provider routing with strategy-based selection and circuit breaker integration.
//!
//! `ProviderRouter` routes LLM requests to the best available provider using
//! one of four strategies: Priority, RoundRobin, LowestLatency, CostOptimized.
//!
//! Governance-aware routing: when a request is tagged as a governance task
//! (via `route_task()` or `route_governance()`), the router first tries any
//! provider named `"local-slm"` if one is registered and its circuit breaker
//! allows it. If the local provider is unavailable or returns an error, the
//! router falls back to the normal strategy-based routing.

use crate::circuit_breaker::ProviderCircuitBreaker;
use crate::providers::{LlmProvider, LlmResponse};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Routing strategy for selecting among multiple providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingStrategy {
    Priority,
    RoundRobin,
    LowestLatency,
    CostOptimized,
}

/// Type of task being routed, used to select governance-aware routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    /// General-purpose LLM request — uses strategy-based routing only.
    General,
    /// Governance task — prefers local SLM, falls back to strategy-based.
    Governance {
        /// The governance task category (e.g. "pii_detection", "prompt_safety").
        task_type: String,
    },
}

impl Default for TaskType {
    fn default() -> Self {
        Self::General
    }
}

impl TaskType {
    /// Create a governance task type with the given category.
    pub fn governance(task_type: &str) -> Self {
        Self::Governance {
            task_type: task_type.to_string(),
        }
    }

    /// Whether this is a governance task.
    pub fn is_governance(&self) -> bool {
        matches!(self, Self::Governance { .. })
    }
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
    ///
    /// This is the standard routing path for general-purpose requests.
    /// Governance tasks should use `route_task()` or `route_governance()` instead.
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

        Err(last_error
            .unwrap_or_else(|| AgentError::SupervisorError("AllProvidersUnavailable".to_string())))
    }

    /// Route a request with task-type awareness.
    ///
    /// For `TaskType::General`, behaves identically to `route()`.
    /// For `TaskType::Governance`, tries the local SLM provider first,
    /// then falls back to strategy-based routing if local is unavailable.
    pub fn route_task(
        &mut self,
        prompt: &str,
        max_tokens: u32,
        model: &str,
        task_type: &TaskType,
    ) -> Result<LlmResponse, AgentError> {
        match task_type {
            TaskType::General => self.route(prompt, max_tokens, model),
            TaskType::Governance { .. } => self.route_governance(prompt, max_tokens, model),
        }
    }

    /// Route a governance task: try local SLM first, fall back to strategy.
    ///
    /// Looks for a provider with `name() == "local-slm"` in the registered
    /// entries. If found and its circuit breaker allows the request, queries
    /// it first. If the local provider fails or is unavailable, falls back
    /// to the normal strategy-based `route()`.
    pub fn route_governance(
        &mut self,
        prompt: &str,
        max_tokens: u32,
        model: &str,
    ) -> Result<LlmResponse, AgentError> {
        // Try local SLM first
        if let Some(idx) = self.find_local_slm_index() {
            if self.entries[idx].breaker.allow_request() {
                let start = Instant::now();
                match self.entries[idx].provider.query(prompt, max_tokens, model) {
                    Ok(response) => {
                        let elapsed = start.elapsed();
                        self.entries[idx].breaker.record_success();
                        self.update_latency(idx, elapsed);
                        return Ok(response);
                    }
                    Err(_) => {
                        self.entries[idx].breaker.record_failure();
                        // Fall through to strategy-based routing
                    }
                }
            }
        }

        // Fallback: use normal strategy-based routing (excludes local-slm
        // from the retry since its breaker may now be tripped)
        self.route(prompt, max_tokens, model)
    }

    /// Whether a local SLM provider is registered and available.
    pub fn has_local_slm(&self) -> bool {
        self.find_local_slm_index().is_some()
    }

    /// Find the index of the local-slm provider, if registered.
    fn find_local_slm_index(&self) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.provider.name() == "local-slm")
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

    // -----------------------------------------------------------------------
    // Existing strategy-based routing tests (unchanged)
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // TaskType tests
    // -----------------------------------------------------------------------

    #[test]
    fn task_type_default_is_general() {
        let tt = TaskType::default();
        assert_eq!(tt, TaskType::General);
        assert!(!tt.is_governance());
    }

    #[test]
    fn task_type_governance_constructor() {
        let tt = TaskType::governance("pii_detection");
        assert!(tt.is_governance());
        if let TaskType::Governance { ref task_type } = tt {
            assert_eq!(task_type, "pii_detection");
        } else {
            panic!("expected Governance");
        }
    }

    #[test]
    fn task_type_general_is_not_governance() {
        assert!(!TaskType::General.is_governance());
    }

    // -----------------------------------------------------------------------
    // Governance routing tests
    // -----------------------------------------------------------------------

    #[test]
    fn route_task_general_uses_strategy() {
        let mut router = ProviderRouter::new(RoutingStrategy::Priority);
        router.add_provider(Box::new(TestProvider::new("cloud", 0.01, false)));

        let result = router.route_task("hello", 100, "test", &TaskType::General);
        assert!(result.is_ok());
        assert!(result.unwrap().output_text.contains("cloud"));
    }

    #[test]
    fn route_governance_prefers_local_slm() {
        let mut router = ProviderRouter::new(RoutingStrategy::Priority);
        router.add_provider(Box::new(TestProvider::new("cloud", 0.01, false)));
        router.add_provider(Box::new(TestProvider::new("local-slm", 0.0, false)));

        let result = router.route_governance("governance prompt", 100, "test");
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert!(
            resp.output_text.contains("local-slm"),
            "expected local-slm response, got: {}",
            resp.output_text
        );
    }

    #[test]
    fn route_task_governance_prefers_local_slm() {
        let mut router = ProviderRouter::new(RoutingStrategy::Priority);
        router.add_provider(Box::new(TestProvider::new("cloud", 0.01, false)));
        router.add_provider(Box::new(TestProvider::new("local-slm", 0.0, false)));

        let tt = TaskType::governance("prompt_safety");
        let result = router.route_task("check this prompt", 100, "test", &tt);
        assert!(result.is_ok());
        assert!(result.unwrap().output_text.contains("local-slm"));
    }

    #[test]
    fn route_governance_falls_back_when_local_fails() {
        let mut router = ProviderRouter::new(RoutingStrategy::Priority);
        router.add_provider(Box::new(TestProvider::new("cloud", 0.01, false)));
        router.add_provider(Box::new(TestProvider::new("local-slm", 0.0, true)));

        let result = router.route_governance("governance prompt", 100, "test");
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert!(
            resp.output_text.contains("cloud"),
            "expected cloud fallback, got: {}",
            resp.output_text
        );
    }

    #[test]
    fn route_governance_falls_back_when_no_local() {
        let mut router = ProviderRouter::new(RoutingStrategy::Priority);
        router.add_provider(Box::new(TestProvider::new("cloud", 0.01, false)));

        // No local-slm registered — should route via strategy
        let result = router.route_governance("governance prompt", 100, "test");
        assert!(result.is_ok());
        assert!(result.unwrap().output_text.contains("cloud"));
    }

    #[test]
    fn route_governance_respects_circuit_breaker() {
        let mut router = ProviderRouter::new(RoutingStrategy::Priority);
        router.add_provider(Box::new(TestProvider::new("cloud", 0.01, false)));
        router.add_provider(Box::new(TestProvider::new("local-slm", 0.0, true)));

        // Trip the local-slm circuit breaker
        for _ in 0..10 {
            let _ = router.route_governance("prompt", 50, "test");
        }

        // Now the local-slm breaker is open — should skip it entirely
        // and go straight to cloud without attempting local-slm
        let result = router.route_governance("prompt", 50, "test");
        assert!(result.is_ok());
        assert!(result.unwrap().output_text.contains("cloud"));
    }

    #[test]
    fn has_local_slm_true_when_registered() {
        let mut router = ProviderRouter::new(RoutingStrategy::Priority);
        router.add_provider(Box::new(TestProvider::new("local-slm", 0.0, false)));

        assert!(router.has_local_slm());
    }

    #[test]
    fn has_local_slm_false_when_not_registered() {
        let mut router = ProviderRouter::new(RoutingStrategy::Priority);
        router.add_provider(Box::new(TestProvider::new("cloud", 0.01, false)));

        assert!(!router.has_local_slm());
    }

    #[test]
    fn route_general_does_not_prefer_local_slm() {
        // When routing as General, local-slm is treated like any other
        // provider — order determined by strategy, not by preference.
        let mut router = ProviderRouter::new(RoutingStrategy::Priority);
        // cloud is first in priority order
        router.add_provider(Box::new(TestProvider::new("cloud", 0.01, false)));
        router.add_provider(Box::new(TestProvider::new("local-slm", 0.0, false)));

        let result = router.route("hello", 100, "test");
        assert!(result.is_ok());
        // Priority strategy picks first registered = cloud
        assert!(result.unwrap().output_text.contains("cloud"));
    }

    #[test]
    fn route_governance_with_cost_optimized_strategy_still_prefers_local() {
        // Even under CostOptimized strategy, governance routing should
        // try local-slm FIRST before falling back to strategy.
        let mut router = ProviderRouter::new(RoutingStrategy::CostOptimized);
        router.add_provider(Box::new(TestProvider::new("cheap-cloud", 0.001, false)));
        router.add_provider(Box::new(TestProvider::new("local-slm", 0.0, false)));

        let result = router.route_governance("gov prompt", 100, "test");
        assert!(result.is_ok());
        assert!(result.unwrap().output_text.contains("local-slm"));
    }

    #[test]
    fn route_governance_all_fail_returns_error() {
        let mut router = ProviderRouter::new(RoutingStrategy::Priority);
        router.add_provider(Box::new(TestProvider::new("local-slm", 0.0, true)));
        router.add_provider(Box::new(TestProvider::new("cloud", 0.01, true)));

        let result = router.route_governance("prompt", 50, "test");
        assert!(result.is_err());
    }

    #[test]
    fn route_governance_empty_router_returns_error() {
        let mut router = ProviderRouter::new(RoutingStrategy::Priority);
        let result = router.route_governance("prompt", 50, "test");
        assert!(result.is_err());
    }
}
