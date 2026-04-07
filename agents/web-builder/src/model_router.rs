//! Multi-model router for the Nexus Builder.
//!
//! Routes builder tasks to the cheapest capable model:
//! - **Local (Ollama)**: Free, used for planning and section edits
//! - **Anthropic Haiku/Sonnet**: Paid, Haiku for cheap tasks, Sonnet for full generation
//! - **OpenAI**: Paid backup when Anthropic budget is low

use nexus_connectors_llm::providers::{LlmProvider, LlmResponse, OllamaProvider};
use serde::{Deserialize, Serialize};

// ─── Task Types ─────────────────────────────────────────────────────────────

/// What kind of work the builder needs to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuilderTask {
    /// Plan generation — structured JSON output, cheap
    PlanGeneration,
    /// Template classification — short JSON, cheap
    TemplateClassification,
    /// Section-level edit — moderate HTML, medium cost
    SectionEdit,
    /// Full page generation — 500+ line HTML, expensive
    FullGeneration,
    /// Full page iteration — send entire HTML + edit, expensive
    FullIteration,
}

// ─── Provider Types ─────────────────────────────────────────────────────────

/// Which provider backend to use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderType {
    Ollama,
    Anthropic,
    OpenAI,
    /// Codex CLI — uses ChatGPT Plus/Pro subscription, $0 per build.
    CodexCli,
    /// Claude Code CLI — uses Claude subscription, $0 per build.
    ClaudeCode,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ollama => write!(f, "ollama"),
            Self::Anthropic => write!(f, "anthropic"),
            Self::OpenAI => write!(f, "openai"),
            Self::CodexCli => write!(f, "codex-cli"),
            Self::ClaudeCode => write!(f, "claude-code"),
        }
    }
}

/// A concrete model selection with cost estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSelection {
    pub provider: ProviderType,
    pub model_id: String,
    pub display_name: String,
    pub estimated_cost: f64,
    pub is_local: bool,
}

// ─── Budget Status ──────────────────────────────────────────────────────────

/// Current budget state for routing decisions.
#[derive(Debug, Clone)]
pub struct RoutingBudget {
    pub anthropic_remaining: f64,
    pub openai_remaining: f64,
    pub ollama_available: bool,
    /// Codex CLI (GPT-5.4) detected and authenticated — $0, fast builds.
    pub codex_cli_available: bool,
    /// Claude Code CLI detected and authenticated — $0, streaming builds.
    pub claude_code_available: bool,
}

impl RoutingBudget {
    /// Build from the budget tracker and local provider detection.
    pub fn from_budget_tracker() -> Self {
        let tracker = crate::budget::BudgetTracker::new();
        let status = tracker.get_budget_status();
        let ollama_available = check_ollama_available();
        let codex_cli_available = check_codex_cli_available();
        let claude_code_available = check_claude_code_available();
        Self {
            anthropic_remaining: status.anthropic_remaining,
            openai_remaining: status.openai_remaining,
            ollama_available,
            codex_cli_available,
            claude_code_available,
        }
    }
}

// ─── Model Constants ────────────────────────────────────────────────────────

pub const OLLAMA_SMALL: &str = "gemma4:e2b";
pub const OLLAMA_LARGE: &str = "gemma4:e4b";
pub const HAIKU: &str = "claude-haiku-4-5-20251001";
pub const SONNET: &str = "claude-sonnet-4-6";
pub const GPT4O_MINI: &str = "gpt-4o-mini";
pub const GPT4O: &str = "gpt-4o";
/// GPT-5.4 via Codex CLI — subscription-covered, $0 per build.
pub const CODEX_GPT5: &str = "gpt-5.4";

/// Minimum Anthropic budget before we consider switching to OpenAI.
const ANTHROPIC_LOW_BUDGET: f64 = 0.50;

// ─── Ollama Availability ────────────────────────────────────────────────────

/// Check if Ollama is running locally (fast TCP probe, 200ms timeout).
pub fn check_ollama_available() -> bool {
    let ollama = OllamaProvider::from_env();
    ollama.health_check().unwrap_or(false)
}

/// Check if Codex CLI is installed and authenticated.
pub fn check_codex_cli_available() -> bool {
    let status = nexus_connectors_llm::providers::codex_cli::detect_codex_cli();
    status.installed && status.authenticated
}

