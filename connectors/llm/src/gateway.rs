use crate::providers::{LlmProvider, LlmResponse};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
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
    pub timestamp: u64,
}

#[derive(Debug)]
pub struct GovernedLlmGateway<P: LlmProvider> {
    provider: P,
    audit_trail: AuditTrail,
    oracle_events: Vec<OracleEvent>,
}

impl<P: LlmProvider> GovernedLlmGateway<P> {
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            audit_trail: AuditTrail::new(),
            oracle_events: Vec::new(),
        }
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
        if agent.fuel_remaining < estimated_tokens {
            return Err(AgentError::FuelExhausted);
        }

        let response = self.provider.query(prompt, max_tokens, model)?;
        let actual_tokens = u64::from(response.token_count);
        if agent.fuel_remaining < actual_tokens {
            return Err(AgentError::FuelExhausted);
        }
        agent.fuel_remaining -= actual_tokens;

        let prompt_hash = sha256_hex(prompt.as_bytes());
        let response_hash = sha256_hex(response.output_text.as_bytes());
        let cost = (actual_tokens as f64) * self.provider.cost_per_token();
        let timestamp = current_unix_timestamp();

        let payload = json!({
            "event_kind": "OracleEvent",
            "prompt_hash": prompt_hash,
            "response_hash": response_hash,
            "token_count": response.token_count,
            "cost": cost,
            "model_name": response.model_name,
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
    use super::{AgentRuntimeContext, GovernedLlmGateway};
    use crate::providers::ClaudeProvider;
    use nexus_kernel::errors::AgentError;
    use std::collections::HashSet;
    use uuid::Uuid;

    fn capabilities(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn test_governed_query_deducts_fuel() {
        let provider = ClaudeProvider::new("key");
        let mut gateway = GovernedLlmGateway::new(provider);
        let mut context = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities: capabilities(&["llm.query"]),
            fuel_remaining: 1_000,
        };

        let result = gateway.query(&mut context, "What is zero trust?", 50, "claude-sonnet-4-5");
        assert!(result.is_ok());
        assert_eq!(context.fuel_remaining, 950);
    }

    #[test]
    fn test_capability_denied() {
        let provider = ClaudeProvider::new("key");
        let mut gateway = GovernedLlmGateway::new(provider);
        let mut context = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities: capabilities(&["web.search"]),
            fuel_remaining: 1_000,
        };

        let result = gateway.query(&mut context, "Hello", 50, "claude-sonnet-4-5");
        assert_eq!(
            result,
            Err(AgentError::CapabilityDenied("llm.query".to_string()))
        );
    }

    #[test]
    fn test_fuel_exhausted_blocks_query() {
        let provider = ClaudeProvider::new("key");
        let mut gateway = GovernedLlmGateway::new(provider);
        let mut context = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities: capabilities(&["llm.query"]),
            fuel_remaining: 10,
        };

        let result = gateway.query(&mut context, "Large request", 500, "claude-sonnet-4-5");
        assert_eq!(result, Err(AgentError::FuelExhausted));
    }

    #[test]
    fn test_response_cached_as_oracle() {
        let provider = ClaudeProvider::new("key");
        let mut gateway = GovernedLlmGateway::new(provider);
        let agent_id = Uuid::new_v4();
        let mut context = AgentRuntimeContext {
            agent_id,
            capabilities: capabilities(&["llm.query"]),
            fuel_remaining: 1_000,
        };

        let result = gateway.query(&mut context, "Return audit-safe output", 25, "claude-sonnet-4-5");
        assert!(result.is_ok());
        assert_eq!(gateway.oracle_events().len(), 1);

        let mut found = false;
        for event in gateway.audit_trail().events() {
            let event_kind = event.payload.get("event_kind").and_then(|value| value.as_str());
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
}
