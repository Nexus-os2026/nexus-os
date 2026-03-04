use crate::providers::{
    ClaudeProvider, DeepSeekProvider, LlmProvider, LlmResponse, MockProvider, OllamaProvider,
};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::safety_supervisor::{OperatingMode, SafetyAction, SafetySupervisor};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
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
    safety_supervisor: SafetySupervisor,
}

impl<P: LlmProvider> GovernedLlmGateway<P> {
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            audit_trail: AuditTrail::new(),
            oracle_events: Vec::new(),
            safety_supervisor: SafetySupervisor::default(),
        }
    }

    pub fn safety_mode(&self, agent_id: Uuid) -> OperatingMode {
        self.safety_supervisor.mode_for_agent(agent_id)
    }

    pub fn query(
        &mut self,
        agent: &mut AgentRuntimeContext,
        prompt: &str,
        max_tokens: u32,
        model: &str,
    ) -> Result<LlmResponse, AgentError> {
        let audit_len_before = self.audit_trail.events().len();

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

        let started = Instant::now();
        let response = self.provider.query(prompt, max_tokens, model)?;
        let latency_ms = started.elapsed().as_millis() as u64;

        let actual_tokens = u64::from(response.token_count);
        if agent.fuel_remaining < actual_tokens {
            return Err(AgentError::FuelExhausted);
        }
        agent.fuel_remaining -= actual_tokens;

        let cost = f64::from(response.token_count) * self.provider.cost_per_token();
        let prompt_hash = sha256_hex(prompt.as_bytes());
        let response_hash = sha256_hex(response.output_text.as_bytes());
        let timestamp = current_unix_timestamp();

        let payload = json!({
            "event_kind": "OracleEvent",
            "prompt_hash": prompt_hash,
            "response_hash": response_hash,
            "model": response.model_name,
            "tokens": response.token_count,
            "cost": cost,
            "latency_ms": latency_ms,
            "provider_name": self.provider.name(),
            "timestamp": timestamp
        });
        let _ = self
            .audit_trail
            .append_event(agent.agent_id, EventType::LlmCall, payload);

        let audit_len_after = self.audit_trail.events().len();
        let audit_events_added = audit_len_after.saturating_sub(audit_len_before);
        let token_denominator = f64::from(response.token_count.max(1));
        let governance_overhead_pct = (audit_events_added as f64 / token_denominator) * 100.0;

        let safety_action = self.safety_supervisor.observe_llm_response(
            agent.agent_id,
            latency_ms,
            governance_overhead_pct,
            &mut self.audit_trail,
        );
        if let SafetyAction::Halted { reason, report_id } = safety_action {
            return Err(AgentError::SupervisorError(format!(
                "safety supervisor halted llm call for agent '{}': {} (report_id={})",
                agent.agent_id, reason, report_id
            )));
        }

        self.oracle_events.push(OracleEvent {
            agent_id: agent.agent_id,
            prompt_hash,
            response_hash,
            model_name: response.model_name.clone(),
            response_text: response.output_text.clone(),
            token_count: response.token_count,
            cost,
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
        select_provider, AgentRuntimeContext, GovernedLlmGateway, ProviderSelectionConfig,
    };
    use crate::providers::{LlmProvider, LlmResponse, MockProvider};
    use nexus_kernel::errors::AgentError;
    use nexus_kernel::safety_supervisor::OperatingMode;
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
            assert!(event.cost > 0.0);
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
    fn test_safety_kpi_events_are_emitted() {
        let provider = MockProvider::new();
        let mut gateway = GovernedLlmGateway::new(provider);
        let mut context = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities: capabilities(&["llm.query"]),
            fuel_remaining: 1_000,
        };

        let query_result = gateway.query(&mut context, "safety kpi probe", 20, "mock-1");
        assert!(query_result.is_ok());

        let has_kpi_event = gateway.audit_trail().events().iter().any(|event| {
            event
                .payload
                .get("event_kind")
                .and_then(|value| value.as_str())
                == Some("safety.kpi_checked")
        });
        assert!(has_kpi_event);
    }

    #[test]
    fn test_safety_halts_after_three_consecutive_critical_overhead_violations() {
        let provider = MockProvider::new();
        let mut gateway = GovernedLlmGateway::new(provider);
        let agent_id = Uuid::new_v4();
        let mut context = AgentRuntimeContext {
            agent_id,
            capabilities: capabilities(&["llm.query"]),
            fuel_remaining: 1_000,
        };

        let first = gateway.query(&mut context, "tiny", 1, "mock-1");
        let second = gateway.query(&mut context, "tiny", 1, "mock-1");
        let third = gateway.query(&mut context, "tiny", 1, "mock-1");

        assert!(first.is_ok());
        assert!(second.is_ok());
        assert!(matches!(third, Err(AgentError::SupervisorError(_))));
        assert!(matches!(
            gateway.safety_mode(agent_id),
            OperatingMode::Halted(_)
        ));
    }
}
