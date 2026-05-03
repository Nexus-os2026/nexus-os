//! Phase B.3 — Scenario D: plan drift mid-run.
//!
//! `Director::plan` succeeds against an Approved oracle. Before the
//! coordinator sees the approved plan, `Scenario::run`'s registered
//! DAG mutator injects a structural change that invalidates the
//! approved `ticket.dag_content_hash`. The coordinator's
//! per-iteration drift check (coordinator.rs:177–192) detects the
//! hash mismatch and surfaces it.
//!
//! Asserts (per investigation):
//!   - run() returns Ok(SwarmRunHandle) — the spawned task does the
//!     drift detection asynchronously; run() is fire-and-forget.
//!   - The captured event stream contains at least one
//!     OracleRuntimeCheck whose `highrisk_event` is
//!     `HighRiskEvent::PlanDrift`.
//!   - A NodeFailed event for the synthetic node id "(coordinator)"
//!     fires with "plan drift" in its reason — the coordinator's
//!     execute_loop returns Err and the spawned task surfaces it
//!     via the broadcast.
//!   - The terminal event is SwarmCompleted (cancelled = false in
//!     the spawn block's match path).
//!   - Zero NodeStarted / NodeCompleted events — the synthetic
//!     capabilities never executed because drift fires before any
//!     node is dispatched.
//!   - The capability call log is empty.
//!   - Audit↔events correlation: at HEAD audit payloads from
//!     `SecretsFacade` carry no `run_id` / `ticket_nonce`; the
//!     correlation report's intersection sets are empty by design.
//!     This is documented and asserted as a forward-compatibility
//!     anchor.

use nexus_integration::swarm_harness::prelude::*;
use serde_json::json;
use std::time::Duration;

const CANNED_TWO_NODE_PLAN: &str = r#"{
    "nodes": [
        {
            "id": "n1",
            "capability_id": "herald.post",
            "profile": {
                "reasoning": "Light",
                "tool_use": "None",
                "latency": "Batch",
                "context": "Small",
                "privacy": "StrictLocal",
                "cost": "Free"
            },
            "inputs": {}
        },
        {
            "id": "n2",
            "capability_id": "scout.research",
            "profile": {
                "reasoning": "Light",
                "tool_use": "None",
                "latency": "Batch",
                "context": "Small",
                "privacy": "StrictLocal",
                "cost": "Free"
            },
            "inputs": {}
        }
    ],
    "edges": [{"from": "n1", "to": "n2"}]
}"#;

