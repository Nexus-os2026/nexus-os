//! The Decision Engine — runs in complete isolation from agent requests.
//!
//! Receives requests from a queue, evaluates against the governance model,
//! and places decisions in response channels.
//!
//! Bug M: ruleset is held behind `Arc<RwLock<...>>` so hot-swaps from
//! `AppState::update_governance_ruleset` reach the running engine within
//! milliseconds — no engine restart required. Reads (per request) take a
//! read-lock; writes (rare, evolution-driven) take a write-lock.

use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

use nexus_governance_oracle::{CapabilityRequest, GovernanceDecision, OracleRequest};

use crate::audit::DecisionAuditLog;
use crate::rules::{GovernanceRuleset, RuleResult};

/// Shared, hot-swappable handle to the governance ruleset.
pub type RulesetHandle = Arc<RwLock<GovernanceRuleset>>;

/// The isolated decision engine.
pub struct DecisionEngine {
    request_rx: mpsc::Receiver<OracleRequest>,
    ruleset: RulesetHandle,
    audit_log: DecisionAuditLog,
}

impl DecisionEngine {
    /// Construct with an owned ruleset. Internally wraps it in
    /// `Arc<RwLock<...>>`; callers that want shared swap access should
    /// use [`Self::with_shared_ruleset`] instead and clone the Arc.
    pub fn new(request_rx: mpsc::Receiver<OracleRequest>, ruleset: GovernanceRuleset) -> Self {
        Self::with_shared_ruleset(request_rx, Arc::new(RwLock::new(ruleset)))
    }

    /// Construct sharing an external `Arc<RwLock<GovernanceRuleset>>`.
    /// Used by `OracleRuntime` so AppState can hot-swap the ruleset and
    /// the engine sees the change on the next request.
    pub fn with_shared_ruleset(
        request_rx: mpsc::Receiver<OracleRequest>,
        ruleset: RulesetHandle,
    ) -> Self {
        Self {
            request_rx,
            ruleset,
            audit_log: DecisionAuditLog::new(),
        }
    }

    /// Run the decision loop — processes requests until the channel closes.
    pub async fn run(&mut self) {
        while let Some(oracle_request) = self.request_rx.recv().await {
            // Read-lock per request; release before audit append so the
            // lock is held for as little time as possible.
            let (decision, version_hash) = {
                let guard = self.ruleset.read().unwrap_or_else(|p| p.into_inner());
                let decision = self.evaluate_request(&oracle_request.request, &guard);
                let hash = guard.version_hash();
                (decision, hash)
            };

            self.audit_log
                .record(&oracle_request.request, &decision, &version_hash);

            let _ = oracle_request.response_tx.send(decision);
        }
    }

    /// Evaluate a request against the engine's current (live) ruleset.
    /// Reads under the shared read-lock — equivalent to what `run` does
    /// per request. Kept for in-crate test ergonomics.
    pub fn evaluate(&self, request: &CapabilityRequest) -> GovernanceDecision {
        let guard = self.ruleset.read().unwrap_or_else(|p| p.into_inner());
        self.evaluate_request(request, &guard)
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

    /// Hot-swap the governance ruleset. Writes through the shared
    /// `Arc<RwLock<...>>`; the change is visible to the next request the
    /// engine processes.
    pub fn update_ruleset(&mut self, new_ruleset: GovernanceRuleset) {
        let mut guard = self.ruleset.write().unwrap_or_else(|p| p.into_inner());
        *guard = new_ruleset;
    }

    /// Return a clone of the shared ruleset handle. `OracleRuntime`
    /// exposes this clone so `AppState` can hold a sibling Arc and write
    /// through it from outside the engine task.
    pub fn ruleset_handle(&self) -> RulesetHandle {
        Arc::clone(&self.ruleset)
    }

    /// Snapshot accessor for callers that just want to inspect the
    /// current ruleset. Clones to escape the lock guard.
    pub fn ruleset_snapshot(&self) -> GovernanceRuleset {
        self.ruleset
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
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

    /// Bug M: writes through a sibling `Arc<RwLock<...>>` (the handle the
    /// engine returns from `ruleset_handle()`) are visible on subsequent
    /// engine reads. This is the fundamental property the hot-swap path
    /// in OracleRuntime depends on.
    #[test]
    fn test_shared_ruleset_handle_propagates_writes() {
        let (_, rx) = mpsc::channel::<OracleRequest>(1);
        let shared: RulesetHandle = Arc::new(RwLock::new(default_ruleset()));
        let engine = DecisionEngine::with_shared_ruleset(rx, Arc::clone(&shared));

        // Initially: fs.write denied (default_ruleset only allows llm.query).
        let d1 = engine.evaluate(&make_request("a1", "fs.write"));
        assert_eq!(d1, GovernanceDecision::Denied);

        // Mutate the ruleset through the SHARED Arc (not through the
        // engine's &mut self). The engine must see the change on the next
        // evaluate() call.
        {
            let mut guard = shared.write().expect("write-lock");
            *guard = GovernanceRuleset::new(
                "test".into(),
                2,
                vec![GovernanceRule {
                    id: "allow-fs".into(),
                    description: "Allow FS write".into(),
                    effect: RuleEffect::Allow,
                    conditions: vec![RuleCondition::CapabilityInSet(vec!["fs.write".into()])],
                }],
            );
        }

        let d2 = engine.evaluate(&make_request("a1", "fs.write"));
        assert!(
            matches!(d2, GovernanceDecision::Approved { .. }),
            "engine must see the swapped ruleset on the next read; got {d2:?}"
        );

        // Engine's own handle is the same Arc the test holds.
        assert!(
            Arc::ptr_eq(&shared, &engine.ruleset_handle()),
            "ruleset_handle() must return the same Arc the engine reads from"
        );
    }
}
