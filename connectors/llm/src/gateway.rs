use crate::providers::{
    ClaudeProvider, DeepSeekProvider, LlmProvider, LlmResponse, MockProvider, OllamaProvider,
};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::fuel_hardening::{
    AgentFuelLedger, BudgetPeriodId, BurnAnomalyDetector, FuelAuditReport, FuelToTokenModel,
    ModelCost,
};
use nexus_kernel::redaction::{RedactionEngine, RedactionMetrics, RedactionPolicy};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::env;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRuntimeContext {
    pub agent_id: Uuid,
    pub capabilities: HashSet<String>,
    pub fuel_remaining: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OracleEvent {
    pub agent_id: Uuid,
    pub prompt_hash: String,
    pub response_hash: String,
    pub model_name: String,
    pub response_text: String,
    pub token_count: u32,
    pub cost: f64,
    pub cost_units: u64,
    pub latency_ms: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderSelectionConfig {
    pub provider: Option<String>,
    pub ollama_url: Option<String>,
    pub deepseek_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
}

impl ProviderSelectionConfig {
    pub fn from_env() -> Self {
        Self {
            provider: env::var("LLM_PROVIDER").ok(),
            ollama_url: env::var("OLLAMA_URL").ok(),
            deepseek_api_key: env::var("DEEPSEEK_API_KEY").ok(),
            anthropic_api_key: env::var("ANTHROPIC_API_KEY").ok(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentFuelBudgetConfig {
    pub period_id: BudgetPeriodId,
    pub cap_units: u64,
}

impl Default for AgentFuelBudgetConfig {
    fn default() -> Self {
        Self {
            period_id: BudgetPeriodId::new("period.default"),
            cap_units: 100_000,
        }
    }
}

pub fn select_provider(config: &ProviderSelectionConfig) -> Box<dyn LlmProvider> {
    if let Some(explicit) = config.provider.as_deref() {
        return explicit_provider(explicit, config);
    }

    if let Some(url) = config.ollama_url.as_deref() {
        return Box::new(OllamaProvider::new(url.to_string()));
    }

    if config
        .deepseek_api_key
        .as_deref()
        .map(|key| !key.trim().is_empty())
        .unwrap_or(false)
    {
        return Box::new(DeepSeekProvider::new(config.deepseek_api_key.clone()));
    }

    #[cfg(feature = "real-claude")]
    if config
        .anthropic_api_key
        .as_deref()
        .map(|key| !key.trim().is_empty())
        .unwrap_or(false)
    {
        return Box::new(ClaudeProvider::new(config.anthropic_api_key.clone()));
    }

    Box::new(MockProvider::new())
}

fn explicit_provider(explicit: &str, config: &ProviderSelectionConfig) -> Box<dyn LlmProvider> {
    match explicit.to_lowercase().as_str() {
        "ollama" => Box::new(OllamaProvider::new(
            config
                .ollama_url
                .clone()
                .unwrap_or_else(|| "http://localhost:11434".to_string()),
        )),
        "deepseek" => Box::new(DeepSeekProvider::new(config.deepseek_api_key.clone())),
        "claude" | "anthropic" => Box::new(ClaudeProvider::new(config.anthropic_api_key.clone())),
        "mock" => Box::new(MockProvider::new()),
        _ => Box::new(MockProvider::new()),
    }
}

#[derive(Debug)]
pub struct GovernedLlmGateway<P: LlmProvider> {
    provider: P,
    audit_trail: AuditTrail,
    oracle_events: Vec<OracleEvent>,
    redaction_engine: RedactionEngine,
    fuel_model: FuelToTokenModel,
    default_period_id: BudgetPeriodId,
    fuel_ledgers: HashMap<Uuid, AgentFuelLedger>,
}

impl<P: LlmProvider> GovernedLlmGateway<P> {
    pub fn new(provider: P) -> Self {
        Self::with_redaction_policy(provider, RedactionPolicy::default())
    }

    pub fn with_redaction_policy(provider: P, policy: RedactionPolicy) -> Self {
        Self {
            provider,
            audit_trail: AuditTrail::new(),
            oracle_events: Vec::new(),
            redaction_engine: RedactionEngine::new(policy),
            fuel_model: FuelToTokenModel::with_defaults(),
            default_period_id: BudgetPeriodId::new("period.default"),
            fuel_ledgers: HashMap::new(),
        }
    }

    pub fn set_default_period(&mut self, period_id: impl Into<String>) {
        self.default_period_id = BudgetPeriodId::new(period_id);
    }

    pub fn set_model_cost(&mut self, model: impl Into<String>, cost: ModelCost) {
        self.fuel_model.insert(model, cost);
    }

    pub fn configure_agent_budget(&mut self, agent_id: Uuid, config: AgentFuelBudgetConfig) {
        self.fuel_ledgers.insert(
            agent_id,
            AgentFuelLedger::new(
                config.period_id,
                config.cap_units,
                BurnAnomalyDetector::default(),
            ),
        );
    }

    pub fn fuel_audit_report(&self, agent_id: Uuid) -> Option<FuelAuditReport> {
        self.fuel_ledgers
            .get(&agent_id)
            .map(|ledger| ledger.snapshot(agent_id))
    }

    pub fn query(
        &mut self,
        agent: &mut AgentRuntimeContext,
        prompt: &str,
        max_tokens: u32,
        model: &str,
    ) -> Result<LlmResponse, AgentError> {
        if !agent.capabilities.contains("llm.query") {
            return Err(AgentError::CapabilityDenied("llm.query".to_string()));
        }

        let estimated_tokens = u64::from(max_tokens);
        if self.provider.is_paid() && self.provider.requires_real_api_opt_in() {
            let estimated_cost = f64::from(max_tokens) * self.provider.cost_per_token();
            if agent.fuel_remaining < estimated_tokens || estimated_cost.is_sign_negative() {
                return Err(AgentError::FuelExhausted);
            }
        }

        let redaction_result = self.redaction_engine.process_prompt(
            "llm.query",
            "strict",
            vec![agent.agent_id.to_string(), model.to_string()],
            prompt,
        );
        let _ = self.audit_trail.append_event(
            agent.agent_id,
            EventType::LlmCall,
            json!({
                "event_kind": "redaction.scanned",
                "operation": "llm.query",
                "mode": format!("{:?}", self.redaction_engine.policy().mode),
                "counts_by_kind": redaction_result.summary.counts_by_kind,
                "payload_hash": redaction_result.payload_hash_hex,
                "context_ids": redaction_result.envelope.context_ids,
                "total_findings": redaction_result.summary.total_findings
            }),
        );
        let _ = self.audit_trail.append_event(
            agent.agent_id,
            EventType::LlmCall,
            json!({
                "event_kind": "redaction.applied",
                "operation": "llm.query",
                "required_action": "send_redacted_only",
                "counts_by_kind": redaction_result.summary.counts_by_kind,
                "payload_hash": redaction_result.payload_hash_hex,
                "redacted_hash": redaction_result.redacted_hash_hex,
                "prompt_envelope_hash": redaction_result.outbound_prompt_hash_hex
            }),
        );

        let started = Instant::now();
        let response = self
            .provider
            .query(redaction_result.outbound_prompt.as_str(), max_tokens, model)?;
        let latency_ms = started.elapsed().as_millis() as u64;

        let actual_tokens = u64::from(response.token_count);
        if agent.fuel_remaining < actual_tokens {
            return Err(AgentError::FuelExhausted);
        }
        agent.fuel_remaining -= actual_tokens;

        let estimated_input_tokens = self
            .provider
            .estimate_input_tokens(redaction_result.outbound_prompt.as_str());
        let (input_tokens, output_tokens) =
            estimate_token_split(estimated_input_tokens, response.token_count);

        let fallback_cost_per_1k = provider_cost_to_per_1k(self.provider.cost_per_token());
        let fallback_cost = ModelCost {
            cost_per_1k_input: fallback_cost_per_1k,
            cost_per_1k_output: fallback_cost_per_1k,
        };
        let using_fallback_model = !self.fuel_model.models.contains_key(model);
        let cost_units = self.fuel_model.simulate_cost_with_fallback(
            model,
            input_tokens,
            output_tokens,
            fallback_cost.clone(),
        );

        if using_fallback_model {
            let _ = self.audit_trail.append_event(
                agent.agent_id,
                EventType::UserAction,
                json!({
                    "event_kind": "fuel.model_cost_fallback",
                    "agent_id": agent.agent_id,
                    "model": model,
                    "fallback_cost_per_1k_input": fallback_cost.cost_per_1k_input,
                    "fallback_cost_per_1k_output": fallback_cost.cost_per_1k_output,
                }),
            );
        }

        let _ = self.audit_trail.append_event(
            agent.agent_id,
            EventType::LlmCall,
            json!({
                "event_kind": "fuel.token_usage_estimated",
                "agent_id": agent.agent_id,
                "model": model,
                "estimated_input_tokens": input_tokens,
                "estimated_output_tokens": output_tokens,
                "provider_total_tokens": response.token_count,
            }),
        );

        let ledger_default_cap = agent.fuel_remaining.saturating_add(actual_tokens).max(1);
        let ledger_entry = self.fuel_ledgers.entry(agent.agent_id).or_insert_with(|| {
            AgentFuelLedger::new(
                self.default_period_id.clone(),
                ledger_default_cap,
                BurnAnomalyDetector::default(),
            )
        });

        match ledger_entry.record_llm_spend(
            agent.agent_id,
            model,
            input_tokens,
            output_tokens,
            cost_units,
            &mut self.audit_trail,
        ) {
            Ok(()) => {}
            Err(violation) => {
                let reason =
                    format!("fuel hardening violation while recording spend for model '{model}'");
                ledger_entry.register_violation(
                    agent.agent_id,
                    violation.clone(),
                    reason.as_str(),
                    &mut self.audit_trail,
                );
                let _ = self.audit_trail.append_event(
                    agent.agent_id,
                    EventType::UserAction,
                    json!({
                        "event_kind": "autonomy.level_changed",
                        "agent_id": agent.agent_id,
                        "previous_level": "unknown",
                        "new_level": 0,
                        "reason": reason,
                    }),
                );
                agent.fuel_remaining = 0;
                return Err(AgentError::FuelViolation {
                    violation,
                    reason: "fuel violation during LLM query".to_string(),
                });
            }
        }

        let cost = if fallback_cost_per_1k == 0 {
            f64::from(response.token_count) * self.provider.cost_per_token()
        } else {
            cost_units as f64 / 1_000.0
        };
        let prompt_hash = redaction_result.outbound_prompt_hash_hex;
        let response_hash = sha256_hex(response.output_text.as_bytes());
        let timestamp = current_unix_timestamp();

        let payload = json!({
            "event_kind": "OracleEvent",
            "prompt_hash": prompt_hash,
            "response_hash": response_hash,
            "model": response.model_name,
            "tokens": response.token_count,
            "cost": cost,
            "cost_units": cost_units,
            "latency_ms": latency_ms,
            "provider_name": self.provider.name(),
            "timestamp": timestamp
        });
        let _ = self
            .audit_trail
            .append_event(agent.agent_id, EventType::LlmCall, payload);

        self.oracle_events.push(OracleEvent {
            agent_id: agent.agent_id,
            prompt_hash,
            response_hash,
            model_name: response.model_name.clone(),
            response_text: response.output_text.clone(),
            token_count: response.token_count,
            cost,
            cost_units,
            latency_ms,
            timestamp,
        });

        Ok(response)
    }

    pub fn audit_trail(&self) -> &AuditTrail {
        &self.audit_trail
    }

    pub fn audit_trail_mut(&mut self) -> &mut AuditTrail {
        &mut self.audit_trail
    }

    pub fn oracle_events(&self) -> &[OracleEvent] {
        &self.oracle_events
    }

    pub fn redaction_metrics(&self) -> &RedactionMetrics {
        self.redaction_engine.metrics()
    }

    pub fn redaction_zero_pii_leakage_kpi(&self) -> bool {
        self.redaction_engine.metrics().zero_pii_leakage_kpi()
    }
}

fn estimate_token_split(estimated_input_tokens: u32, total_tokens: u32) -> (u32, u32) {
    if total_tokens <= estimated_input_tokens {
        (total_tokens, 0)
    } else {
        (
            estimated_input_tokens,
            total_tokens.saturating_sub(estimated_input_tokens),
        )
    }
}

fn provider_cost_to_per_1k(cost_per_token: f64) -> u64 {
    if !cost_per_token.is_finite() || cost_per_token.is_sign_negative() {
        return 0;
    }

    let scaled = cost_per_token * 1_000.0;
    if scaled > u64::MAX as f64 {
        u64::MAX
    } else {
        scaled.round() as u64
    }
}

fn sha256_hex(input: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    format!("{:x}", hasher.finalize())
}

fn current_unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        select_provider, AgentFuelBudgetConfig, AgentRuntimeContext, GovernedLlmGateway,
        ProviderSelectionConfig,
    };
    use crate::providers::{LlmProvider, LlmResponse, MockProvider};
    use nexus_kernel::errors::AgentError;
    use nexus_kernel::fuel_hardening::{BudgetPeriodId, FuelViolation, ModelCost};
    use std::collections::HashSet;
    use uuid::Uuid;

    fn capabilities(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[derive(Debug, Default)]
    struct MeteredProvider;

    impl LlmProvider for MeteredProvider {
        fn query(
            &self,
            _prompt: &str,
            max_tokens: u32,
            model: &str,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                output_text: "metered response".to_string(),
                token_count: max_tokens.min(50),
                model_name: model.to_string(),
                tool_calls: Vec::new(),
            })
        }

        fn name(&self) -> &str {
            "metered"
        }

        fn cost_per_token(&self) -> f64 {
            0.1
        }

        fn requires_real_api_opt_in(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_governed_query_deducts_fuel() {
        let provider = MockProvider::new();
        let mut gateway = GovernedLlmGateway::new(provider);
        let mut context = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities: capabilities(&["llm.query"]),
            fuel_remaining: 1_000,
        };

        let result = gateway.query(&mut context, "What is zero trust?", 50, "mock-1");
        assert!(result.is_ok());
        assert_eq!(context.fuel_remaining, 950);
    }

    #[test]
    fn test_capability_denied() {
        let provider = MockProvider::new();
        let mut gateway = GovernedLlmGateway::new(provider);
        let mut context = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities: capabilities(&["web.search"]),
            fuel_remaining: 1_000,
        };

        let result = gateway.query(&mut context, "Hello", 50, "mock-1");
        assert_eq!(
            result,
            Err(AgentError::CapabilityDenied("llm.query".to_string()))
        );
    }

    #[test]
    fn test_fuel_exhausted_blocks_query() {
        let provider = MockProvider::new();
        let mut gateway = GovernedLlmGateway::new(provider);
        let mut context = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities: capabilities(&["llm.query"]),
            fuel_remaining: 10,
        };

        let result = gateway.query(&mut context, "Large request", 500, "mock-1");
        assert_eq!(result, Err(AgentError::FuelExhausted));
    }

    #[test]
    fn test_response_cached_as_oracle() {
        let provider = MockProvider::new();
        let mut gateway = GovernedLlmGateway::new(provider);
        let agent_id = Uuid::new_v4();
        let mut context = AgentRuntimeContext {
            agent_id,
            capabilities: capabilities(&["llm.query"]),
            fuel_remaining: 1_000,
        };

        let result = gateway.query(&mut context, "Return audit-safe output", 25, "mock-1");
        assert!(result.is_ok());
        assert_eq!(gateway.oracle_events().len(), 1);

        let mut found = false;
        for event in gateway.audit_trail().events() {
            let event_kind = event
                .payload
                .get("event_kind")
                .and_then(|value| value.as_str());
            let response_hash = event
                .payload
                .get("response_hash")
                .and_then(|value| value.as_str());
            if event.agent_id == agent_id
                && event_kind == Some("OracleEvent")
                && response_hash.is_some()
            {
                found = true;
                break;
            }
        }
        assert!(found);
    }

    #[test]
    fn test_cost_calculation() {
        let provider = MeteredProvider;
        let mut gateway = GovernedLlmGateway::new(provider);
        let mut context = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities: capabilities(&["llm.query"]),
            fuel_remaining: 1_000,
        };

        let result = gateway.query(&mut context, "hello", 20, "metered-model");
        assert!(result.is_ok());

        let last = gateway.oracle_events().last();
        assert!(last.is_some());
        if let Some(event) = last {
            assert!(event.cost >= 0.0);
            assert_eq!(event.model_name, "metered-model");
            assert!(event.latency_ms <= 10_000);
        }
    }

    #[test]
    fn test_selection_prefers_mock_when_unconfigured() {
        let config = ProviderSelectionConfig::default();
        let provider = select_provider(&config);
        assert_eq!(provider.name(), "mock");
    }

    #[test]
    fn test_selection_prefers_ollama_when_url_present() {
        let config = ProviderSelectionConfig {
            provider: None,
            ollama_url: Some("http://localhost:11434".to_string()),
            deepseek_api_key: Some("deepseek-key".to_string()),
            anthropic_api_key: Some("ant-key".to_string()),
        };
        let provider = select_provider(&config);
        assert_eq!(provider.name(), "ollama");
    }

    #[test]
    fn test_gateway_records_spend_and_blocks_on_cap_exceeded() {
        let provider = MockProvider::new();
        let mut gateway = GovernedLlmGateway::new(provider);
        let agent_id = Uuid::new_v4();
        gateway.configure_agent_budget(
            agent_id,
            AgentFuelBudgetConfig {
                period_id: BudgetPeriodId::new("2026-03"),
                cap_units: 30,
            },
        );
        gateway.set_model_cost(
            "mock-1",
            ModelCost {
                cost_per_1k_input: 1_000,
                cost_per_1k_output: 1_000,
            },
        );

        let mut context = AgentRuntimeContext {
            agent_id,
            capabilities: capabilities(&["llm.query"]),
            fuel_remaining: 1_000,
        };

        let first = gateway.query(&mut context, "short", 10, "mock-1");
        assert!(first.is_ok());

        let second = gateway.query(&mut context, "this will exceed cap", 64, "mock-1");
        assert!(matches!(
            second,
            Err(AgentError::FuelViolation {
                violation: FuelViolation::OverMonthlyCap,
                ..
            })
        ));

        let report = gateway
            .fuel_audit_report(agent_id)
            .expect("fuel report should exist");
        assert!(report.spent_units > report.cap_units);

        let has_spend_event = gateway.audit_trail().events().iter().any(|event| {
            event
                .payload
                .get("event_kind")
                .and_then(|value| value.as_str())
                == Some("fuel.spend_recorded")
        });
        assert!(has_spend_event);

        let raw_prompt_leaked = gateway
            .audit_trail()
            .events()
            .iter()
            .any(|event| event.payload.to_string().contains("this will exceed cap"));
        assert!(!raw_prompt_leaked);
    }
}
