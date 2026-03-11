use crate::autonomy::AutonomyLevel;
use crate::consent::HitlTier;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PolicyError {
    #[error("policy load error: {0}")]
    LoadError(String),
    #[error("policy parse error in '{file}': {reason}")]
    ParseError { file: String, reason: String },
}

// ---------------------------------------------------------------------------
// Policy data model
// ---------------------------------------------------------------------------

/// The effect a policy produces when it matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyEffect {
    Allow,
    Deny,
}

/// Optional conditions that must hold for the policy to match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PolicyConditions {
    /// Agent must be at least this autonomy level (0-5).
    pub min_autonomy_level: Option<u8>,
    /// Maximum fuel cost the action may incur.
    pub max_fuel_cost: Option<u64>,
    /// Number of human approvers required (maps to HITL tier).
    pub required_approvers: Option<usize>,
    /// Cron-style or named time window (stored but not evaluated yet).
    pub time_window: Option<String>,
}

/// A single governance policy loaded from a TOML file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    pub policy_id: String,
    #[serde(default)]
    pub description: String,
    pub effect: PolicyEffect,
    /// Agent DID pattern — `"*"` matches any principal.
    pub principal: String,
    /// Operation type such as `"tool_call"`, `"terminal_command"`, or `"*"`.
    pub action: String,
    /// Capability key pattern such as `"fs.write"`, `"web.*"`, or `"*"`.
    pub resource: String,
    /// Priority for ordering — lower number = higher priority.
    #[serde(default = "default_priority")]
    pub priority: u32,
    #[serde(default)]
    pub conditions: PolicyConditions,
}

fn default_priority() -> u32 {
    100
}

// ---------------------------------------------------------------------------
// Evaluation context & decision
// ---------------------------------------------------------------------------

/// Runtime context passed into the policy engine for a single evaluation.
#[derive(Debug, Clone)]
pub struct EvaluationContext {
    pub autonomy_level: AutonomyLevel,
    pub fuel_cost: Option<u64>,
}

impl Default for EvaluationContext {
    fn default() -> Self {
        Self {
            autonomy_level: AutonomyLevel::L0,
            fuel_cost: None,
        }
    }
}

/// The outcome of policy evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    Deny { reason: String },
    RequireApproval { tier: HitlTier },
}

// ---------------------------------------------------------------------------
// Pattern matching helpers
// ---------------------------------------------------------------------------

/// Match a value against a pattern that supports `"*"` (match all) and
/// trailing `".*"` wildcards (e.g. `"web.*"` matches `"web.search"`).
fn pattern_matches(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix(".*") {
        return value == prefix || value.starts_with(&format!("{prefix}."));
    }
    pattern == value
}

fn autonomy_level_from_u8(v: u8) -> AutonomyLevel {
    AutonomyLevel::from_numeric(v).unwrap_or(AutonomyLevel::L5)
}

// ---------------------------------------------------------------------------
// Policy engine
// ---------------------------------------------------------------------------

/// Cedar-inspired policy engine.
///
/// Loads TOML policy files from a directory and evaluates them in priority
/// order.  Default-deny: if no policy matches the request, the action is
/// denied.  An explicit `Deny` always overrides any `Allow`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyEngine {
    policies: Vec<Policy>,
    policy_dir: PathBuf,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self {
            policies: Vec::new(),
            policy_dir: PathBuf::from("~/.nexus/policies"),
        }
    }
}

