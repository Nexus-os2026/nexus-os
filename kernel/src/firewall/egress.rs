//! Egress governor: per-agent URL allowlisting and rate limiting for all
//! outbound network calls.
//!
//! **Default deny** — unless an endpoint is explicitly listed in the agent's
//! `allowed_endpoints` manifest field, the call is blocked.
//!
//! Every egress decision is audited. The governor is fail-closed: any internal
//! error results in a block, never a silent pass.

use crate::audit::{AuditTrail, EventType};
use crate::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

/// Default rate limit: 60 requests per minute per endpoint.
pub const DEFAULT_RATE_LIMIT_PER_MIN: u32 = 60;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Outcome of an egress check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EgressDecision {
    /// Request is allowed — proceed.
    Allow,
    /// Request is blocked with reason.
    Deny { reason: String },
}

/// Per-agent egress policy.
#[derive(Debug, Clone)]
struct AgentEgressPolicy {
    /// URL prefixes that are allowed (e.g. `["https://api.example.com"]`).
    allowed_endpoints: Vec<String>,
    /// Max requests per minute per endpoint.
    rate_limit_per_min: u32,
}

/// Tracks request timestamps for rate limiting.
#[derive(Debug, Clone, Default)]
struct RateState {
    /// Map from endpoint prefix → list of request timestamps (unix secs).
    windows: HashMap<String, Vec<u64>>,
}

// ---------------------------------------------------------------------------
// EgressGovernor
// ---------------------------------------------------------------------------

/// Per-agent egress governor. Enforces URL allowlists and rate limits.
#[derive(Debug, Clone, Default)]
pub struct EgressGovernor {
    policies: HashMap<Uuid, AgentEgressPolicy>,
    rate_state: HashMap<Uuid, RateState>,
}

