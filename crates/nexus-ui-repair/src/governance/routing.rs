//! Provider routing table. See v1.1 §4.
//!
//! The autonomous scout is permitted only `codex_cli`, `ollama`, and a
//! capped `anthropic_api` allowance for vision ambiguity escalation.
//! `claude_cli` and `claude_ai_credits` are explicitly forbidden in
//! autonomous mode (Max plan ToS, account-ban risk).

pub const OLLAMA_MODEL_E2B: &str = "gemma4:e2b";
pub const OLLAMA_MODEL_E4B: &str = "gemma4:e4b";
pub const ANTHROPIC_MODEL_HAIKU_4_5: &str = "haiku-4.5";

/// A provider that the scout may call autonomously.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    /// Codex CLI / GPT-5.4 via the user's ChatGPT Plus subscription.
    CodexCli,
    /// Local Ollama model (e.g. `gemma4:e2b`).
    Ollama { model: String },
    /// Anthropic API direct, capped to a small dollar budget for
    /// vision ambiguity escalation only.
    AnthropicApi { model: String },
}

/// Providers explicitly forbidden in autonomous mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForbiddenProvider {
    /// `claude_cli` — would consume the Max plan, ToS violation.
    ClaudeCli,
    /// `claude_ai_credits` — account ban risk.
    ClaudeAiCredits,
}

/// The signed routing table loaded at session start.
#[derive(Debug, Clone)]
pub struct RoutingTable {
    allowed: Vec<Provider>,
    forbidden: Vec<ForbiddenProvider>,
    anthropic_api_cap_usd: f64,
}

impl RoutingTable {
    /// The default v1.1 §4 routing table.
    pub fn default_v1_1() -> Self {
        Self {
            allowed: vec![
                Provider::CodexCli,
                Provider::Ollama {
                    model: OLLAMA_MODEL_E2B.to_string(),
                },
                Provider::Ollama {
                    model: OLLAMA_MODEL_E4B.to_string(),
                },
                Provider::AnthropicApi {
                    model: ANTHROPIC_MODEL_HAIKU_4_5.to_string(),
                },
            ],
            forbidden: vec![
                ForbiddenProvider::ClaudeCli,
                ForbiddenProvider::ClaudeAiCredits,
            ],
            anthropic_api_cap_usd: 2.0,
        }
    }

    /// Check whether a given provider is permitted under this table.
    /// Returns `Error::ProviderForbidden` for any provider not in the
    /// allowed set.
    pub fn check_provider(&self, p: &Provider) -> crate::Result<()> {
        if self.allowed.contains(p) {
            Ok(())
        } else {
            Err(crate::Error::ProviderForbidden(format!("{:?}", p)))
        }
    }

    /// Allowed providers.
    pub fn allowed(&self) -> &[Provider] {
        &self.allowed
    }

    /// Forbidden providers.
    pub fn forbidden(&self) -> &[ForbiddenProvider] {
        &self.forbidden
    }

    /// Anthropic API spend cap, in USD.
    pub fn anthropic_api_cap_usd(&self) -> f64 {
        self.anthropic_api_cap_usd
    }

    /// Construct an empty routing table — used by tests that want to
    /// trigger the defense-in-depth panic in `vision_judge`. Not for
    /// production use.
    pub fn empty_for_test() -> Self {
        Self {
            allowed: vec![],
            forbidden: vec![],
            anthropic_api_cap_usd: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_table_matches_v1_1_spec() {
        let t = RoutingTable::default_v1_1();
        assert!(t.allowed().contains(&Provider::CodexCli));
        assert!(!t.forbidden().is_empty());
        assert!(t.forbidden().contains(&ForbiddenProvider::ClaudeCli));
        assert!((t.anthropic_api_cap_usd() - 2.0).abs() < f64::EPSILON);
    }
}
