//! Router — resolves `(agent_id, TaskProfile, Budget) -> (provider, model)`.
//!
//! Resolution is a 5-rule elimination chain applied to each candidate in the
//! agent's preference order:
//!
//! 1. Privacy — `StrictLocal` / `Sensitive` tasks reject non-local providers.
//! 2. Health — skip providers whose last probe was not `Ok`.
//! 3. Budget — skip candidates whose estimated cost would breach the budget.
//! 4. Spend cap — skip candidates routed via the Anthropic provider when the
//!    Haiku spend ledger is exhausted. (Reported by the provider via
//!    `health_check` notes; the router reads `ProviderHealth::notes` for the
//!    `spend cap exceeded` marker.)
//! 5. Preference — first survivor wins.
//!
//! Any candidate eliminated contributes a reason string to [`RouteDenied`] so
//! the UI can explain the failure.

use crate::budget::Budget;
use crate::events::{ProviderHealth, ProviderHealthStatus};
use crate::profile::{PrivacyClass, TaskProfile};
use crate::provider::Provider;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteCandidate {
    pub provider_id: String,
    pub model_id: String,
    /// Rough upper-bound on per-invoke cost in cents. Used only for the
    /// budget elimination rule — the router does not bill the budget here.
    #[serde(default)]
    pub est_cost_cents: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutingPolicy {
    pub agent_id: String,
    pub preference_order: Vec<RouteCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteDenied {
    pub agent_id: String,
    pub reasons: Vec<String>,
}

impl std::fmt::Display for RouteDenied {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "no route for `{}`: {}",
            self.agent_id,
            self.reasons.join("; ")
        )
    }
}

