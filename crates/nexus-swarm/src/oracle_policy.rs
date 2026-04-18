//! High-risk event vocabulary and the local policy that decides whether
//! each event triggers a runtime re-check against the GovernanceOracle.
//!
//! Hybrid governance rationale (see `oracle_bridge.rs` for the full
//! paragraph): the oracle approves a DAG once at plan time; the coordinator
//! then does cheap local checks per node. Only a small, exhaustively
//! enumerated set of events — defined in [`HighRiskEvent`] — re-invokes the
//! oracle. This gives us class-based governance instead of rate-limited
//! middleware.
//!
//! Every variant here must map to a real, checkable condition at runtime.
//! Do not add speculative cases without a corresponding coordinator check.

use crate::profile::PrivacyClass;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Compact, serializable summary of an oracle decision for a runtime check.
/// Emitted on `SwarmEvent::OracleRuntimeCheck` so the frontend can render
/// the audit trail without seeing the full `TokenPayload` (which contains
/// nonces and timestamps that are server-side correlation primitives).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OracleDecisionSummary {
    pub approved: bool,
    /// Ticket id of the issued sealed token on approval; `None` on denial.
    /// Not a secret — the ticket body stays server-side; this is an opaque
    /// correlation handle only.
    pub token_id: Option<Uuid>,
}

/// Closed vocabulary of runtime events that may trigger an oracle re-check.
/// No other coordinator condition should ever construct a `HighRiskEvent`
/// directly — add a variant first, with a documented trigger rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HighRiskEvent {
    /// A routed node would invoke a cloud provider with a non-trivial
    /// estimated cost. Triggers when both (a) the estimate exceeds 10¢
    /// and (b) the provider is not local / codex-cli / free-tier.
    CloudCallAboveThreshold {
        provider_id: String,
        estimated_cents: u32,
    },

    /// A node attempted to spawn a sub-agent. Phase 1 bans sub-agent spawn
    /// at the adapter layer; this is the governance gate that would fire
    /// in Phase 2+ if a depth>0 spawn were ever attempted.
    SubagentSpawnAttempt { parent_node: String, depth: u8 },

    /// A node's resolved route would escalate the privacy envelope —
    /// running a `Sensitive` task on a `Public` provider, for example.
    /// The router already hard-denies this at resolve time; the event is
    /// retained so the oracle sees the attempt in its audit stream.
    PrivacyClassEscalation {
        from: PrivacyClass,
        to: PrivacyClass,
    },

    /// The coordinator has consumed ≥80% of the approved budget (taken as
    /// the minimum across tokens / cost / wall-clock). Gives the oracle a
    /// chance to abort before the budget is exhausted.
    BudgetSoftLimitApproach { consumed_pct: u8 },

    /// The DAG content hash at spawn time does not match the hash captured
    /// in the approved `SwarmTicket`. Indicates post-approval mutation;
    /// MUST abort the run.
    PlanDrift {
        original_hash: String,
        current_hash: String,
    },
}

/// Locally-evaluated policy: given a `HighRiskEvent`, should we bother the
/// oracle? These rules match the spec verbatim and are fixed — the policy
/// is not configurable at runtime because loosening it is a governance
/// regression, and tightening it belongs in a new variant, not a knob.
#[derive(Debug, Default, Clone, Copy)]
pub struct HighRiskPolicy;

impl HighRiskPolicy {
    /// Providers whose pricing is zero or effectively free. Cloud calls
    /// against these are not subject to the cost-threshold re-check even
    /// when the estimator returns a number above the threshold.
    const FREE_OR_LOCAL: &'static [&'static str] = &["ollama", "codex-cli", "huggingface"];

    pub fn new() -> Self {
        Self
    }

    /// Spec-defined cost threshold. Fixed at 10¢ per invocation.
    pub const CLOUD_COST_THRESHOLD_CENTS: u32 = 10;

    /// Spec-defined budget soft-limit trigger: ≥80% consumed.
    pub const BUDGET_SOFT_LIMIT_PCT: u8 = 80;

    /// Should this event be forwarded to the oracle for a runtime re-check?
    pub fn should_recheck(&self, event: &HighRiskEvent) -> bool {
        match event {
            HighRiskEvent::CloudCallAboveThreshold {
                provider_id,
                estimated_cents,
            } => {
                *estimated_cents > Self::CLOUD_COST_THRESHOLD_CENTS
                    && !Self::FREE_OR_LOCAL.contains(&provider_id.as_str())
            }
            HighRiskEvent::SubagentSpawnAttempt { depth, .. } => *depth > 0,
            HighRiskEvent::PrivacyClassEscalation { from, to } => matches!(
                (from, to),
                (PrivacyClass::Sensitive, PrivacyClass::Public)
                    | (PrivacyClass::StrictLocal, PrivacyClass::Public)
                    | (PrivacyClass::StrictLocal, PrivacyClass::Sensitive)
            ),
            HighRiskEvent::BudgetSoftLimitApproach { consumed_pct } => {
                *consumed_pct >= Self::BUDGET_SOFT_LIMIT_PCT
            }
            HighRiskEvent::PlanDrift {
                original_hash,
                current_hash,
            } => original_hash != current_hash,
        }
    }
}