/// Check if Claude Code CLI is installed and authenticated.
pub fn check_claude_code_available() -> bool {
    let status = nexus_connectors_llm::providers::claude_code::detect_claude_code();
    status.installed && status.authenticated
}

// ─── Model Selection ────────────────────────────────────────────────────────

/// Select the best model for a given task based on budget and availability.
pub fn select_model(task: &BuilderTask, budget: &RoutingBudget) -> ModelSelection {
    match task {
        BuilderTask::PlanGeneration => select_plan_model(budget),
        BuilderTask::TemplateClassification => select_classification_model(budget),
        BuilderTask::SectionEdit => select_section_edit_model(budget),
        BuilderTask::FullGeneration => select_full_gen_model(budget),
        BuilderTask::FullIteration => select_full_iter_model(budget),
    }
}

fn select_plan_model(budget: &RoutingBudget) -> ModelSelection {
    if budget.ollama_available {
        return ModelSelection {
            provider: ProviderType::Ollama,
            model_id: OLLAMA_SMALL.to_string(),
            display_name: OLLAMA_SMALL.to_string(),
            estimated_cost: 0.0,
            is_local: true,
        };
    }
    if budget.anthropic_remaining > ANTHROPIC_LOW_BUDGET {
        return ModelSelection {
            provider: ProviderType::Anthropic,
            model_id: HAIKU.to_string(),
            display_name: "Haiku 4.5".to_string(),
            estimated_cost: 0.003,
            is_local: false,
        };
    }
    ModelSelection {
        provider: ProviderType::OpenAI,
        model_id: GPT4O_MINI.to_string(),
        display_name: "GPT-4o Mini".to_string(),
        estimated_cost: 0.002,
        is_local: false,
    }
}

fn select_classification_model(budget: &RoutingBudget) -> ModelSelection {
    // Classification is a small JSON task — same priority as planning
    select_plan_model(budget)
}

fn select_section_edit_model(budget: &RoutingBudget) -> ModelSelection {
    if budget.ollama_available {
        return ModelSelection {
            provider: ProviderType::Ollama,
            model_id: OLLAMA_LARGE.to_string(),
            display_name: OLLAMA_LARGE.to_string(),
            estimated_cost: 0.0,
            is_local: true,
        };
    }
    if budget.anthropic_remaining > ANTHROPIC_LOW_BUDGET {
        return ModelSelection {
            provider: ProviderType::Anthropic,
            model_id: SONNET.to_string(),
            display_name: "Sonnet 4.6".to_string(),
            estimated_cost: 0.03,
            is_local: false,
        };
    }
    ModelSelection {
        provider: ProviderType::OpenAI,
        model_id: GPT4O.to_string(),
        display_name: "GPT-4o".to_string(),
        estimated_cost: 0.03,
        is_local: false,
    }
}

fn select_full_gen_model(budget: &RoutingBudget) -> ModelSelection {
    // Priority 1: Codex CLI (GPT-5.4) — fastest + $0 (subscription-covered)
    if budget.codex_cli_available {
        return ModelSelection {
            provider: ProviderType::CodexCli,
            model_id: CODEX_GPT5.to_string(),
            display_name: "GPT-5.4 (Codex CLI)".to_string(),
            estimated_cost: 0.0,
            is_local: false,
        };
    }
    // Priority 2: Anthropic API (fast + paid)
    if budget.anthropic_remaining > ANTHROPIC_LOW_BUDGET {
        return ModelSelection {
            provider: ProviderType::Anthropic,
            model_id: SONNET.to_string(),
            display_name: "Sonnet 4.6".to_string(),
            estimated_cost: 0.20,
            is_local: false,
        };
    }
    // Priority 3: Claude Code CLI (slow but $0)
    if budget.claude_code_available {
        return ModelSelection {
            provider: ProviderType::ClaudeCode,
            model_id: SONNET.to_string(),
            display_name: "Sonnet 4.6 (CLI)".to_string(),
            estimated_cost: 0.0,
            is_local: false,
        };
    }
    // Priority 4: OpenAI API
    if budget.openai_remaining > 0.50 {
        return ModelSelection {
            provider: ProviderType::OpenAI,
            model_id: GPT4O.to_string(),
            display_name: "GPT-4o".to_string(),
            estimated_cost: 0.10,
            is_local: false,
        };
    }
    // Last resort: local (quality will be lower)
    ModelSelection {
        provider: ProviderType::Ollama,
        model_id: OLLAMA_LARGE.to_string(),
        display_name: format!("{} (local)", OLLAMA_LARGE),
        estimated_cost: 0.0,
        is_local: true,
    }
}