pub struct Router {
    pub policies: HashMap<String, RoutingPolicy>,
    pub providers: HashMap<String, Arc<dyn Provider>>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            policies: HashMap::new(),
            providers: HashMap::new(),
        }
    }

    pub fn register_provider(&mut self, p: Arc<dyn Provider>) {
        self.providers.insert(p.id().to_string(), p);
    }

    pub fn set_policy(&mut self, policy: RoutingPolicy) {
        self.policies.insert(policy.agent_id.clone(), policy);
    }

    /// Apply the 5-rule chain. `health` maps provider-id → most recent probe.
    /// Providers absent from the map are treated as `Unhealthy`.
    pub fn resolve(
        &self,
        agent_id: &str,
        profile: &TaskProfile,
        budget: &Budget,
        health: &HashMap<String, ProviderHealth>,
    ) -> Result<RouteCandidate, RouteDenied> {
        let policy = match self.policies.get(agent_id) {
            Some(p) => p,
            None => {
                return Err(RouteDenied {
                    agent_id: agent_id.into(),
                    reasons: vec![format!("no routing policy registered for `{agent_id}`")],
                });
            }
        };

        let mut reasons: Vec<String> = Vec::new();

        for cand in &policy.preference_order {
            let Some(provider) = self.providers.get(&cand.provider_id) else {
                reasons.push(format!(
                    "{}/{}: provider not registered",
                    cand.provider_id, cand.model_id
                ));
                continue;
            };
            let provider_privacy = provider.capabilities().privacy_class;

            // Rule 1: privacy — task privacy must be satisfied by provider.
            if !profile.privacy.satisfied_by(provider_privacy) {
                reasons.push(format!(
                    "{}/{}: privacy {:?} not satisfied by provider privacy {:?}",
                    cand.provider_id, cand.model_id, profile.privacy, provider_privacy
                ));
                continue;
            }

            // Rule 2: health.
            let hs = health.get(&cand.provider_id);
            let status = hs
                .map(|h| h.status.clone())
                .unwrap_or(ProviderHealthStatus::Unhealthy);
            if status != ProviderHealthStatus::Ok {
                reasons.push(format!(
                    "{}/{}: provider health is {:?}",
                    cand.provider_id, cand.model_id, status
                ));
                continue;
            }

            // Rule 4 (checked before rule 3 since it's a hard invariant):
            // Anthropic spend cap is surfaced as a note on the provider's
            // health probe. If the note contains `spend cap exceeded` we skip.
            if let Some(h) = hs {
                if h.notes.to_ascii_lowercase().contains("spend cap exceeded") {
                    reasons.push(format!(
                        "{}/{}: {}",
                        cand.provider_id, cand.model_id, h.notes
                    ));
                    continue;
                }
            }

            // Rule 3: budget.
            if cand.est_cost_cents > budget.cost_cents {
                reasons.push(format!(
                    "{}/{}: est cost {}¢ exceeds remaining budget {}¢",
                    cand.provider_id, cand.model_id, cand.est_cost_cents, budget.cost_cents
                ));
                continue;
            }

            // Rule 5: first survivor wins.
            return Ok(cand.clone());
        }

        if reasons.is_empty() {
            reasons.push("preference list is empty".into());
        }
        Err(RouteDenied {
            agent_id: agent_id.into(),
            reasons,
        })
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

// Note: privacy helpers live on `PrivacyClass` itself.
#[allow(dead_code)]
fn _privacy_type_ref(_p: PrivacyClass) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{
        InvokeRequest, InvokeResponse, ModelDescriptor, ProviderCapabilities, ProviderError,
    };
    use async_trait::async_trait;

    struct MockProvider {
        id: String,
        privacy: PrivacyClass,
    }

    #[async_trait]
    impl Provider for MockProvider {
        fn id(&self) -> &str {
            &self.id
        }
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                models: vec![ModelDescriptor {
                    id: "m".into(),
                    param_count_b: None,
                    tier: crate::profile::ReasoningTier::Medium,
                    context_window: 8192,
                }],
                supports_tool_use: false,
                supports_streaming: false,
                max_context: 8192,
                cost_class: crate::profile::CostClass::Free,
                privacy_class: self.privacy,
            }
        }
        async fn health_check(&self) -> ProviderHealth {
            unreachable!("router tests do not probe")
        }
        async fn invoke(&self, _r: InvokeRequest) -> Result<InvokeResponse, ProviderError> {
            unreachable!("router tests do not invoke")
        }
    }

    fn healthy(id: &str) -> ProviderHealth {
        ProviderHealth {
            provider_id: id.into(),
            status: ProviderHealthStatus::Ok,
            latency_ms: Some(1),
            models: vec!["m".into()],
            notes: String::new(),
            checked_at_secs: 0,
        }
    }
    fn unhealthy(id: &str) -> ProviderHealth {
        ProviderHealth {
            provider_id: id.into(),
            status: ProviderHealthStatus::Unhealthy,
            latency_ms: None,
            models: vec![],
            notes: "down".into(),
            checked_at_secs: 0,
        }
    }

    fn make_router(pairs: &[(&str, PrivacyClass)]) -> Router {
        let mut r = Router::new();
        for (id, pc) in pairs {
            r.register_provider(Arc::new(MockProvider {
                id: (*id).to_string(),
                privacy: *pc,
            }));
        }
        r
    }

    #[test]
    fn strict_local_rejects_cloud_even_first() {
        let mut r = make_router(&[
            ("openai", PrivacyClass::Public),
            ("ollama", PrivacyClass::StrictLocal),
        ]);
        r.set_policy(RoutingPolicy {
            agent_id: "a".into(),
            preference_order: vec![
                RouteCandidate {
                    provider_id: "openai".into(),
                    model_id: "gpt-4o".into(),
                    est_cost_cents: 5,
                },
                RouteCandidate {
                    provider_id: "ollama".into(),
                    model_id: "gemma4:e2b".into(),
                    est_cost_cents: 0,
                },
            ],
        });
        let mut h = HashMap::new();
        h.insert("openai".into(), healthy("openai"));
        h.insert("ollama".into(), healthy("ollama"));
        let profile = TaskProfile {
            privacy: PrivacyClass::StrictLocal,
            ..TaskProfile::local_light()
        };
        let chosen = r
            .resolve("a", &profile, &Budget::unlimited_for_tests(), &h)
            .unwrap();
        assert_eq!(chosen.provider_id, "ollama");
    }

    #[test]
    fn sensitive_rejects_cloud() {
        let mut r = make_router(&[("openai", PrivacyClass::Public)]);
        r.set_policy(RoutingPolicy {
            agent_id: "a".into(),
            preference_order: vec![RouteCandidate {
                provider_id: "openai".into(),
                model_id: "gpt-4o".into(),
                est_cost_cents: 1,
            }],
        });
        let mut h = HashMap::new();
        h.insert("openai".into(), healthy("openai"));
        let profile = TaskProfile {
            privacy: PrivacyClass::Sensitive,
            ..TaskProfile::local_light()
        };
        let err = r
            .resolve("a", &profile, &Budget::unlimited_for_tests(), &h)
            .unwrap_err();
        assert!(err.reasons.iter().any(|r| r.contains("privacy")));
    }

    #[test]
    fn unhealthy_provider_is_skipped() {
        let mut r = make_router(&[
            ("openrouter", PrivacyClass::Public),
            ("openai", PrivacyClass::Public),
        ]);
        r.set_policy(RoutingPolicy {
            agent_id: "a".into(),
            preference_order: vec![
                RouteCandidate {
                    provider_id: "openrouter".into(),
                    model_id: "x".into(),
                    est_cost_cents: 1,
                },
                RouteCandidate {
                    provider_id: "openai".into(),
                    model_id: "gpt-4o".into(),
                    est_cost_cents: 1,
                },
            ],
        });
        let mut h = HashMap::new();
        h.insert("openrouter".into(), unhealthy("openrouter"));
        h.insert("openai".into(), healthy("openai"));
        let profile = TaskProfile {
            privacy: PrivacyClass::Public,
            ..TaskProfile::local_light()
        };
        let chosen = r
            .resolve("a", &profile, &Budget::unlimited_for_tests(), &h)
            .unwrap();
        assert_eq!(chosen.provider_id, "openai");
    }

    #[test]
    fn budget_exhausted_falls_through() {
        let mut r = make_router(&[
            ("openai", PrivacyClass::Public),
            ("openrouter", PrivacyClass::Public),
        ]);
        r.set_policy(RoutingPolicy {
            agent_id: "a".into(),
            preference_order: vec![
                RouteCandidate {
                    provider_id: "openai".into(),
                    model_id: "o1".into(),
                    est_cost_cents: 1000,
                },
                RouteCandidate {
                    provider_id: "openrouter".into(),
                    model_id: "cheap".into(),
                    est_cost_cents: 1,
                },
            ],
        });
        let mut h = HashMap::new();
        h.insert("openai".into(), healthy("openai"));
        h.insert("openrouter".into(), healthy("openrouter"));
        let profile = TaskProfile {
            privacy: PrivacyClass::Public,
            ..TaskProfile::local_light()
        };
        let budget = Budget::new(1_000_000, 10, 1_000_000);
        let chosen = r.resolve("a", &profile, &budget, &h).unwrap();
        assert_eq!(chosen.provider_id, "openrouter");
    }

    #[test]
    fn route_denied_includes_all_reasons() {
        let mut r = make_router(&[("openai", PrivacyClass::Public)]);
        r.set_policy(RoutingPolicy {
            agent_id: "a".into(),
            preference_order: vec![RouteCandidate {
                provider_id: "openai".into(),
                model_id: "gpt-4o".into(),
                est_cost_cents: 1,
            }],
        });
        let mut h = HashMap::new();
        h.insert("openai".into(), unhealthy("openai"));
        let profile = TaskProfile {
            privacy: PrivacyClass::Public,
            ..TaskProfile::local_light()
        };
        let err = r
            .resolve("a", &profile, &Budget::unlimited_for_tests(), &h)
            .unwrap_err();
        assert!(!err.reasons.is_empty());
    }

    #[test]
    fn anthropic_spend_cap_blocks_route() {
        let mut r = make_router(&[("anthropic", PrivacyClass::Public)]);
        r.set_policy(RoutingPolicy {
            agent_id: "a".into(),
            preference_order: vec![RouteCandidate {
                provider_id: "anthropic".into(),
                model_id: "claude-haiku-4-5-20251001".into(),
                est_cost_cents: 1,
            }],
        });
        let mut h = HashMap::new();
        let mut ph = healthy("anthropic");
        ph.notes = "spend cap exceeded: $2.00 / $2.00".into();
        h.insert("anthropic".into(), ph);
        let profile = TaskProfile {
            privacy: PrivacyClass::Public,
            ..TaskProfile::local_light()
        };
        let err = r
            .resolve("a", &profile, &Budget::unlimited_for_tests(), &h)
            .unwrap_err();
        assert!(err.reasons.iter().any(|r| r.contains("spend")));
    }

    #[test]
    fn missing_policy_reports_reason() {
        let r = Router::new();
        let err = r
            .resolve(
                "unknown",
                &TaskProfile::local_light(),
                &Budget::unlimited_for_tests(),
                &HashMap::new(),
            )
            .unwrap_err();
        assert!(err.reasons.iter().any(|r| r.contains("no routing policy")));
    }

    #[test]
    fn three_deep_preference_cascades() {
        let mut r = make_router(&[
            ("p1", PrivacyClass::Public),
            ("p2", PrivacyClass::Public),
            ("p3", PrivacyClass::Public),
        ]);
        r.set_policy(RoutingPolicy {
            agent_id: "a".into(),
            preference_order: vec![
                RouteCandidate {
                    provider_id: "p1".into(),
                    model_id: "x".into(),
                    est_cost_cents: 1,
                },
                RouteCandidate {
                    provider_id: "p2".into(),
                    model_id: "x".into(),
                    est_cost_cents: 1,
                },
                RouteCandidate {
                    provider_id: "p3".into(),
                    model_id: "x".into(),
                    est_cost_cents: 1,
                },
            ],
        });
        let mut h = HashMap::new();
        h.insert("p1".into(), unhealthy("p1"));
        h.insert("p2".into(), unhealthy("p2"));
        h.insert("p3".into(), healthy("p3"));
        let profile = TaskProfile {
            privacy: PrivacyClass::Public,
            ..TaskProfile::local_light()
        };
        let chosen = r
            .resolve("a", &profile, &Budget::unlimited_for_tests(), &h)
            .unwrap();
        assert_eq!(chosen.provider_id, "p3");
    }
}
