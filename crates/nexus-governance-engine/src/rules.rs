//! Governance ruleset — versioned, hashable, deny-first policy logic.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use nexus_governance_oracle::CapabilityRequest;

/// A complete governance ruleset — versioned and hashable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceRuleset {
    pub id: String,
    pub version: u64,
    /// Rules sorted: deny rules first (higher priority), then allow rules.
    pub rules: Vec<GovernanceRule>,
    pub created_at: u64,
    hash: Option<String>,
}

impl GovernanceRuleset {
    pub fn new(id: String, version: u64, mut rules: Vec<GovernanceRule>) -> Self {
        rules.sort_by_key(|r| match r.effect {
            RuleEffect::Deny => 0,
            RuleEffect::Allow => 1,
        });

        let mut rs = Self {
            id,
            version,
            rules,
            created_at: epoch_secs(),
            hash: None,
        };
        rs.hash = Some(rs.compute_hash());
        rs
    }

    pub fn version_hash(&self) -> String {
        self.hash.clone().unwrap_or_else(|| self.compute_hash())
    }

    fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.id.as_bytes());
        hasher.update(self.version.to_le_bytes());
        for rule in &self.rules {
            hasher.update(rule.id.as_bytes());
            hasher.update(
                serde_json::to_string(&rule.conditions)
                    .unwrap_or_default()
                    .as_bytes(),
            );
        }
        format!("{:x}", hasher.finalize())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceRule {
    pub id: String,
    pub description: String,
    pub effect: RuleEffect,
    pub conditions: Vec<RuleCondition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleEffect {
    Allow,
    Deny,
}

/// Conditions that must ALL be true for the rule to match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleCondition {
    MaxAutonomyLevel(u8),
    RequiresCapability(String),
    CapabilityInSet(Vec<String>),
    CapabilityNotInSet(Vec<String>),
    MinBudgetRemaining {
        capability: String,
        minimum: u64,
    },
    ParameterMatch {
        key: String,
        expected: serde_json::Value,
    },
    MaxLineageDepth(u32),
    TimeWindow {
        start: u64,
        end: u64,
    },
}

/// Result of evaluating a rule against a request.
pub enum RuleResult {
    Allow,
    Deny,
    NoMatch,
}

impl GovernanceRule {
    pub fn evaluate(&self, request: &CapabilityRequest) -> RuleResult {
        let all_match = self.conditions.iter().all(|cond| match cond {
            RuleCondition::CapabilityInSet(set) => set.contains(&request.capability),
            RuleCondition::CapabilityNotInSet(set) => !set.contains(&request.capability),
            RuleCondition::MaxAutonomyLevel(max) => request
                .parameters
                .get("autonomy_level")
                .and_then(|v| v.as_u64())
                .map(|l| l <= *max as u64)
                .unwrap_or(false),
            RuleCondition::RequiresCapability(cap) => request
                .parameters
                .get("capabilities")
                .and_then(|v| v.as_array())
                .map(|caps| caps.iter().any(|c| c.as_str() == Some(cap.as_str())))
                .unwrap_or(false),
            RuleCondition::MinBudgetRemaining {
                capability,
                minimum,
            } => request
                .parameters
                .get("budget_remaining")
                .and_then(|v| v.get(capability.as_str()))
                .and_then(|v| v.as_u64())
                .map(|remaining| remaining >= *minimum)
                .unwrap_or(false),
            RuleCondition::ParameterMatch { key, expected } => {
                request.parameters.get(key.as_str()) == Some(expected)
            }
            RuleCondition::MaxLineageDepth(max) => request
                .parameters
                .get("lineage_depth")
                .and_then(|v| v.as_u64())
                .map(|d| d <= *max as u64)
                .unwrap_or(false),
            RuleCondition::TimeWindow { start, end } => {
                let now = epoch_secs();
                now >= *start && now <= *end
            }
        });

        if all_match {
            match self.effect {
                RuleEffect::Allow => RuleResult::Allow,
                RuleEffect::Deny => RuleResult::Deny,
            }
        } else {
            RuleResult::NoMatch
        }
    }
}

fn epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allow_rule(id: &str, caps: Vec<&str>) -> GovernanceRule {
        GovernanceRule {
            id: id.into(),
            description: "test allow".into(),
            effect: RuleEffect::Allow,
            conditions: vec![RuleCondition::CapabilityInSet(
                caps.into_iter().map(String::from).collect(),
            )],
        }
    }

    fn deny_rule(id: &str, caps: Vec<&str>) -> GovernanceRule {
        GovernanceRule {
            id: id.into(),
            description: "test deny".into(),
            effect: RuleEffect::Deny,
            conditions: vec![RuleCondition::CapabilityInSet(
                caps.into_iter().map(String::from).collect(),
            )],
        }
    }

    fn make_request(cap: &str) -> CapabilityRequest {
        CapabilityRequest {
            agent_id: "agent-1".into(),
            capability: cap.into(),
            parameters: serde_json::json!({}),
            budget_hash: String::new(),
            request_nonce: "n1".into(),
        }
    }

    #[test]
    fn test_deny_rule_priority() {
        let ruleset = GovernanceRuleset::new(
            "test".into(),
            1,
            vec![
                allow_rule("a1", vec!["llm.query"]),
                deny_rule("d1", vec!["llm.query"]),
            ],
        );
        // Deny rules must be sorted first
        assert!(matches!(ruleset.rules[0].effect, RuleEffect::Deny));
        assert!(matches!(ruleset.rules[1].effect, RuleEffect::Allow));
    }

    #[test]
    fn test_capability_allowlist() {
        let rule = allow_rule("a1", vec!["llm.query", "fs.read"]);
        assert!(matches!(
            rule.evaluate(&make_request("llm.query")),
            RuleResult::Allow
        ));
        assert!(matches!(
            rule.evaluate(&make_request("process.exec")),
            RuleResult::NoMatch
        ));
    }

    #[test]
    fn test_capability_denylist() {
        let rule = GovernanceRule {
            id: "d1".into(),
            description: "deny dangerous".into(),
            effect: RuleEffect::Deny,
            conditions: vec![RuleCondition::CapabilityNotInSet(vec![
                "llm.query".into(),
                "fs.read".into(),
            ])],
        };
        // "process.exec" is NOT in the safe set, so CapabilityNotInSet matches → Deny
        assert!(matches!(
            rule.evaluate(&make_request("process.exec")),
            RuleResult::Deny
        ));
        // "llm.query" IS in the safe set, so CapabilityNotInSet does NOT match → NoMatch
        assert!(matches!(
            rule.evaluate(&make_request("llm.query")),
            RuleResult::NoMatch
        ));
    }

    #[test]
    fn test_ruleset_versioning() {
        let rs1 = GovernanceRuleset::new("v1".into(), 1, vec![allow_rule("a1", vec!["llm.query"])]);
        let rs2 = GovernanceRuleset::new("v1".into(), 2, vec![allow_rule("a1", vec!["llm.query"])]);
        assert_ne!(rs1.version_hash(), rs2.version_hash());
    }
}
