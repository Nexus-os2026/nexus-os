//! Typed swarm errors. No silent failures, no panics.

use crate::budget::BudgetError;
use crate::provider::ProviderError;
use crate::routing::RouteDenied;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SwarmError {
    #[error("capability not found in registry: {0}")]
    RegistryMiss(String),

    #[error("director failed to parse provider plan: {0}")]
    DirectorParse(String),

    #[error("director provider is unavailable: {0}")]
    DirectorUnavailable(String),

    #[error("governance denied capability request for agent `{agent_id}`")]
    GovernanceDenied { agent_id: String },

    #[error("budget exhausted: {0}")]
    BudgetExhausted(#[from] BudgetError),

    #[error("DAG cycle detected when inserting edge {from} -> {to}")]
    DagCycle { from: String, to: String },

    #[error("governance oracle timed out after {wait_ms}ms")]
    OracleTimeout { wait_ms: u64 },

    #[error("route denied: {0}")]
    RouteDenied(RouteDenied),

    #[error("provider `{provider_id}` unreachable: {reason}")]
    ProviderUnreachable { provider_id: String, reason: String },

    #[error("Anthropic spend cap exceeded: ${spent:.2} / ${cap:.2}")]
    SpendCapExceeded { spent: f64, cap: f64 },

    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error("sub-agent spawning is not supported in Phase 1")]
    SubagentSpawnRejected,
}

impl From<RouteDenied> for SwarmError {
    fn from(d: RouteDenied) -> Self {
        SwarmError::RouteDenied(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_error_converts() {
        let e = SwarmError::from(BudgetError::TokensExhausted {
            requested: 5,
            remaining: 1,
        });
        assert!(matches!(e, SwarmError::BudgetExhausted(_)));
    }

    #[test]
    fn display_includes_agent_id() {
        let e = SwarmError::GovernanceDenied {
            agent_id: "broker".into(),
        };
        assert!(e.to_string().contains("broker"));
    }
}
