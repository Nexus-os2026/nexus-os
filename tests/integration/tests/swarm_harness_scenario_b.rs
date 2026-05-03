//! Phase B.2 — Scenario B: execute under synthetic oracle approval.
//!
//! Drives the full Director::plan → SwarmCoordinator::run path against
//! a synthetic-approval oracle. Asserts on the broadcast event sequence
//! end-to-end:
//!   - Plan + run both succeed.
//!   - Exactly one `OracleTicketIssued`, two `NodeStarted`, two
//!     `NodeCompleted`, and one terminal `SwarmCompleted`.
//!   - At least two `BudgetUpdate` events with non-increasing
//!     tokens_remaining (monotonicity sanity).
//!   - No `NodeFailed` events.
//!   - The capability invocation order matches the DAG topological
//!     order (herald.post → scout.research).
//!
//! `SwarmEvent::SwarmCompleted` carries `run_id` only — no embedded
//! summary. The "summary" assertions therefore derive completed /
//! failed / cancelled from event counts and the terminal event variant
//! itself.

use nexus_integration::swarm_harness::prelude::*;
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
async fn execute_under_oracle_approval_emits_correct_event_sequence() {
    let scenario = ScenarioBuilder::new()
        .with_canned_plan(CANNED_TWO_NODE_PLAN)
        .with_synthetic_capability("herald.post")
        .with_synthetic_capability("scout.research")
        .with_oracle_decision(GovernanceDecision::Approved {
            capability_token: "scenario-b-approval-token".to_string(),
        })
        .build();

    // Subscribe BEFORE run so the receiver doesn't miss
    // OracleTicketIssued (emitted at the top of coordinator.run).
    let event_rx = scenario.events_tx.subscribe();

    let planned = scenario
        .plan("post a tweet about Rust")
        .await
        .expect("Director::plan should accept the canned two-node plan");

    let _handle = scenario
        .run(planned)
        .await
        .expect("SwarmCoordinator::run should accept the approved plan");

    let events = drain_events_until_terminal(event_rx, Duration::from_secs(30))
        .await
        .expect("a terminal event should arrive within 30s");

    let kinds: Vec<&str> = events.iter().map(event_kind_str).collect();

    assert_eq!(
        kinds.iter().filter(|k| **k == "OracleTicketIssued").count(),
        1,
        "coordinator emits exactly one OracleTicketIssued at run start"
    );
    assert_eq!(
        kinds.iter().filter(|k| **k == "NodeStarted").count(),
        2,
        "two nodes were planned, two should start"
    );
    assert_eq!(
        kinds.iter().filter(|k| **k == "NodeCompleted").count(),
        2,
        "both nodes complete cleanly under oracle approval"
    );
    assert_eq!(
        kinds.iter().filter(|k| **k == "NodeFailed").count(),
        0,
        "no node should fail in the happy path"
    );
    assert_eq!(
        kinds.iter().filter(|k| **k == "SwarmCompleted").count(),
        1,
        "exactly one terminal SwarmCompleted"
    );
    assert_eq!(
        kinds.iter().filter(|k| **k == "SwarmCancelled").count(),
        0,
        "the run is not cancelled"
    );

    // Topological execution order: herald.post (n1) precedes
    // scout.research (n2) per the edge n1 -> n2.
    let started_order: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            SwarmEvent::NodeStarted { capability_id, .. } => Some(capability_id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        started_order,
        vec!["herald.post".to_string(), "scout.research".to_string()],
        "NodeStarted events should fire in topological order"
    );

    let completed_order: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            SwarmEvent::NodeCompleted { r#ref, .. } => Some(r#ref.node_id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        completed_order,
        vec!["n1".to_string(), "n2".to_string()],
        "NodeCompleted events should fire in topological order"
    );

    // BudgetUpdate monotonicity. The coordinator's current budget
    // accounting is read-only (no try_consume in execute_loop), so
    // every BudgetUpdate carries the same `tokens_remaining`. The
    // assertion is non-increasing rather than strictly decreasing.
    let token_updates: Vec<u64> = events
        .iter()
        .filter_map(|e| match e {
            SwarmEvent::BudgetUpdate {
                tokens_remaining, ..
            } => Some(*tokens_remaining),
            _ => None,
        })
        .collect();
    assert!(
        token_updates.len() >= 2,
        "expected at least two BudgetUpdate events (one per node)"
    );
    for window in token_updates.windows(2) {
        assert!(
            window[1] <= window[0],
            "tokens_remaining must be monotonically non-increasing across BudgetUpdate events"
        );
    }

    // Terminal event must be the last one in the captured stream.
    assert!(
        matches!(events.last(), Some(SwarmEvent::SwarmCompleted { .. })),
        "the captured stream's last event is SwarmCompleted"
    );

    // Capability invocation order via the shared call log.
    let call_log = scenario.capability_call_log();
    assert_eq!(
        call_log,
        vec!["herald.post".to_string(), "scout.research".to_string()],
        "capabilities fire in DAG topological order"
    );
}
