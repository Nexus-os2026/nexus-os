//! Swarm events emitted over a `tokio::sync::broadcast` channel.
//!
//! Every variant is `Serialize` so the coordinator can ship them as tagged
//! JSON through the Tauri event channel `"swarm:event"`.
//!
//! Phase 1.5b: provider-touching variants carry a `ticket_nonce: Uuid` so
//! every event in a run can be correlated back to the `SwarmTicket` issued
//! by the GovernanceOracle when the DAG was approved. This is the audit
//! primitive that ties the broadcast stream to the oracle's hash-chained
//! decision log. Three new variants (`OracleTicketIssued`,
//! `OracleRuntimeCheck`, `OracleRuntimeDenial`) surface the oracle
//! interactions themselves.

use crate::oracle_policy::{HighRiskEvent, OracleDecisionSummary};
use crate::routing::RouteDenied;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Health status reported by a single provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderHealthStatus {
    Ok,
    Degraded,
    Unhealthy,
}

/// Result of a provider health check, surfaced to the UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderHealth {
    pub provider_id: String,
    pub status: ProviderHealthStatus,
    /// Observed latency in milliseconds; `None` when the probe failed before
    /// a response was received.
    pub latency_ms: Option<u64>,
    pub models: Vec<String>,
    /// Free-form notes (e.g. `"api_key not in keyring"`,
    /// `"spend: $0.42 / $2.00"`).
    pub notes: String,
    /// Unix timestamp (seconds) when the probe completed.
    pub checked_at_secs: i64,
}

/// Coarse identity of a DAG node, used for event addressing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRef {
    pub run_id: Uuid,
    pub node_id: String,
}

/// The single event vocabulary emitted by the coordinator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SwarmEvent {
    PlanProposed {
        run_id: Uuid,
        dag_json: serde_json::Value,
    },
    PlanApproved {
        run_id: Uuid,
    },
    PlanRejected {
        run_id: Uuid,
        reason: String,
    },
    NodeStarted {
        r#ref: NodeRef,
        capability_id: String,
        provider_id: String,
        model_id: String,
        ticket_nonce: Uuid,
    },
    /// Free-form progress payload from a node — streaming tokens, subtask
    /// updates, etc.
    NodeEvent {
        r#ref: NodeRef,
        phase: String,
        payload: serde_json::Value,
        ticket_nonce: Uuid,
    },
    NodeCompleted {
        r#ref: NodeRef,
        result: serde_json::Value,
        ticket_nonce: Uuid,
    },
    NodeFailed {
        r#ref: NodeRef,
        reason: String,
        ticket_nonce: Uuid,
    },
    RouteDenied {
        r#ref: NodeRef,
        denied: RouteDenied,
    },
    BudgetUpdate {
        run_id: Uuid,
        tokens_remaining: u64,
        cents_remaining: u32,
        wall_ms_remaining: u64,
        ticket_nonce: Uuid,
    },
    ProviderHealthUpdate {
        providers: Vec<ProviderHealth>,
    },
    SwarmCompleted {
        run_id: Uuid,
    },
    SwarmCancelled {
        run_id: Uuid,
    },

    /// Emitted once when the Director receives a sealed approval token from
    /// the oracle. Carries only the opaque identifiers the frontend needs to
    /// correlate a run with its ticket; the full SwarmTicket (including the
    /// SealedToken body) is kept server-side.
    OracleTicketIssued {
        ticket_id: Uuid,
        budget_hash: String,
        dag_content_hash: String,
    },

    /// A runtime high-risk re-check was submitted to the oracle; `decision`
    /// summarizes the outcome. Always emitted when a check is made — both
    /// on approval and denial — so the event stream reflects every oracle
    /// interaction, not just the failures. Field named `highrisk_event`
    /// rather than `event` to avoid collision with the enum's serde tag.
    OracleRuntimeCheck {
        ticket_nonce: Uuid,
        highrisk_event: HighRiskEvent,
        decision: OracleDecisionSummary,
    },

    /// A runtime high-risk check was denied. The corresponding `NodeFailed`
    /// is emitted separately by the coordinator's node-failure path.
    OracleRuntimeDenial {
        ticket_nonce: Uuid,
        hints: Vec<String>,
        node_id: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_proposed_round_trips() {
        let ev = SwarmEvent::PlanProposed {
            run_id: Uuid::nil(),
            dag_json: serde_json::json!({"nodes": [], "edges": []}),
        };
        let j = serde_json::to_string(&ev).unwrap();
        assert!(j.contains("plan_proposed"));
        let _back: SwarmEvent = serde_json::from_str(&j).unwrap();
    }

    #[test]
    fn node_failed_reason_survives_round_trip() {
        let ev = SwarmEvent::NodeFailed {
            r#ref: NodeRef {
                run_id: Uuid::nil(),
                node_id: "n1".into(),
            },
            reason: "provider unreachable".into(),
            ticket_nonce: Uuid::nil(),
        };
        let j = serde_json::to_string(&ev).unwrap();
        assert!(j.contains("provider unreachable"));
    }

    #[test]
    fn provider_health_update_serializes() {
        let ev = SwarmEvent::ProviderHealthUpdate {
            providers: vec![ProviderHealth {
                provider_id: "ollama".into(),
                status: ProviderHealthStatus::Ok,
                latency_ms: Some(12),
                models: vec!["gemma4:e2b".into()],
                notes: String::new(),
                checked_at_secs: 0,
            }],
        };
        let j = serde_json::to_string(&ev).unwrap();
        assert!(j.contains("provider_health_update"));
        assert!(j.contains("gemma4:e2b"));
    }
}