impl EgressGovernor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if a policy is registered for the given agent.
    pub fn has_policy(&self, agent_id: Uuid) -> bool {
        self.policies.contains_key(&agent_id)
    }

    /// Register an agent's egress policy with the default rate limit.
    ///
    /// * `allowed_endpoints` — URL prefixes the agent may call. Empty = deny all.
    pub fn register_agent(&mut self, agent_id: Uuid, allowed_endpoints: Vec<String>) {
        self.register_agent_with_limit(agent_id, allowed_endpoints, 0);
    }

    /// Register an agent's egress policy with a custom rate limit.
    ///
    /// * `allowed_endpoints` — URL prefixes the agent may call. Empty = deny all.
    /// * `rate_limit_per_min` — max requests per minute per endpoint (0 = use default).
    pub fn register_agent_with_limit(
        &mut self,
        agent_id: Uuid,
        allowed_endpoints: Vec<String>,
        rate_limit_per_min: u32,
    ) {
        let limit = if rate_limit_per_min == 0 {
            DEFAULT_RATE_LIMIT_PER_MIN
        } else {
            rate_limit_per_min
        };
        self.policies.insert(
            agent_id,
            AgentEgressPolicy {
                allowed_endpoints,
                rate_limit_per_min: limit,
            },
        );
    }

    /// Check whether `agent_id` is allowed to call `url`.
    ///
    /// Returns `Deny` if:
    /// - No policy registered for the agent (default deny).
    /// - URL doesn't match any allowed endpoint prefix.
    /// - Rate limit exceeded for the matching endpoint.
    ///
    /// On `Allow`, the rate counter is incremented.
    pub fn check_egress(
        &mut self,
        agent_id: Uuid,
        url: &str,
        audit: &mut AuditTrail,
    ) -> EgressDecision {
        let policy = match self.policies.get(&agent_id) {
            Some(p) => p.clone(),
            None => {
                let decision = EgressDecision::Deny {
                    reason: "no egress policy registered for agent (default deny)".to_string(),
                };
                let _ = Self::audit(agent_id, url, &decision, audit);
                return decision;
            }
        };

        // Find the matching allowed endpoint prefix.
        let matched_prefix = policy
            .allowed_endpoints
            .iter()
            .find(|prefix| url.starts_with(prefix.as_str()));

        let prefix = match matched_prefix {
            Some(p) => p.clone(),
            None => {
                let decision = EgressDecision::Deny {
                    reason: format!(
                        "URL '{url}' does not match any allowed endpoint for this agent"
                    ),
                };
                let _ = Self::audit(agent_id, url, &decision, audit);
                return decision;
            }
        };

        // Rate limit check.
        let now = now_secs();
        let window_start = now.saturating_sub(60);
        let rate = self.rate_state.entry(agent_id).or_default();
        let timestamps = rate.windows.entry(prefix).or_default();

        // Purge stale entries.
        timestamps.retain(|&t| t > window_start);

        if timestamps.len() as u32 >= policy.rate_limit_per_min {
            let decision = EgressDecision::Deny {
                reason: format!(
                    "rate limit exceeded: {} requests in last 60s (limit {})",
                    timestamps.len(),
                    policy.rate_limit_per_min
                ),
            };
            let _ = Self::audit(agent_id, url, &decision, audit);
            return decision;
        }

        // Allow — record this request.
        timestamps.push(now);

        let decision = EgressDecision::Allow;
        let _ = Self::audit(agent_id, url, &decision, audit);
        decision
    }

    fn audit(
        agent_id: Uuid,
        url: &str,
        decision: &EgressDecision,
        audit: &mut AuditTrail,
    ) -> Result<(), AgentError> {
        let (action, details) = match decision {
            EgressDecision::Allow => ("allow", json!({})),
            EgressDecision::Deny { reason } => ("deny", json!({ "reason": reason })),
        };
        audit.append_event(
            agent_id,
            EventType::UserAction,
            json!({
                "event_kind": "firewall.egress",
                "url": url,
                "action": action,
                "details": details,
            }),
        )?;
        Ok(())
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditTrail;

    fn id() -> Uuid {
        Uuid::new_v4()
    }

    #[test]
    fn allowed_passes() {
        let mut gov = EgressGovernor::new();
        let mut audit = AuditTrail::new();
        let agent = id();

        gov.register_agent(agent, vec!["https://api.example.com".into()]);

        let result = gov.check_egress(agent, "https://api.example.com/v1/data", &mut audit);
        assert_eq!(result, EgressDecision::Allow);
    }

    #[test]
    fn disallowed_blocked() {
        let mut gov = EgressGovernor::new();
        let mut audit = AuditTrail::new();
        let agent = id();

        gov.register_agent(agent, vec!["https://api.example.com".into()]);

        let result = gov.check_egress(agent, "https://evil.com/exfiltrate", &mut audit);
        assert!(matches!(result, EgressDecision::Deny { .. }));
    }

    #[test]
    fn default_deny_works() {
        let mut gov = EgressGovernor::new();
        let mut audit = AuditTrail::new();
        let agent = id();
        // No policy registered.

        let result = gov.check_egress(agent, "https://anything.com", &mut audit);
        assert!(matches!(result, EgressDecision::Deny { .. }));
    }

    #[test]
    fn empty_allowlist_denies_all() {
        let mut gov = EgressGovernor::new();
        let mut audit = AuditTrail::new();
        let agent = id();

        gov.register_agent(agent, vec![]);

        let result = gov.check_egress(agent, "https://api.example.com/v1/data", &mut audit);
        assert!(matches!(result, EgressDecision::Deny { .. }));
    }

    #[test]
    fn rate_limit_enforced() {
        let mut gov = EgressGovernor::new();
        let mut audit = AuditTrail::new();
        let agent = id();

        // Set very low rate limit: 3 per minute.
        gov.register_agent_with_limit(agent, vec!["https://api.example.com".into()], 3);

        for _ in 0..3 {
            let r = gov.check_egress(agent, "https://api.example.com/call", &mut audit);
            assert_eq!(r, EgressDecision::Allow);
        }

        // 4th call should be rate-limited.
        let r = gov.check_egress(agent, "https://api.example.com/call", &mut audit);
        assert!(matches!(r, EgressDecision::Deny { .. }));
        if let EgressDecision::Deny { reason } = &r {
            assert!(reason.contains("rate limit"));
        }
    }

    #[test]
    fn audited() {
        let mut gov = EgressGovernor::new();
        let mut audit = AuditTrail::new();
        let agent = id();

        gov.register_agent(agent, vec!["https://ok.com".into()]);

        // 1. Allow
        gov.check_egress(agent, "https://ok.com/a", &mut audit);
        // 2. Deny (wrong URL)
        gov.check_egress(agent, "https://bad.com/b", &mut audit);

        let events = audit.events();
        assert_eq!(events.len(), 2, "expected 2 audit events");

        let kinds: Vec<&str> = events
            .iter()
            .filter_map(|e| e.payload.get("event_kind").and_then(|v| v.as_str()))
            .collect();
        assert!(kinds.iter().all(|k| *k == "firewall.egress"));

        let actions: Vec<&str> = events
            .iter()
            .filter_map(|e| e.payload.get("action").and_then(|v| v.as_str()))
            .collect();
        assert_eq!(actions[0], "allow");
        assert_eq!(actions[1], "deny");
    }

    #[test]
    fn multiple_allowed_prefixes() {
        let mut gov = EgressGovernor::new();
        let mut audit = AuditTrail::new();
        let agent = id();

        gov.register_agent(
            agent,
            vec![
                "https://api.example.com".into(),
                "https://cdn.example.com".into(),
            ],
        );

        assert_eq!(
            gov.check_egress(agent, "https://api.example.com/v1", &mut audit),
            EgressDecision::Allow
        );
        assert_eq!(
            gov.check_egress(agent, "https://cdn.example.com/img.png", &mut audit),
            EgressDecision::Allow
        );
        assert!(matches!(
            gov.check_egress(agent, "https://other.com", &mut audit),
            EgressDecision::Deny { .. }
        ));
    }
}
