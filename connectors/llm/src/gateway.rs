use crate::providers::{
    ClaudeProvider, CohereProvider, DeepSeekProvider, FireworksProvider, GeminiProvider,
    GroqProvider, LlmProvider, LlmResponse, MistralProvider, MockProvider, NvidiaProvider,
    OllamaProvider, OpenAiProvider, OpenRouterProvider, PerplexityProvider, TogetherProvider,
};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::firewall::{
    ContentClassification, ContentOrigin, EgressGovernor, FirewallAction, InputFilter,
    OutputFilter, SemanticBoundary,
};
use nexus_kernel::fuel_hardening::{
    AgentFuelLedger, BudgetPeriodId, BurnAnomalyDetector, FuelAuditReport, FuelToTokenModel,
    ModelCost,
};
use nexus_kernel::redaction::{RedactionEngine, RedactionMetrics, RedactionPolicy};
use nexus_kernel::safety_supervisor::{OperatingMode, SafetyAction, SafetySupervisor};
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
    pub openai_api_key: Option<String>,
    pub gemini_api_key: Option<String>,
    pub groq_api_key: Option<String>,
    pub mistral_api_key: Option<String>,
    pub together_api_key: Option<String>,
    pub fireworks_api_key: Option<String>,
    pub perplexity_api_key: Option<String>,
    pub cohere_api_key: Option<String>,
    pub openrouter_api_key: Option<String>,
    pub nvidia_api_key: Option<String>,
}

