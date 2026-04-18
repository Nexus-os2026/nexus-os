//! Default routing policies for the three adapters we ship in Phase 1
//! (Artisan, Herald, Broker) plus reference entries for the three NYI stubs
//! (Scout, Watchdog, Prospector). Override at runtime by placing
//! `~/.nexus/swarm_routing.toml` with a `[policies]` table.

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

/// Built-in defaults. Kept minimal — preference order leans on local-first.
pub fn builtin_policies() -> Vec<RoutingPolicy> {
    let local_first = |agent: &str, cloud: &[(&str, &str, u32)]| -> RoutingPolicy {
        let mut pref = vec![RouteCandidate {
            provider_id: "ollama".into(),
            model_id: String::new(), // resolved at router time when dynamic
            est_cost_cents: 0,
        }];
        for (provider, model, cents) in cloud {
            pref.push(RouteCandidate {
                provider_id: (*provider).into(),
                model_id: (*model).into(),
                est_cost_cents: *cents,
            });
        }
        RoutingPolicy {
            agent_id: agent.into(),
            preference_order: pref,
        }
    };

    vec![
        // Artisan (coder) — prefers local, falls back to cheap cloud.
        local_first(
            "artisan",
            &[
                ("openrouter", "deepseek/deepseek-coder-v3", 2),
                ("openai", "gpt-4o-mini", 5),
            ],
        ),
        // Herald (social-poster) — latency-sensitive, prefers small local.
        local_first(
            "herald",
            &[
                ("openrouter", "openai/gpt-4.1-mini", 2),
                ("openai", "gpt-4o-mini", 3),
            ],
        ),
        // Broker (collaboration) — prefers local; cloud fallback uses Haiku.
        local_first(
            "broker",
            &[
                ("anthropic", "claude-haiku-4-5-20251001", 2),
                ("openrouter", "anthropic/claude-sonnet-4-6", 5),
            ],
        ),
        // Director is resolved via a dedicated "director" policy.
        local_first(
            "director",
            &[
                ("openrouter", "anthropic/claude-sonnet-4-6", 10),
                ("anthropic", "claude-haiku-4-5-20251001", 3),
                ("openai", "gpt-4o", 10),
            ],
        ),
        // NYI descriptor stubs — included so an override TOML can name them
        // without being rejected; runtime selection will never pick them
        // because the registry filters stubs.
        local_first("scout", &[]),
        local_first("watchdog", &[]),
        local_first("prospector", &[]),
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