impl PolicyEngine {
    /// Create an engine that will load policies from `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            policies: Vec::new(),
            policy_dir: dir.into(),
        }
    }

    /// Create an engine pre-loaded with the given policies (useful for tests).
    pub fn with_policies(policies: Vec<Policy>) -> Self {
        let mut engine = Self {
            policies,
            policy_dir: PathBuf::from("~/.nexus/policies"),
        };
        engine.sort_policies();
        engine
    }

    /// Load all `*.toml` files from the configured directory.
    pub fn load_policies(&mut self) -> Result<usize, PolicyError> {
        self.policies.clear();

        let dir = &self.policy_dir;
        if !dir.exists() {
            return Ok(0);
        }

        let entries = std::fs::read_dir(dir).map_err(|e| PolicyError::LoadError(e.to_string()))?;

        for entry in entries {
            let entry = entry.map_err(|e| PolicyError::LoadError(e.to_string()))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                self.load_policy_file(&path)?;
            }
        }

        self.sort_policies();
        Ok(self.policies.len())
    }

    /// Load and parse a single TOML policy file.
    fn load_policy_file(&mut self, path: &Path) -> Result<(), PolicyError> {
        let content = std::fs::read_to_string(path).map_err(|e| PolicyError::ParseError {
            file: path.display().to_string(),
            reason: e.to_string(),
        })?;

        let policy: Policy = toml::from_str(&content).map_err(|e| PolicyError::ParseError {
            file: path.display().to_string(),
            reason: e.to_string(),
        })?;

        self.policies.push(policy);
        Ok(())
    }

    /// Sort policies: lower priority number first, then Deny before Allow at
    /// the same priority level.
    fn sort_policies(&mut self) {
        self.policies.sort_by(|a, b| {
            a.priority.cmp(&b.priority).then_with(|| {
                // Deny sorts before Allow so it wins at equal priority
                let a_deny = matches!(a.effect, PolicyEffect::Deny);
                let b_deny = matches!(b.effect, PolicyEffect::Deny);
                b_deny.cmp(&a_deny)
            })
        });
    }

    /// Evaluate all matching policies for the given request.
    ///
    /// Returns [`PolicyDecision::Allow`] only if at least one policy matches
    /// with `Allow` and no policy matches with `Deny`.  If a matching `Allow`
    /// policy specifies `required_approvers`, the result is
    /// [`PolicyDecision::RequireApproval`] instead.  If nothing matches the
    /// default is [`PolicyDecision::Deny`].
    pub fn evaluate(
        &self,
        principal: &str,
        action: &str,
        resource: &str,
        context: &EvaluationContext,
    ) -> PolicyDecision {
        let mut found_allow: Option<&Policy> = None;

        for policy in &self.policies {
            if !pattern_matches(&policy.principal, principal) {
                continue;
            }
            if !pattern_matches(&policy.action, action) {
                continue;
            }
            if !pattern_matches(&policy.resource, resource) {
                continue;
            }

            // Check conditions
            if !self.conditions_met(policy, context) {
                continue;
            }

            match policy.effect {
                PolicyEffect::Deny => {
                    return PolicyDecision::Deny {
                        reason: if policy.description.is_empty() {
                            format!("denied by policy '{}'", policy.policy_id)
                        } else {
                            policy.description.clone()
                        },
                    };
                }
                PolicyEffect::Allow => {
                    if found_allow.is_none() {
                        found_allow = Some(policy);
                    }
                }
            }
        }

        match found_allow {
            Some(policy) => {
                if let Some(approvers) = policy.conditions.required_approvers {
                    let tier = match approvers {
                        0 => HitlTier::Tier0,
                        1 => HitlTier::Tier1,
                        2 => HitlTier::Tier2,
                        _ => HitlTier::Tier3,
                    };
                    PolicyDecision::RequireApproval { tier }
                } else {
                    PolicyDecision::Allow
                }
            }
            None => PolicyDecision::Deny {
                reason: "no matching policy (default deny)".to_string(),
            },
        }
    }

    /// Check whether all conditions on a policy are satisfied.
    fn conditions_met(&self, policy: &Policy, context: &EvaluationContext) -> bool {
        if let Some(min) = policy.conditions.min_autonomy_level {
            let required = autonomy_level_from_u8(min);
            if context.autonomy_level < required {
                return false;
            }
        }
        if let Some(max_fuel) = policy.conditions.max_fuel_cost {
            if let Some(cost) = context.fuel_cost {
                if cost > max_fuel {
                    return false;
                }
            }
        }
        true
    }

    /// Return a reference to the loaded policies.
    pub fn policies(&self) -> &[Policy] {
        &self.policies
    }

    /// Returns `true` when at least one policy is loaded.
    pub fn has_policies(&self) -> bool {
        !self.policies.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn allow_policy(id: &str, principal: &str, action: &str, resource: &str) -> Policy {
        Policy {
            policy_id: id.to_string(),
            description: String::new(),
            effect: PolicyEffect::Allow,
            principal: principal.to_string(),
            action: action.to_string(),
            resource: resource.to_string(),
            priority: 100,
            conditions: PolicyConditions::default(),
        }
    }

    fn deny_policy(id: &str, principal: &str, action: &str, resource: &str) -> Policy {
        Policy {
            policy_id: id.to_string(),
            description: format!("denied by {id}"),
            effect: PolicyEffect::Deny,
            principal: principal.to_string(),
            action: action.to_string(),
            resource: resource.to_string(),
            priority: 100,
            conditions: PolicyConditions::default(),
        }
    }

    #[test]
    fn explicit_allow_passes() {
        let engine =
            PolicyEngine::with_policies(vec![allow_policy("p1", "*", "tool_call", "web.search")]);
        let ctx = EvaluationContext::default();
        let result = engine.evaluate("did:nexus:agent1", "tool_call", "web.search", &ctx);
        assert_eq!(result, PolicyDecision::Allow);
    }

    #[test]
    fn explicit_deny_blocks() {
        let engine = PolicyEngine::with_policies(vec![deny_policy(
            "p1",
            "*",
            "terminal_command",
            "process.exec",
        )]);
        let ctx = EvaluationContext::default();
        let result = engine.evaluate("did:nexus:agent1", "terminal_command", "process.exec", &ctx);
        assert!(matches!(result, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn no_matching_policy_defaults_to_deny() {
        let engine = PolicyEngine::with_policies(vec![allow_policy(
            "p1",
            "did:nexus:special",
            "tool_call",
            "web.search",
        )]);
        let ctx = EvaluationContext::default();
        let result = engine.evaluate("did:nexus:other", "tool_call", "web.search", &ctx);
        match result {
            PolicyDecision::Deny { reason } => {
                assert!(reason.contains("default deny"), "reason: {reason}");
            }
            other => panic!("expected Deny, got {other:?}"),
        }
    }

    #[test]
    fn deny_overrides_allow() {
        let engine = PolicyEngine::with_policies(vec![
            allow_policy("allow-all", "*", "*", "*"),
            deny_policy("block-exec", "*", "terminal_command", "process.exec"),
        ]);
        let ctx = EvaluationContext::default();

        // The allow-all would match, but deny-exec also matches and wins.
        let result = engine.evaluate("did:nexus:agent1", "terminal_command", "process.exec", &ctx);
        assert!(matches!(result, PolicyDecision::Deny { .. }));

        // Other actions still allowed.
        let result = engine.evaluate("did:nexus:agent1", "tool_call", "web.search", &ctx);
        assert_eq!(result, PolicyDecision::Allow);
    }

    #[test]
    fn conditions_min_autonomy_level() {
        let mut policy = allow_policy("p1", "*", "tool_call", "*");
        policy.conditions.min_autonomy_level = Some(2);

        let engine = PolicyEngine::with_policies(vec![policy]);

        // L0 — condition not met, falls through to default deny.
        let ctx_l0 = EvaluationContext {
            autonomy_level: AutonomyLevel::L0,
            fuel_cost: None,
        };
        let result = engine.evaluate("agent1", "tool_call", "web.search", &ctx_l0);
        assert!(matches!(result, PolicyDecision::Deny { .. }));

        // L2 — condition met.
        let ctx_l2 = EvaluationContext {
            autonomy_level: AutonomyLevel::L2,
            fuel_cost: None,
        };
        let result = engine.evaluate("agent1", "tool_call", "web.search", &ctx_l2);
        assert_eq!(result, PolicyDecision::Allow);
    }

    #[test]
    fn conditions_max_fuel_cost() {
        let mut policy = allow_policy("p1", "*", "*", "*");
        policy.conditions.max_fuel_cost = Some(500);

        let engine = PolicyEngine::with_policies(vec![policy]);

        // Under budget — allowed.
        let ctx_ok = EvaluationContext {
            autonomy_level: AutonomyLevel::L5,
            fuel_cost: Some(100),
        };
        assert_eq!(
            engine.evaluate("a", "tool_call", "web.search", &ctx_ok),
            PolicyDecision::Allow,
        );

        // Over budget — condition fails, default deny.
        let ctx_over = EvaluationContext {
            autonomy_level: AutonomyLevel::L5,
            fuel_cost: Some(1000),
        };
        assert!(matches!(
            engine.evaluate("a", "tool_call", "web.search", &ctx_over),
            PolicyDecision::Deny { .. },
        ));
    }

    #[test]
    fn required_approvers_returns_require_approval() {
        let mut policy = allow_policy("p1", "*", "social_post_publish", "*");
        policy.conditions.required_approvers = Some(2);

        let engine = PolicyEngine::with_policies(vec![policy]);
        let ctx = EvaluationContext::default();
        let result = engine.evaluate("agent1", "social_post_publish", "social.post", &ctx);
        assert_eq!(
            result,
            PolicyDecision::RequireApproval {
                tier: HitlTier::Tier2
            }
        );
    }

    #[test]
    fn wildcard_patterns() {
        let engine = PolicyEngine::with_policies(vec![allow_policy("p1", "*", "*", "web.*")]);
        let ctx = EvaluationContext::default();

        assert_eq!(
            engine.evaluate("a", "tool_call", "web.search", &ctx),
            PolicyDecision::Allow,
        );
        assert_eq!(
            engine.evaluate("a", "tool_call", "web.read", &ctx),
            PolicyDecision::Allow,
        );
        assert!(matches!(
            engine.evaluate("a", "tool_call", "fs.write", &ctx),
            PolicyDecision::Deny { .. },
        ));
    }

    #[test]
    fn priority_ordering() {
        // High-priority allow (10) vs low-priority deny (200).
        // But deny always wins — even at lower priority — because the engine
        // scans *all* matching policies and any deny short-circuits.
        let mut allow = allow_policy("allow", "*", "*", "*");
        allow.priority = 10;
        let mut deny = deny_policy("deny", "*", "tool_call", "process.exec");
        deny.priority = 200;

        let engine = PolicyEngine::with_policies(vec![allow, deny]);
        let ctx = EvaluationContext::default();

        // Deny still wins on process.exec because deny overrides allow.
        let result = engine.evaluate("a", "tool_call", "process.exec", &ctx);
        assert!(matches!(result, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn load_from_directory() {
        let dir = tempfile::tempdir().unwrap();
        let policy_toml = r#"
policy_id = "test-allow"
description = "allow all tool calls"
effect = "allow"
principal = "*"
action = "tool_call"
resource = "*"
priority = 50
"#;
        std::fs::write(dir.path().join("allow-tools.toml"), policy_toml).unwrap();

        let mut engine = PolicyEngine::new(dir.path());
        let count = engine.load_policies().unwrap();
        assert_eq!(count, 1);
        assert_eq!(engine.policies()[0].policy_id, "test-allow");
    }

    #[test]
    fn load_nonexistent_dir_returns_zero() {
        let mut engine = PolicyEngine::new("/tmp/nexus-nonexistent-dir-xyz");
        let count = engine.load_policies().unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn empty_engine_denies_everything() {
        let engine = PolicyEngine::with_policies(vec![]);
        let ctx = EvaluationContext::default();
        let result = engine.evaluate("a", "tool_call", "web.search", &ctx);
        assert!(matches!(result, PolicyDecision::Deny { .. }));
    }
}