fn select_full_iter_model(budget: &RoutingBudget) -> ModelSelection {
    // Same priority as full generation
    select_full_gen_model(budget)
}

// ─── Failover Selection ─────────────────────────────────────────────────────

/// Get the next-best model when the primary fails.
pub fn failover_model(
    task: &BuilderTask,
    failed: &ProviderType,
    budget: &RoutingBudget,
) -> Option<ModelSelection> {
    match (task, failed) {
        // Cheap tasks: Ollama failed → Haiku → GPT-4o-mini
        (
            BuilderTask::PlanGeneration | BuilderTask::TemplateClassification,
            ProviderType::Ollama,
        ) => {
            if budget.anthropic_remaining > ANTHROPIC_LOW_BUDGET {
                Some(ModelSelection {
                    provider: ProviderType::Anthropic,
                    model_id: HAIKU.to_string(),
                    display_name: "Haiku 4.5".to_string(),
                    estimated_cost: 0.003,
                    is_local: false,
                })
            } else if budget.openai_remaining > 0.10 {
                Some(ModelSelection {
                    provider: ProviderType::OpenAI,
                    model_id: GPT4O_MINI.to_string(),
                    display_name: "GPT-4o Mini".to_string(),
                    estimated_cost: 0.002,
                    is_local: false,
                })
            } else {
                None
            }
        }
        (
            BuilderTask::PlanGeneration | BuilderTask::TemplateClassification,
            ProviderType::Anthropic,
        ) => {
            if budget.openai_remaining > 0.10 {
                Some(ModelSelection {
                    provider: ProviderType::OpenAI,
                    model_id: GPT4O_MINI.to_string(),
                    display_name: "GPT-4o Mini".to_string(),
                    estimated_cost: 0.002,
                    is_local: false,
                })
            } else {
                None
            }
        }
        // Section edits: Ollama failed → Sonnet → GPT-4o
        (BuilderTask::SectionEdit, ProviderType::Ollama) => {
            if budget.anthropic_remaining > ANTHROPIC_LOW_BUDGET {
                Some(ModelSelection {
                    provider: ProviderType::Anthropic,
                    model_id: SONNET.to_string(),
                    display_name: "Sonnet 4.6".to_string(),
                    estimated_cost: 0.03,
                    is_local: false,
                })
            } else if budget.openai_remaining > 0.50 {
                Some(ModelSelection {
                    provider: ProviderType::OpenAI,
                    model_id: GPT4O.to_string(),
                    display_name: "GPT-4o".to_string(),
                    estimated_cost: 0.03,
                    is_local: false,
                })
            } else {
                None
            }
        }
        // Full gen/iter: CodexCli failed → Anthropic API → Claude CLI → OpenAI → Ollama
        (BuilderTask::FullGeneration | BuilderTask::FullIteration, ProviderType::CodexCli) => {
            if budget.anthropic_remaining > ANTHROPIC_LOW_BUDGET {
                Some(ModelSelection {
                    provider: ProviderType::Anthropic,
                    model_id: SONNET.to_string(),
                    display_name: "Sonnet 4.6".to_string(),
                    estimated_cost: 0.20,
                    is_local: false,
                })
            } else if budget.claude_code_available {
                Some(ModelSelection {
                    provider: ProviderType::ClaudeCode,
                    model_id: SONNET.to_string(),
                    display_name: "Sonnet 4.6 (CLI)".to_string(),
                    estimated_cost: 0.0,
                    is_local: false,
                })
            } else if budget.openai_remaining > 0.50 {
                Some(ModelSelection {
                    provider: ProviderType::OpenAI,
                    model_id: GPT4O.to_string(),
                    display_name: "GPT-4o".to_string(),
                    estimated_cost: 0.10,
                    is_local: false,
                })
            } else {
                None
            }
        }
        // Anthropic failed → Claude CLI → OpenAI → Ollama
        (BuilderTask::FullGeneration | BuilderTask::FullIteration, ProviderType::Anthropic) => {
            if budget.claude_code_available {
                Some(ModelSelection {
                    provider: ProviderType::ClaudeCode,
                    model_id: SONNET.to_string(),
                    display_name: "Sonnet 4.6 (CLI)".to_string(),
                    estimated_cost: 0.0,
                    is_local: false,
                })
            } else if budget.openai_remaining > 0.50 {
                Some(ModelSelection {
                    provider: ProviderType::OpenAI,
                    model_id: GPT4O.to_string(),
                    display_name: "GPT-4o".to_string(),
                    estimated_cost: 0.10,
                    is_local: false,
                })
            } else if budget.ollama_available {
                Some(ModelSelection {
                    provider: ProviderType::Ollama,
                    model_id: OLLAMA_LARGE.to_string(),
                    display_name: format!("{} (local)", OLLAMA_LARGE),
                    estimated_cost: 0.0,
                    is_local: true,
                })
            } else {
                None
            }
        }
        (BuilderTask::FullGeneration | BuilderTask::FullIteration, ProviderType::OpenAI) => {
            if budget.ollama_available {
                Some(ModelSelection {
                    provider: ProviderType::Ollama,
                    model_id: OLLAMA_LARGE.to_string(),
                    display_name: format!("{} (local)", OLLAMA_LARGE),
                    estimated_cost: 0.0,
                    is_local: true,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

// ─── Normalized Response ────────────────────────────────────────────────────

/// Unified response from any provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedLlmResponse {
    pub content: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub model_id: String,
    pub provider: String,
    pub latency_ms: u64,
    pub cost_usd: f64,
}

/// Normalize an LlmResponse from any provider.
pub fn normalize_response(
    resp: &LlmResponse,
    provider: &str,
    latency_ms: u64,
) -> NormalizedLlmResponse {
    let cost = crate::build_stream::calculate_cost(
        &resp.model_name,
        resp.input_tokens.unwrap_or(0) as usize,
        resp.token_count as usize,
    );
    NormalizedLlmResponse {
        content: resp.output_text.clone(),
        input_tokens: resp.input_tokens.unwrap_or(0) as usize,
        output_tokens: resp.token_count as usize,
        model_id: resp.model_name.clone(),
        provider: provider.to_string(),
        latency_ms,
        cost_usd: cost,
    }
}

// ─── Query with Failover ────────────────────────────────────────────────────

/// Query a provider, returning a normalized response.
///
/// Retries on 429/529 errors (up to 3 attempts with exponential backoff).
/// On other errors, returns the error for the caller to handle failover.
pub fn query_provider(
    provider: &dyn LlmProvider,
    prompt: &str,
    max_tokens: u32,
    model: &str,
) -> Result<NormalizedLlmResponse, String> {
    let start = std::time::Instant::now();
    let provider_name = provider.name().to_string();

    // Retry loop for rate-limit / overload errors
    let mut last_err = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            let delay = std::time::Duration::from_millis(2000 * (1 << (attempt - 1)));
            std::thread::sleep(delay);
            eprintln!(
                "[model-router] Retry attempt {} for {} ({})",
                attempt + 1,
                model,
                provider_name
            );
        }

        match provider.query(prompt, max_tokens, model) {
            Ok(resp) => {
                let latency = start.elapsed().as_millis() as u64;
                return Ok(normalize_response(&resp, &provider_name, latency));
            }
            Err(e) => {
                let msg = e.to_string();
                // Retry on rate limit (429) or overload (529)
                if msg.contains("429") || msg.contains("529") || msg.contains("rate") {
                    last_err = msg;
                    continue;
                }
                // Non-retryable error — return immediately for failover
                return Err(msg);
            }
        }
    }

    Err(format!("exhausted retries: {last_err}"))
}

/// Query with automatic failover between providers.
///
/// Tries the primary model, then fails over to alternatives based on the
/// task type and budget. Returns the response along with whether failover occurred.
pub fn query_with_failover(
    task: &BuilderTask,
    budget: &RoutingBudget,
    prompt: &str,
    max_tokens: u32,
    providers: &ProviderSet,
) -> Result<(NormalizedLlmResponse, ModelSelection), String> {
    let primary = select_model(task, budget);
    eprintln!(
        "[model-router] Selected {} ({}) for {:?}",
        primary.display_name, primary.provider, task
    );

    let provider = providers.get(&primary.provider);
    match provider {
        Some(p) => match query_provider(p.as_ref(), prompt, max_tokens, &primary.model_id) {
            Ok(resp) => return Ok((resp, primary)),
            Err(e) => {
                eprintln!(
                    "[model-router] {} failed: {}, attempting failover",
                    primary.display_name, e
                );
            }
        },
        None => {
            eprintln!(
                "[model-router] Provider {} not available, attempting failover",
                primary.provider
            );
        }
    }

    // Try failover
    if let Some(fallback) = failover_model(task, &primary.provider, budget) {
        eprintln!(
            "[model-router] Failing over to {} ({})",
            fallback.display_name, fallback.provider
        );
        if let Some(p) = providers.get(&fallback.provider) {
            match query_provider(p.as_ref(), prompt, max_tokens, &fallback.model_id) {
                Ok(resp) => return Ok((resp, fallback)),
                Err(e) => {
                    return Err(format!(
                        "failover to {} also failed: {e}",
                        fallback.display_name
                    ));
                }
            }
        }
    }

    Err(format!(
        "all providers failed for {:?} (primary: {})",
        task, primary.display_name
    ))
}

// ─── Provider Set ───────────────────────────────────────────────────────────

/// A collection of available providers, keyed by type.
pub struct ProviderSet {
    pub ollama: Option<Box<dyn LlmProvider>>,
    pub anthropic: Option<Box<dyn LlmProvider>>,
    pub openai: Option<Box<dyn LlmProvider>>,
    pub codex_cli: Option<Box<dyn LlmProvider>>,
    pub claude_code: Option<Box<dyn LlmProvider>>,
}

impl ProviderSet {
    pub fn get(&self, pt: &ProviderType) -> &Option<Box<dyn LlmProvider>> {
        match pt {
            ProviderType::Ollama => &self.ollama,
            ProviderType::Anthropic => &self.anthropic,
            ProviderType::OpenAI => &self.openai,
            ProviderType::CodexCli => &self.codex_cli,
            ProviderType::ClaudeCode => &self.claude_code,
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn budget_all_available() -> RoutingBudget {
        RoutingBudget {
            anthropic_remaining: 5.0,
            openai_remaining: 10.0,
            ollama_available: true,
            codex_cli_available: false,
            claude_code_available: false,
        }
    }

    fn budget_no_ollama() -> RoutingBudget {
        RoutingBudget {
            anthropic_remaining: 5.0,
            openai_remaining: 10.0,
            ollama_available: false,
            codex_cli_available: false,
            claude_code_available: false,
        }
    }

    fn budget_low_anthropic() -> RoutingBudget {
        RoutingBudget {
            anthropic_remaining: 0.20,
            openai_remaining: 10.0,
            ollama_available: false,
            codex_cli_available: false,
            claude_code_available: false,
        }
    }

    fn budget_all_exhausted() -> RoutingBudget {
        RoutingBudget {
            anthropic_remaining: 0.0,
            openai_remaining: 0.0,
            ollama_available: true,
            codex_cli_available: false,
            claude_code_available: false,
        }
    }

    fn budget_only_openai() -> RoutingBudget {
        RoutingBudget {
            anthropic_remaining: 0.10,
            openai_remaining: 8.0,
            ollama_available: false,
            codex_cli_available: false,
            claude_code_available: false,
        }
    }

    fn budget_with_codex_cli() -> RoutingBudget {
        RoutingBudget {
            anthropic_remaining: 5.0,
            openai_remaining: 10.0,
            ollama_available: true,
            codex_cli_available: true,
            claude_code_available: false,
        }
    }

    fn budget_with_claude_code() -> RoutingBudget {
        RoutingBudget {
            anthropic_remaining: 0.10,
            openai_remaining: 0.10,
            ollama_available: false,
            codex_cli_available: false,
            claude_code_available: true,
        }
    }

    // ── Plan Generation ──

    #[test]
    fn test_plan_prefers_ollama() {
        let sel = select_model(&BuilderTask::PlanGeneration, &budget_all_available());
        assert_eq!(sel.provider, ProviderType::Ollama);
        assert_eq!(sel.estimated_cost, 0.0);
        assert!(sel.is_local);
    }

    #[test]
    fn test_plan_falls_back_to_haiku() {
        let sel = select_model(&BuilderTask::PlanGeneration, &budget_no_ollama());
        assert_eq!(sel.provider, ProviderType::Anthropic);
        assert_eq!(sel.model_id, HAIKU);
    }

    #[test]
    fn test_plan_falls_back_to_openai() {
        let sel = select_model(&BuilderTask::PlanGeneration, &budget_low_anthropic());
        assert_eq!(sel.provider, ProviderType::OpenAI);
        assert_eq!(sel.model_id, GPT4O_MINI);
    }

    // ── Classification ──

    #[test]
    fn test_classification_same_as_plan() {
        let plan = select_model(&BuilderTask::PlanGeneration, &budget_all_available());
        let classify = select_model(
            &BuilderTask::TemplateClassification,
            &budget_all_available(),
        );
        assert_eq!(plan.provider, classify.provider);
        assert_eq!(plan.model_id, classify.model_id);
    }

    // ── Section Edit ──

    #[test]
    fn test_section_prefers_ollama_large() {
        let sel = select_model(&BuilderTask::SectionEdit, &budget_all_available());
        assert_eq!(sel.provider, ProviderType::Ollama);
        assert_eq!(sel.model_id, OLLAMA_LARGE);
        assert!(sel.is_local);
    }

    #[test]
    fn test_section_falls_back_to_sonnet() {
        let sel = select_model(&BuilderTask::SectionEdit, &budget_no_ollama());
        assert_eq!(sel.provider, ProviderType::Anthropic);
        assert_eq!(sel.model_id, SONNET);
    }

    #[test]
    fn test_section_falls_back_to_gpt4o() {
        let sel = select_model(&BuilderTask::SectionEdit, &budget_low_anthropic());
        assert_eq!(sel.provider, ProviderType::OpenAI);
        assert_eq!(sel.model_id, GPT4O);
    }

    // ── Full Generation ──

    #[test]
    fn test_full_gen_prefers_sonnet() {
        let sel = select_model(&BuilderTask::FullGeneration, &budget_all_available());
        assert_eq!(sel.provider, ProviderType::Anthropic);
        assert_eq!(sel.model_id, SONNET);
    }

    #[test]
    fn test_full_gen_uses_gpt4o_when_anthropic_low() {
        let sel = select_model(&BuilderTask::FullGeneration, &budget_only_openai());
        assert_eq!(sel.provider, ProviderType::OpenAI);
        assert_eq!(sel.model_id, GPT4O);
    }

    #[test]
    fn test_full_gen_last_resort_ollama() {
        let sel = select_model(&BuilderTask::FullGeneration, &budget_all_exhausted());
        assert_eq!(sel.provider, ProviderType::Ollama);
        assert!(sel.is_local);
    }

    // ── Failover ──

    #[test]
    fn test_failover_plan_ollama_to_haiku() {
        let f = failover_model(
            &BuilderTask::PlanGeneration,
            &ProviderType::Ollama,
            &budget_no_ollama(),
        );
        assert!(f.is_some());
        let f = f.unwrap();
        assert_eq!(f.provider, ProviderType::Anthropic);
        assert_eq!(f.model_id, HAIKU);
    }

    #[test]
    fn test_failover_plan_anthropic_to_openai() {
        let f = failover_model(
            &BuilderTask::PlanGeneration,
            &ProviderType::Anthropic,
            &budget_low_anthropic(),
        );
        assert!(f.is_some());
        assert_eq!(f.unwrap().provider, ProviderType::OpenAI);
    }

    #[test]
    fn test_failover_section_ollama_to_sonnet() {
        let b = budget_no_ollama();
        let f = failover_model(&BuilderTask::SectionEdit, &ProviderType::Ollama, &b);
        assert!(f.is_some());
        assert_eq!(f.unwrap().model_id, SONNET);
    }

    #[test]
    fn test_failover_fullgen_anthropic_to_openai() {
        let f = failover_model(
            &BuilderTask::FullGeneration,
            &ProviderType::Anthropic,
            &budget_only_openai(),
        );
        assert!(f.is_some());
        assert_eq!(f.unwrap().model_id, GPT4O);
    }

    #[test]
    fn test_no_failover_when_all_exhausted() {
        let b = RoutingBudget {
            anthropic_remaining: 0.0,
            openai_remaining: 0.0,
            ollama_available: false,
            codex_cli_available: false,
            claude_code_available: false,
        };
        let f = failover_model(&BuilderTask::PlanGeneration, &ProviderType::Ollama, &b);
        assert!(f.is_none());
    }

    // ── Full Iteration same as Full Generation ──

    #[test]
    fn test_full_iter_same_as_full_gen() {
        let gen = select_model(&BuilderTask::FullGeneration, &budget_all_available());
        let iter = select_model(&BuilderTask::FullIteration, &budget_all_available());
        assert_eq!(gen.provider, iter.provider);
        assert_eq!(gen.model_id, iter.model_id);
    }

    // ── Codex CLI Routing ──

    #[test]
    fn test_full_gen_prefers_codex_cli_when_available() {
        let sel = select_model(&BuilderTask::FullGeneration, &budget_with_codex_cli());
        assert_eq!(sel.provider, ProviderType::CodexCli);
        assert_eq!(sel.model_id, CODEX_GPT5);
        assert_eq!(sel.estimated_cost, 0.0);
    }

    #[test]
    fn test_full_gen_codex_cli_over_anthropic_api() {
        // Even with Anthropic budget, Codex CLI wins (faster + free)
        let b = budget_with_codex_cli();
        assert!(b.anthropic_remaining > ANTHROPIC_LOW_BUDGET);
        let sel = select_model(&BuilderTask::FullGeneration, &b);
        assert_eq!(sel.provider, ProviderType::CodexCli);
    }

    #[test]
    fn test_full_gen_falls_back_to_claude_code_cli() {
        let sel = select_model(&BuilderTask::FullGeneration, &budget_with_claude_code());
        assert_eq!(sel.provider, ProviderType::ClaudeCode);
        assert_eq!(sel.model_id, SONNET);
        assert_eq!(sel.estimated_cost, 0.0);
    }

    #[test]
    fn test_failover_codex_cli_to_anthropic() {
        let f = failover_model(
            &BuilderTask::FullGeneration,
            &ProviderType::CodexCli,
            &budget_with_codex_cli(),
        );
        assert!(f.is_some());
        let f = f.unwrap();
        assert_eq!(f.provider, ProviderType::Anthropic);
        assert_eq!(f.model_id, SONNET);
    }

    #[test]
    fn test_failover_anthropic_to_claude_code_cli() {
        let b = RoutingBudget {
            anthropic_remaining: 0.10,
            openai_remaining: 0.10,
            ollama_available: false,
            codex_cli_available: false,
            claude_code_available: true,
        };
        let f = failover_model(&BuilderTask::FullGeneration, &ProviderType::Anthropic, &b);
        assert!(f.is_some());
        assert_eq!(f.unwrap().provider, ProviderType::ClaudeCode);
    }

    #[test]
    fn test_plan_still_prefers_ollama_not_codex() {
        // Codex CLI is for full gen, not cheap planning tasks
        let sel = select_model(&BuilderTask::PlanGeneration, &budget_with_codex_cli());
        assert_eq!(sel.provider, ProviderType::Ollama);
    }

    #[test]
    fn test_codex_cost_is_zero() {
        let sel = select_model(&BuilderTask::FullGeneration, &budget_with_codex_cli());
        assert_eq!(sel.estimated_cost, 0.0);
        assert_eq!(sel.display_name, "GPT-5.4 (Codex CLI)");
    }

    #[test]
    fn test_failover_fullgen_anthropic_to_claude_code_to_openai() {
        // Anthropic failed, Claude Code not available → OpenAI
        let f = failover_model(
            &BuilderTask::FullGeneration,
            &ProviderType::Anthropic,
            &budget_only_openai(),
        );
        assert!(f.is_some());
        assert_eq!(f.unwrap().provider, ProviderType::OpenAI);
    }

    // ── Normalized Response ──

    #[test]
    fn test_normalize_ollama_response() {
        let resp = LlmResponse {
            output_text: "test output".to_string(),
            token_count: 100,
            model_name: "gemma4:e2b".to_string(),
            tool_calls: vec![],
            input_tokens: Some(50),
        };
        let norm = normalize_response(&resp, "ollama", 500);
        assert_eq!(norm.cost_usd, 0.0); // local model
        assert_eq!(norm.provider, "ollama");
        assert_eq!(norm.latency_ms, 500);
        assert_eq!(norm.input_tokens, 50);
        assert_eq!(norm.output_tokens, 100);
    }

    #[test]
    fn test_normalize_anthropic_response() {
        let resp = LlmResponse {
            output_text: "test".to_string(),
            token_count: 500,
            model_name: "claude-haiku-4-5-20251001".to_string(),
            tool_calls: vec![],
            input_tokens: Some(200),
        };
        let norm = normalize_response(&resp, "anthropic", 3000);
        assert!(norm.cost_usd > 0.0); // paid model
        assert_eq!(norm.provider, "anthropic");
    }
}