/// Return type of `SwarmOracleBridge::check_highrisk`. `hints` are locally
/// synthesized from the denial class that was tripped — the oracle carries
/// no reason in its `Denied` decision by design. See `oracle_bridge.rs`.
#[derive(Debug, Clone)]
pub struct OracleDenial {
    pub hints: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_cost_threshold_rejects_cheap_cloud() {
        let p = HighRiskPolicy::new();
        assert!(!p.should_recheck(&HighRiskEvent::CloudCallAboveThreshold {
            provider_id: "openai".into(),
            estimated_cents: 5,
        }));
    }

    #[test]
    fn cloud_cost_threshold_triggers_above_10_cents() {
        let p = HighRiskPolicy::new();
        assert!(p.should_recheck(&HighRiskEvent::CloudCallAboveThreshold {
            provider_id: "openai".into(),
            estimated_cents: 11,
        }));
    }

    #[test]
    fn cloud_cost_threshold_skips_local_providers() {
        let p = HighRiskPolicy::new();
        assert!(!p.should_recheck(&HighRiskEvent::CloudCallAboveThreshold {
            provider_id: "ollama".into(),
            estimated_cents: 1_000,
        }));
        assert!(!p.should_recheck(&HighRiskEvent::CloudCallAboveThreshold {
            provider_id: "codex-cli".into(),
            estimated_cents: 1_000,
        }));
    }

    #[test]
    fn subagent_spawn_triggers_on_any_depth() {
        let p = HighRiskPolicy::new();
        assert!(!p.should_recheck(&HighRiskEvent::SubagentSpawnAttempt {
            parent_node: "a".into(),
            depth: 0,
        }));
        assert!(p.should_recheck(&HighRiskEvent::SubagentSpawnAttempt {
            parent_node: "a".into(),
            depth: 1,
        }));
    }

    #[test]
    fn privacy_escalation_triggers_sensitive_to_public() {
        let p = HighRiskPolicy::new();
        assert!(p.should_recheck(&HighRiskEvent::PrivacyClassEscalation {
            from: PrivacyClass::Sensitive,
            to: PrivacyClass::Public,
        }));
    }

    #[test]
    fn privacy_escalation_skips_public_to_sensitive() {
        let p = HighRiskPolicy::new();
        assert!(!p.should_recheck(&HighRiskEvent::PrivacyClassEscalation {
            from: PrivacyClass::Public,
            to: PrivacyClass::Sensitive,
        }));
    }

    #[test]
    fn budget_soft_limit_triggers_at_80_pct() {
        let p = HighRiskPolicy::new();
        assert!(!p.should_recheck(&HighRiskEvent::BudgetSoftLimitApproach { consumed_pct: 79 }));
        assert!(p.should_recheck(&HighRiskEvent::BudgetSoftLimitApproach { consumed_pct: 80 }));
        assert!(p.should_recheck(&HighRiskEvent::BudgetSoftLimitApproach { consumed_pct: 100 }));
    }

    #[test]
    fn plan_drift_triggers_on_mismatch() {
        let p = HighRiskPolicy::new();
        assert!(p.should_recheck(&HighRiskEvent::PlanDrift {
            original_hash: "a".into(),
            current_hash: "b".into(),
        }));
        assert!(!p.should_recheck(&HighRiskEvent::PlanDrift {
            original_hash: "a".into(),
            current_hash: "a".into(),
        }));
    }
}
