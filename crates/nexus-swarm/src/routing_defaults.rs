//! Default routing policies for the three adapters we ship in Phase 1
//! (Artisan, Herald, Broker) plus the Director policy and reference entries
//! for the three NYI stubs (Scout, Watchdog, Prospector). Override at
//! runtime by placing `~/.nexus/swarm_routing.toml` with a `[policies]`
//! table.
//!
//! Phase 1.5a update: preference lists are now explicit per agent, with
//! dedicated primary and fallback Ollama model_ids rather than a single
//! generic "ollama" slot. Herald omits cloud routes entirely because its
//! TaskProfile is `privacy=Sensitive`; the router's PrivacyClass guard
//! would reject any cloud candidate anyway, but dropping them from the
//! preference list makes the policy self-documenting.

use crate::routing::{RouteCandidate, RoutingPolicy};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RoutingOverrideFile {
    #[serde(default)]
    pub policies: HashMap<String, Vec<RouteCandidate>>,
}

fn cand(provider: &str, model: &str) -> RouteCandidate {
    RouteCandidate {
        provider_id: provider.into(),
        model_id: model.into(),
        est_cost_cents: 0,
    }
}

/// Built-in defaults. Preference order is local-first with a named cloud
/// fallback for everything except Herald (which is privacy-sensitive).
pub fn builtin_policies() -> Vec<RoutingPolicy> {
    vec![
        // Director — plans DAGs. Prefers a mid-size local model; falls back
        // to a smaller local model, then to codex-cli for heavier reasoning.
        RoutingPolicy {
            agent_id: "director".into(),
            preference_order: vec![
                cand("ollama", "qwen3.5:9b"),
                cand("ollama", "gemma4:e4b"),
                cand("codex-cli", "gpt-5.4"),
            ],
        },
        // Artisan — code generation. Uses coder-tuned models locally.
        RoutingPolicy {
            agent_id: "artisan".into(),
            preference_order: vec![
                cand("ollama", "qwen2.5-coder:14b"),
                cand("ollama", "qwen2.5-coder:7b"),
                cand("codex-cli", "gpt-5.4"),
            ],
        },
        // Herald — social posting. Sensitive privacy class → no cloud.
        RoutingPolicy {
            agent_id: "herald".into(),
            preference_order: vec![cand("ollama", "gemma4:e2b"), cand("ollama", "qwen3.5:4b")],
        },
        // Broker — coordinates other agents. Prefers fast local models.
        RoutingPolicy {
            agent_id: "broker".into(),
            preference_order: vec![
                cand("ollama", "qwen3.5:9b"),
                cand("ollama", "glm4:9b"),
                cand("codex-cli", "gpt-5.4"),
            ],
        },
        // NYI descriptor stubs — included so an override TOML can name
        // them without being rejected; runtime selection will never pick
        // them because the registry filters stubs out of `select_for_task`.
        RoutingPolicy {
            agent_id: "scout".into(),
            preference_order: vec![],
        },
        RoutingPolicy {
            agent_id: "watchdog".into(),
            preference_order: vec![],
        },
        RoutingPolicy {
            agent_id: "prospector".into(),
            preference_order: vec![],
        },
    ]
}

/// Load defaults, then overlay `~/.nexus/swarm_routing.toml` if present.
pub fn load_policies() -> Vec<RoutingPolicy> {
    let mut map: HashMap<String, RoutingPolicy> = builtin_policies()
        .into_iter()
        .map(|p| (p.agent_id.clone(), p))
        .collect();

    if let Some(path) = override_path() {
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(cfg) = toml::from_str::<RoutingOverrideFile>(&text) {
                for (agent, pref) in cfg.policies {
                    map.insert(
                        agent.clone(),
                        RoutingPolicy {
                            agent_id: agent,
                            preference_order: pref,
                        },
                    );
                }
            } else {
                tracing::warn!(
                    target: "nexus_swarm::routing",
                    "ignoring malformed routing override at {}",
                    path.display()
                );
            }
        }
    }

    map.into_values().collect()
}

fn override_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".nexus").join("swarm_routing.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_policies_cover_three_real_agents_plus_director_plus_stubs() {
        let ps = builtin_policies();
        let ids: Vec<&str> = ps.iter().map(|p| p.agent_id.as_str()).collect();
        for id in [
            "artisan",
            "herald",
            "broker",
            "director",
            "scout",
            "watchdog",
            "prospector",
        ] {
            assert!(ids.contains(&id), "missing default for `{id}`");
        }
    }

    #[test]
    fn director_has_primary_and_cloud_fallback() {
        let ps = builtin_policies();
        let director = ps.iter().find(|p| p.agent_id == "director").unwrap();
        assert_eq!(director.preference_order.len(), 3);
        assert_eq!(director.preference_order[0].provider_id, "ollama");
        assert_eq!(director.preference_order[0].model_id, "qwen3.5:9b");
        assert_eq!(director.preference_order[1].provider_id, "ollama");
        assert_eq!(director.preference_order[2].provider_id, "codex-cli");
    }

    #[test]
    fn artisan_uses_coder_models() {
        let ps = builtin_policies();
        let artisan = ps.iter().find(|p| p.agent_id == "artisan").unwrap();
        assert_eq!(artisan.preference_order[0].model_id, "qwen2.5-coder:14b");
        assert_eq!(artisan.preference_order[1].model_id, "qwen2.5-coder:7b");
    }

    #[test]
    fn herald_has_no_cloud_routes() {
        let ps = builtin_policies();
        let herald = ps.iter().find(|p| p.agent_id == "herald").unwrap();
        assert_eq!(herald.preference_order.len(), 2);
        assert!(herald
            .preference_order
            .iter()
            .all(|c| c.provider_id == "ollama"));
    }

    #[test]
    fn broker_has_two_local_and_one_cloud() {
        let ps = builtin_policies();
        let broker = ps.iter().find(|p| p.agent_id == "broker").unwrap();
        assert_eq!(broker.preference_order.len(), 3);
        assert_eq!(broker.preference_order[0].model_id, "qwen3.5:9b");
        assert_eq!(broker.preference_order[1].model_id, "glm4:9b");
        assert_eq!(broker.preference_order[2].provider_id, "codex-cli");
    }

    #[test]
    fn override_file_is_applied_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg_path = tmp.path().join("swarm_routing.toml");
        let toml_src = r#"
            [policies]
            artisan = [
              { provider_id = "openai", model_id = "gpt-4o", est_cost_cents = 5 },
            ]
        "#;
        std::fs::write(&cfg_path, toml_src).unwrap();

        // We can't redirect `dirs::home_dir()` cleanly in a unit test, so
        // we test the parse path directly.
        let cfg: RoutingOverrideFile = toml::from_str(toml_src).unwrap();
        assert_eq!(cfg.policies["artisan"].len(), 1);
        assert_eq!(cfg.policies["artisan"][0].provider_id, "openai");
    }
}
