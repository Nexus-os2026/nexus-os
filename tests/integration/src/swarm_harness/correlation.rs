//! Audit ↔ broadcast event correlation primitive.
//!
//! Walks the captured `SwarmEvent` stream and the kernel `AuditTrail`'s
//! events, collects every `run_id` and `ticket_nonce` it sees on each
//! side, and surfaces both the per-side sets and their intersections.
//!
//! At HEAD the kernel `AuditTrail` is wired only to `SecretsFacade`
//! (post-AK-15). Its event payloads carry
//! `{event, scope, name, result, capability, resolved_from}` — none of
//! which reference swarm `run_id` or `ticket_nonce`. Correlation
//! intersections are therefore expected to be empty for B.3 scenarios.
//! The utility is forward-compatible: when AK-2 (capability ledger) or
//! a future commit threads run/ticket identifiers into audit payloads,
//! the same call surfaces real intersections automatically.

use nexus_kernel::audit::AuditEvent;
use nexus_swarm::events::SwarmEvent;
use std::collections::HashSet;
use uuid::Uuid;

/// Intersection report between a captured event stream and an audit
/// trail snapshot. All sets are read-only views; no allocation beyond
/// the report itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelationReport {
    pub events_total: usize,
    pub audit_total: usize,
    pub event_run_ids: HashSet<Uuid>,
    pub event_ticket_nonces: HashSet<Uuid>,
    pub audit_run_ids: HashSet<Uuid>,
    pub audit_ticket_nonces: HashSet<Uuid>,
    pub shared_run_ids: HashSet<Uuid>,
    pub shared_ticket_nonces: HashSet<Uuid>,
}

/// Collect every `run_id` and `ticket_nonce` from `events` and every
/// `run_id` / `ticket_nonce` from `audit_events.payload`, then build
/// the intersections.
pub fn correlate_audit_with_events(
    events: &[SwarmEvent],
    audit_events: &[AuditEvent],
) -> CorrelationReport {
    let mut event_run_ids: HashSet<Uuid> = HashSet::new();
    let mut event_ticket_nonces: HashSet<Uuid> = HashSet::new();

    for event in events {
        if let Some(run_id) = event_run_id(event) {
            event_run_ids.insert(run_id);
        }
        if let Some(nonce) = event_ticket_nonce(event) {
            event_ticket_nonces.insert(nonce);
        }
    }

    let mut audit_run_ids: HashSet<Uuid> = HashSet::new();
    let mut audit_ticket_nonces: HashSet<Uuid> = HashSet::new();

    for audit in audit_events {
        if let Some(uuid) = payload_uuid(&audit.payload, "run_id") {
            audit_run_ids.insert(uuid);
        }
        if let Some(uuid) = payload_uuid(&audit.payload, "ticket_nonce") {
            audit_ticket_nonces.insert(uuid);
        }
    }

    let shared_run_ids = event_run_ids
        .intersection(&audit_run_ids)
        .copied()
        .collect();
    let shared_ticket_nonces = event_ticket_nonces
        .intersection(&audit_ticket_nonces)
        .copied()
        .collect();

    CorrelationReport {
        events_total: events.len(),
        audit_total: audit_events.len(),
        event_run_ids,
        event_ticket_nonces,
        audit_run_ids,
        audit_ticket_nonces,
        shared_run_ids,
        shared_ticket_nonces,
    }
}

fn event_run_id(event: &SwarmEvent) -> Option<Uuid> {
    match event {
        SwarmEvent::PlanProposed { run_id, .. }
        | SwarmEvent::PlanApproved { run_id }
        | SwarmEvent::PlanRejected { run_id, .. }
        | SwarmEvent::BudgetUpdate { run_id, .. }
        | SwarmEvent::SwarmCompleted { run_id }
        | SwarmEvent::SwarmCancelled { run_id } => Some(*run_id),
        SwarmEvent::NodeStarted { r#ref, .. }
        | SwarmEvent::NodeEvent { r#ref, .. }
        | SwarmEvent::NodeCompleted { r#ref, .. }
        | SwarmEvent::NodeFailed { r#ref, .. }
        | SwarmEvent::RouteDenied { r#ref, .. } => Some(r#ref.run_id),
        SwarmEvent::ProviderHealthUpdate { .. }
        | SwarmEvent::OracleTicketIssued { .. }
        | SwarmEvent::OracleRuntimeCheck { .. }
        | SwarmEvent::OracleRuntimeDenial { .. } => None,
    }
}

fn event_ticket_nonce(event: &SwarmEvent) -> Option<Uuid> {
    match event {
        SwarmEvent::NodeStarted { ticket_nonce, .. }
        | SwarmEvent::NodeEvent { ticket_nonce, .. }
        | SwarmEvent::NodeCompleted { ticket_nonce, .. }
        | SwarmEvent::NodeFailed { ticket_nonce, .. }
        | SwarmEvent::BudgetUpdate { ticket_nonce, .. }
        | SwarmEvent::OracleRuntimeCheck { ticket_nonce, .. }
        | SwarmEvent::OracleRuntimeDenial { ticket_nonce, .. } => Some(*ticket_nonce),
        _ => None,
    }
}

fn payload_uuid(payload: &serde_json::Value, key: &str) -> Option<Uuid> {
    payload
        .get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
}
