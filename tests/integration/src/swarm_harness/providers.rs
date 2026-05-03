//! Synthetic provider for the Director's planner LLM.

use async_trait::async_trait;
use nexus_swarm::events::{ProviderHealth, ProviderHealthStatus};
use nexus_swarm::profile::{CostClass, PrivacyClass, ReasoningTier};
use nexus_swarm::provider::{
    InvokeRequest, InvokeResponse, ModelDescriptor, Provider, ProviderCapabilities, ProviderError,
};
use std::sync::Mutex;

/// Synthetic Director-planner provider.
///
/// Returns a single canned `PlanSchema` JSON string on every `invoke`.
/// Token counts and cost are deterministic so scenario tests can assert
/// budget deltas without flake. Privacy class is `Public` and cost is
/// `Free` — the harness never exercises a privacy gate at this layer.
pub struct SyntheticPlannerProvider {
    canned_plan_json: String,
    model_id: String,
    invocations: Mutex<u32>,
}

impl SyntheticPlannerProvider {
    /// Build a provider that returns `plan_json` verbatim from
    /// `invoke().text`. The Director's `parse_plan` consumes the text
    /// and feeds it through `serde_json::from_str::<PlanSchema>`.
    pub fn with_canned_plan(plan_json: impl Into<String>) -> Self {
        Self {
            canned_plan_json: plan_json.into(),
            model_id: "synthetic-planner".to_string(),
            invocations: Mutex::new(0),
        }
    }

    /// Number of times `invoke` has been called. Useful for tests that
    /// want to confirm the Director only contacted the planner once.
    pub fn invocation_count(&self) -> u32 {
        *self.invocations.lock().unwrap()
    }
}

#[async_trait]
impl Provider for SyntheticPlannerProvider {
    fn id(&self) -> &str {
        "synthetic-planner"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            models: vec![ModelDescriptor {
                id: self.model_id.clone(),
                param_count_b: None,
                tier: ReasoningTier::Heavy,
                context_window: 32_000,
            }],
            supports_tool_use: false,
            supports_streaming: false,
            max_context: 32_000,
            cost_class: CostClass::Free,
            privacy_class: PrivacyClass::Public,
        }
    }

    async fn health_check(&self) -> ProviderHealth {
        ProviderHealth {
            provider_id: self.id().to_string(),
            status: ProviderHealthStatus::Ok,
            latency_ms: Some(0),
            models: vec![self.model_id.clone()],
            notes: String::new(),
            checked_at_secs: 0,
        }
    }

    async fn invoke(&self, _req: InvokeRequest) -> Result<InvokeResponse, ProviderError> {
        *self.invocations.lock().unwrap() += 1;
        Ok(InvokeResponse {
            text: self.canned_plan_json.clone(),
            tokens_in: 10,
            tokens_out: 50,
            cost_cents: 0,
            latency_ms: 0,
            model_id: self.model_id.clone(),
        })
    }
}