#[tokio::test]
async fn mid_run_plan_mutation_surfaces_drift_signal() {
    let scenario = ScenarioBuilder::new()
        .with_canned_plan(CANNED_TWO_NODE_PLAN)
        .with_synthetic_capability("herald.post")
        .with_synthetic_capability("scout.research")
        .with_oracle_decision(GovernanceDecision::Approved {
            capability_token: "scenario-d-token".to_string(),
        })
        .with_dag_mutator(|dag| {
            // Smallest semantic mutation that changes
            // dag_content_hash without breaking topology: rewrite
            // an existing node's `inputs` field. The hash covers
            // (id, capability_id, profile, inputs, edges); status
            // is excluded by design (oracle_bridge.rs:391).
            if let Some(node) = dag.get_mut("n1") {
                node.inputs = json!({"drift_injected": true});
            }
        })
        .build();

    let event_rx = scenario.events_tx.subscribe();

    let planned = scenario
        .plan("post a tweet about Rust")
        .await
        .expect("plan should succeed pre-drift");

    let _handle = scenario
        .run(planned)
        .await
        .expect("run() returns Ok(handle); drift surfaces in events");

    let events = drain_events_until_terminal(event_rx, Duration::from_secs(10))
        .await
        .expect("a terminal event should arrive within 10s");

    // Drift signal #1: an OracleRuntimeCheck wrapping a PlanDrift
    // highrisk_event must appear. The coordinator submits the drift
    // to the bridge; with our Approved oracle, the check returns
    // approved (which is still emitted as a runtime check event).
    let drift_check_present = events.iter().any(|e| {
        matches!(
            e,
            SwarmEvent::OracleRuntimeCheck {
                highrisk_event: HighRiskEvent::PlanDrift { .. },
                ..
            }
        )
    });
    assert!(
        drift_check_present,
        "expected at least one OracleRuntimeCheck wrapping HighRiskEvent::PlanDrift; got {:?}",
        events.iter().map(event_kind_str).collect::<Vec<_>>()
    );

    // Drift signal #2: the coordinator's execute_loop returns
    // Err(OraclePolicyDenied) on drift even when the bridge approves
    // the runtime check (coordinator.rs:189–192 — the return is
    // unconditional after a drift detection). The spawn block
    // surfaces it as NodeFailed with node_id "(coordinator)".
    let coordinator_failure_reason = events
        .iter()
        .find_map(|e| match e {
            SwarmEvent::NodeFailed { r#ref, reason, .. } if r#ref.node_id == "(coordinator)" => {
                Some(reason.clone())
            }
            _ => None,
        })
        .expect("expected a coordinator-level NodeFailed event after drift");
    assert!(
        coordinator_failure_reason.to_lowercase().contains("drift")
            || coordinator_failure_reason
                .to_lowercase()
                .contains("oracle policy denied"),
        "coordinator NodeFailed reason should reference drift; got: {coordinator_failure_reason}"
    );

    // Synthetic capabilities never executed — drift detection runs
    // before any node dispatch.
    let node_started_count = events
        .iter()
        .filter(|e| matches!(e, SwarmEvent::NodeStarted { .. }))
        .count();
    let node_completed_count = events
        .iter()
        .filter(|e| matches!(e, SwarmEvent::NodeCompleted { .. }))
        .count();
    assert_eq!(node_started_count, 0, "no node should have started");
    assert_eq!(node_completed_count, 0, "no node should have completed");

    let calls = scenario.capability_call_log();
    assert!(
        calls.is_empty(),
        "no synthetic capability should fire on drift: {calls:?}"
    );

    // Terminal event: per coordinator.rs spawn block, on Err(_) the
    // task sets cancelled=false and emits SwarmCompleted. Verify.
    assert!(
        matches!(events.last(), Some(SwarmEvent::SwarmCompleted { .. })),
        "terminal event should be SwarmCompleted (the spawn block emits it on Err with cancelled=false); got {:?}",
        events.last().map(event_kind_str)
    );

    // Audit ↔ events correlation. At HEAD the SecretsFacade is the
    // only audit emitter wired to AuditTrail and its payloads carry
    // {event, scope, name, result, capability, resolved_from} —
    // none of which reference run_id or ticket_nonce. Director and
    // coordinator do not emit audit events. The intersection sets
    // are therefore expected to be empty. This is a forward-
    // compatibility anchor: AK-2 / future commits that thread run
    // context into audit payloads will flip this assertion and tell
    // us at the harness boundary.
    let audit = scenario.audit.lock().unwrap();
    let report = correlate_audit_with_events(&events, audit.events());
    // Forward-compat anchor: HEAD has no swarm-side
    // audit hooks. When AK-2 or a future swarm-audit
    // commit lands, this assertion will fail.
    // If you're seeing this fail: audit hooks were
    // added to the swarm path. Update this assertion
    // to allow non-zero audit_total, or extend the
    // correlation primitive to assert intersection
    // content rather than absence.
    assert_eq!(report.audit_total, 0);
    assert!(
        !report.event_run_ids.is_empty(),
        "the captured stream covers at least one run_id"
    );
    assert!(
        report.shared_run_ids.is_empty(),
        "audit↔event run_id intersection is empty at HEAD; flips when AK-2 lands"
    );
    assert!(
        report.shared_ticket_nonces.is_empty(),
        "audit↔event ticket_nonce intersection is empty at HEAD"
    );
}