impl ProviderSelectionConfig {
    pub fn from_env() -> Self {
        Self {
            provider: env::var("LLM_PROVIDER").ok(),
            ollama_url: env::var("OLLAMA_URL").ok(),
            deepseek_api_key: env::var("DEEPSEEK_API_KEY").ok(),
            anthropic_api_key: env::var("ANTHROPIC_API_KEY").ok(),
            openai_api_key: env::var("OPENAI_API_KEY").ok(),
            gemini_api_key: env::var("GEMINI_API_KEY").ok(),
            groq_api_key: env::var("GROQ_API_KEY").ok(),
            mistral_api_key: env::var("MISTRAL_API_KEY").ok(),
            together_api_key: env::var("TOGETHER_API_KEY").ok(),
            fireworks_api_key: env::var("FIREWORKS_API_KEY").ok(),
            perplexity_api_key: env::var("PERPLEXITY_API_KEY").ok(),
            cohere_api_key: env::var("COHERE_API_KEY").ok(),
            openrouter_api_key: env::var("OPENROUTER_API_KEY").ok(),
            nvidia_api_key: env::var("NVIDIA_NIM_API_KEY").ok(),
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

    if has_key(&config.deepseek_api_key) {
        return Box::new(DeepSeekProvider::new(config.deepseek_api_key.clone()));
    }

    if has_key(&config.groq_api_key) {
        return Box::new(GroqProvider::new(config.groq_api_key.clone()));
    }

    if has_key(&config.mistral_api_key) {
        return Box::new(MistralProvider::new(config.mistral_api_key.clone()));
    }

    if has_key(&config.together_api_key) {
        return Box::new(TogetherProvider::new(config.together_api_key.clone()));
    }

    if has_key(&config.fireworks_api_key) {
        return Box::new(FireworksProvider::new(config.fireworks_api_key.clone()));
    }

    if has_key(&config.perplexity_api_key) {
        return Box::new(PerplexityProvider::new(config.perplexity_api_key.clone()));
    }

    if has_key(&config.cohere_api_key) {
        return Box::new(CohereProvider::new(config.cohere_api_key.clone()));
    }

    if has_key(&config.openrouter_api_key) {
        return Box::new(OpenRouterProvider::new(config.openrouter_api_key.clone()));
    }

    if has_key(&config.nvidia_api_key) {
        return Box::new(NvidiaProvider::new(config.nvidia_api_key.clone()));
    }

    if has_key(&config.openai_api_key) {
        return Box::new(OpenAiProvider::new(config.openai_api_key.clone()));
    }

    if has_key(&config.gemini_api_key) {
        return Box::new(GeminiProvider::new(config.gemini_api_key.clone()));
    }

    #[cfg(feature = "real-claude")]
    if has_key(&config.anthropic_api_key) {
        return Box::new(ClaudeProvider::new(config.anthropic_api_key.clone()));
    }

    // Auto-detect Ollama on default port before falling back to Mock
    let ollama_default = OllamaProvider::from_env();
    if ollama_default.health_check().is_ok() {
        eprintln!("[nexus-llm] auto-detected Ollama at localhost:11434, using as default provider");
        return Box::new(ollama_default);
    }

    Box::new(MockProvider::new())
}

fn has_key(key: &Option<String>) -> bool {
    key.as_deref()
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false)
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
        "groq" => Box::new(GroqProvider::new(config.groq_api_key.clone())),
        "mistral" => Box::new(MistralProvider::new(config.mistral_api_key.clone())),
        "together" => Box::new(TogetherProvider::new(config.together_api_key.clone())),
        "fireworks" => Box::new(FireworksProvider::new(config.fireworks_api_key.clone())),
        "perplexity" => Box::new(PerplexityProvider::new(config.perplexity_api_key.clone())),
        "cohere" => Box::new(CohereProvider::new(config.cohere_api_key.clone())),
        "openrouter" => Box::new(OpenRouterProvider::new(config.openrouter_api_key.clone())),
        "nvidia" | "nvidia-nim" | "nim" => {
            Box::new(NvidiaProvider::new(config.nvidia_api_key.clone()))
        }
        "openai" => Box::new(OpenAiProvider::new(config.openai_api_key.clone())),
        "gemini" | "google" => Box::new(GeminiProvider::new(config.gemini_api_key.clone())),
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
    semantic_boundary: SemanticBoundary,
    input_filter: InputFilter,
    egress_governor: EgressGovernor,
    fuel_model: FuelToTokenModel,
    default_period_id: BudgetPeriodId,
    fuel_ledgers: HashMap<Uuid, AgentFuelLedger>,
    safety_supervisor: SafetySupervisor,
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
            semantic_boundary: SemanticBoundary::new(),
            input_filter: InputFilter::new(),
            egress_governor: EgressGovernor::new(),
            fuel_model: FuelToTokenModel::with_defaults(),
            default_period_id: BudgetPeriodId::new("period.default"),
            fuel_ledgers: HashMap::new(),
            safety_supervisor: SafetySupervisor::default(),
        }
    }

    /// Register an agent's allowed egress endpoints (from manifest).
    pub fn register_agent_egress(&mut self, agent_id: Uuid, allowed_endpoints: Vec<String>) {
        self.egress_governor
            .register_agent(agent_id, allowed_endpoints);
    }

    /// Register an agent's allowed egress endpoints with a custom rate limit.
    pub fn register_agent_egress_with_limit(
        &mut self,
        agent_id: Uuid,
        allowed_endpoints: Vec<String>,
        rate_limit_per_min: u32,
    ) {
        self.egress_governor.register_agent_with_limit(
            agent_id,
            allowed_endpoints,
            rate_limit_per_min,
        );
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
        self.query_with_origin(agent, prompt, max_tokens, model, ContentOrigin::UserPrompt)
    }

    /// Query with an explicit [`ContentOrigin`] so the semantic boundary
    /// filter can classify external data appropriately.
    pub fn query_with_origin(
        &mut self,
        agent: &mut AgentRuntimeContext,
        prompt: &str,
        max_tokens: u32,
        model: &str,
        origin: ContentOrigin,
    ) -> Result<LlmResponse, AgentError> {
        let audit_len_before = self.audit_trail.events().len();

        if !agent.capabilities.contains("llm.query") {
            return Err(AgentError::CapabilityDenied("llm.query".to_string()));
        }

        let estimated_tokens = u64::from(max_tokens);
        if agent.fuel_remaining < estimated_tokens {
            return Err(AgentError::FuelExhausted);
        }

        // ── Semantic boundary (before redaction + input firewall) ───────
        let (boundary_prompt, classification) = self
            .semantic_boundary
            .wrap_for_prompt(prompt, origin.clone());

        if classification == ContentClassification::Suspicious {
            self.audit_trail.append_event(
                agent.agent_id,
                EventType::UserAction,
                json!({
                    "event_kind": "firewall.semantic_boundary",
                    "classification": "Suspicious",
                    "origin": format!("{origin}"),
                    "action": "wrapped_with_warning",
                    "prompt_length": prompt.len(),
                }),
            )?;
        } else if classification == ContentClassification::Mixed {
            self.audit_trail.append_event(
                agent.agent_id,
                EventType::UserAction,
                json!({
                    "event_kind": "firewall.semantic_boundary",
                    "classification": "Mixed",
                    "origin": format!("{origin}"),
                    "action": "wrapped",
                    "prompt_length": prompt.len(),
                }),
            )?;
        }

        let prompt_for_redaction = match classification {
            // User prompts pass through unwrapped (classify returns Instruction,
            // sanitize_data returns text unchanged).
            ContentClassification::Instruction => prompt.to_string(),
            // All external data is wrapped with semantic delimiters.
            _ => boundary_prompt,
        };

        let mut redaction_result = self.redaction_engine.process_prompt(
            "llm.query",
            "strict",
            vec![agent.agent_id.to_string(), model.to_string()],
            &prompt_for_redaction,
        );
        self.audit_trail.append_event(
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
        )?;
        self.audit_trail.append_event(
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
        )?;

        // ── Input firewall (after redaction, before provider call) ──────
        match self.input_filter.check(
            agent.agent_id,
            redaction_result.outbound_prompt.as_str(),
            &mut self.audit_trail,
        ) {
            FirewallAction::Block { reason } => {
                return Err(AgentError::CapabilityDenied(format!(
                    "prompt firewall blocked: {reason}"
                )));
            }
            FirewallAction::Redacted { redacted_text, .. } => {
                // Replace outbound prompt with further-redacted version
                // and recalculate the hash to match the actual sent content.
                redaction_result.outbound_prompt_hash_hex = sha256_hex(redacted_text.as_bytes());
                redaction_result.outbound_prompt = redacted_text;
            }
            FirewallAction::Allow => {}
        }

        // ── Egress check (before provider call) ────────────────────────
        if self.egress_governor.has_policy(agent.agent_id) {
            let provider_endpoint = self.provider.endpoint_url();
            if let nexus_kernel::firewall::EgressDecision::Deny { reason } = self
                .egress_governor
                .check_egress(agent.agent_id, &provider_endpoint, &mut self.audit_trail)
            {
                return Err(AgentError::CapabilityDenied(format!(
                    "egress blocked: {reason}"
                )));
            }
        }

        // ── Pre-flight fuel check (before provider call) ─────────────
        if agent.fuel_remaining < estimated_tokens {
            return Err(AgentError::FuelExhausted);
        }

        // Pre-deduct estimated fuel BEFORE provider call (fail-fast on budget)
        agent.fuel_remaining -= estimated_tokens;

        let started = Instant::now();
        let mut response =
            match self
                .provider
                .query(redaction_result.outbound_prompt.as_str(), max_tokens, model)
            {
                Ok(r) => r,
                Err(e) => {
                    // Refund pre-deducted fuel on provider error
                    agent.fuel_remaining += estimated_tokens;
                    let _ = self.audit_trail.append_event(
                        agent.agent_id,
                        EventType::Error,
                        json!({
                            "event_kind": "llm.provider_error",
                            "model": model,
                            "provider": self.provider.name(),
                            "error": e.to_string(),
                        }),
                    );
                    return Err(e);
                }
            };
        let latency_ms = started.elapsed().as_millis() as u64;

        // Adjust fuel: refund estimate, charge actual
        let actual_tokens = u64::from(response.token_count);
        agent.fuel_remaining += estimated_tokens; // refund estimate
        if agent.fuel_remaining < actual_tokens {
            return Err(AgentError::FuelExhausted);
        }
        agent.fuel_remaining -= actual_tokens; // charge actual

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
            self.audit_trail.append_event(
                agent.agent_id,
                EventType::UserAction,
                json!({
                    "event_kind": "fuel.model_cost_fallback",
                    "agent_id": agent.agent_id,
                    "model": model,
                    "fallback_cost_per_1k_input": fallback_cost.cost_per_1k_input,
                    "fallback_cost_per_1k_output": fallback_cost.cost_per_1k_output,
                }),
            )?;
        }

        self.audit_trail.append_event(
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
        )?;

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
                self.audit_trail.append_event(
                    agent.agent_id,
                    EventType::UserAction,
                    json!({
                        "event_kind": "autonomy.level_changed",
                        "agent_id": agent.agent_id,
                        "previous_level": "unknown",
                        "new_level": 0,
                        "reason": reason,
                    }),
                )?;
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
            .append_event(agent.agent_id, EventType::LlmCall, payload)?;

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

        // ── Output firewall (after response, before returning to agent) ──
        match OutputFilter::check(
            agent.agent_id,
            &response.output_text,
            None,
            &mut self.audit_trail,
        ) {
            FirewallAction::Block { reason } => {
                return Err(AgentError::CapabilityDenied(format!(
                    "output firewall blocked: {reason}"
                )));
            }
            FirewallAction::Redacted { redacted_text, .. } => {
                response.output_text = redacted_text;
            }
            FirewallAction::Allow => {}
        }

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
