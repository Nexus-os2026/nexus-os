//! The Decision Engine — runs in complete isolation from agent requests.
//!
//! Receives requests from a queue, evaluates against the governance model,
//! and places decisions in response channels.

use tokio::sync::mpsc;

use nexus_governance_oracle::{CapabilityRequest, GovernanceDecision, OracleRequest};

use crate::audit::DecisionAuditLog;
use crate::rules::{GovernanceRuleset, RuleResult};

/// The isolated decision engine.
pub struct DecisionEngine {
    request_rx: mpsc::Receiver<OracleRequest>,
    ruleset: GovernanceRuleset,
    audit_log: DecisionAuditLog,
}

impl DecisionEngine {
    pub fn new(request_rx: mpsc::Receiver<OracleRequest>, ruleset: GovernanceRuleset) -> Self {
        Self {
            request_rx,
            ruleset,
            audit_log: DecisionAuditLog::new(),
        }
    }

    /// Run the decision loop — processes requests until the channel closes.
    pub async fn run(&mut self) {
        while let Some(oracle_request) = self.request_rx.recv().await {
            let decision = self.evaluate(&oracle_request.request);

            self.audit_log.record(
                &oracle_request.request,
                &decision,
                &self.ruleset.version_hash(),
            );

            let _ = oracle_request.response_tx.send(decision);
        }
    }

    /// Evaluate a capability request against the current ruleset.
    /// Deny-by-default: if no rule explicitly allows, deny.
    fn evaluate(&self, request: &CapabilityRequest) -> GovernanceDecision {
        self.evaluate_request(request, &self.ruleset)
    }

    /// Synchronous evaluation against a given ruleset (for evolution engine testing).
    pub fn evaluate_request(
        &self,
        request: &CapabilityRequest,
        ruleset: &GovernanceRuleset,
    ) -> GovernanceDecision {
        for rule in &ruleset.rules {
            match rule.evaluate(request) {
                RuleResult::Deny => return GovernanceDecision::Denied,
                RuleResult::Allow => {
                    return GovernanceDecision::Approved {
                        capability_token: uuid::Uuid::new_v4().to_string(),
                    };
                }
                RuleResult::NoMatch => continue,
            }
        }
        // Default deny
        GovernanceDecision::Denied
    }

    /// Hot-swap the governance ruleset.
    pub fn update_ruleset(&mut self, new_ruleset: GovernanceRuleset) {
        self.ruleset = new_ruleset;
    }

    pub fn ruleset(&self) -> &GovernanceRuleset {
        &self.ruleset
    }

    pub fn audit_log(&self) -> &DecisionAuditLog {
        &self.audit_log
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{GovernanceRule, RuleCondition, RuleEffect};

    fn make_request(agent: &str, cap: &str) -> CapabilityRequest {
        CapabilityRequest {
            agent_id: agent.into(),
            capability: cap.into(),
            parameters: serde_json::json!({}),
            budget_hash: String::new(),
            request_nonce: "n".into(),
        }
    }

    fn default_ruleset() -> GovernanceRuleset {
        GovernanceRuleset::new(
            "test".into(),
            1,
            vec![GovernanceRule {
                id: "allow-llm".into(),
                description: "Allow LLM queries".into(),
                effect: RuleEffect::Allow,
                conditions: vec![RuleCondition::CapabilityInSet(vec!["llm.query".into()])],
            }],
        )
    }

    #[test]
    fn test_deny_by_default() {
        let (_, rx) = mpsc::channel::<OracleRequest>(1);
        let engine = DecisionEngine::new(rx, default_ruleset());

        let decision = engine.evaluate(&make_request("a1", "process.exec"));
        assert_eq!(decision, GovernanceDecision::Denied);
    }

    #[test]
    fn test_allow_matching_rule() {
        let (_, rx) = mpsc::channel::<OracleRequest>(1);
        let engine = DecisionEngine::new(rx, default_ruleset());

        let decision = engine.evaluate(&make_request("a1", "llm.query"));
        assert!(matches!(decision, GovernanceDecision::Approved { .. }));
    }

    #[test]
    fn test_ruleset_hot_swap() {
        let (_, rx) = mpsc::channel::<OracleRequest>(1);
        let mut engine = DecisionEngine::new(rx, default_ruleset());

        // Initially, fs.write is denied
        let d1 = engine.evaluate(&make_request("a1", "fs.write"));
        assert_eq!(d1, GovernanceDecision::Denied);

        // Hot-swap ruleset to allow fs.write
        let new_ruleset = GovernanceRuleset::new(
            "test".into(),
            2,
            vec![
                GovernanceRule {
                    id: "allow-llm".into(),
                    description: "Allow LLM".into(),
                    effect: RuleEffect::Allow,
                    conditions: vec![RuleCondition::CapabilityInSet(vec!["llm.query".into()])],
                },
                GovernanceRule {
                    id: "allow-fs".into(),
                    description: "Allow FS write".into(),
                    effect: RuleEffect::Allow,
                    conditions: vec![RuleCondition::CapabilityInSet(vec!["fs.write".into()])],
                },
            ],
        );
        engine.update_ruleset(new_ruleset);

        // Now fs.write should be allowed
        let d2 = engine.evaluate(&make_request("a1", "fs.write"));
        assert!(matches!(d2, GovernanceDecision::Approved { .. }));
    }
}
