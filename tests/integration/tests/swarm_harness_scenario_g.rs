//! Phase BL.3b — Scenario G: non-retryable failure → no retry.
//!
//! Drives the swarm coordinator through HeraldHarnessCapability with a
//! ScriptedPublishExecutor that errors with a CapabilityDenied outcome.
//! The BK fail-closed classifier treats every variant other than
//! `KernelAgentError::SupervisorError` containing "rate limited" as
//! non-retryable. Scenario G asserts that a non-retryable failure:
//!   - propagates immediately (zero retry_attempt NodeEvents),
//!   - consumes exactly one scripted outcome (one publish attempt),
//!   - surfaces a NodeFailed event for the herald node with a reason
//!     mentioning the capability denial,
//!   - the run terminates with SwarmCompleted (per PB-4 — the
//!     coordinator emits SwarmCompleted on Err paths at HEAD; not
//!     SwarmCancelled. Mirror of scenario_d.rs's pattern).
//!
//! Test wall-clock <1s (no retry → no sleep).

use nexus_integration::swarm_harness::prelude::*;
use std::sync::Arc;
use std::time::Duration;

const CANNED_HERALD_PLAN: &str = r#"{
    "nodes": [
        {
            "id": "n1",
            "capability_id": "herald",
            "profile": {
                "reasoning": "Light",
                "tool_use": "None",
                "latency": "Batch",
                "context": "Small",
                "privacy": "StrictLocal",
                "cost": "Free"
            },
            "inputs": {
                "channel": "X",
                "audience": "Rust devs",
                "message": "Tokio 1.50 release",
                "dry_run": false
            }
        }
    ],
    "edges": []
}"#;

#[tokio::test]
async fn non_retryable_failure_emits_no_retry_attempt() {
    let executor = ScriptedPublishExecutor::new(
        true,
        vec![ScriptedOutcome::Capability("social.x.post".into())],
    );

    let scenario = ScenarioBuilder::new()
        .with_canned_plan(CANNED_HERALD_PLAN)
        .with_publish_capability(
            Arc::clone(&executor) as Arc<dyn social_poster_agent::swarm_entry::PublishExecutor>
        )
        .with_oracle_decision(GovernanceDecision::Approved {
            capability_token: "scenario-g-approval-token".to_string(),
        })
        .build();

    // Subscribe BEFORE run so the receiver doesn't miss
    // OracleTicketIssued (emitted at the top of coordinator.run).
    let event_rx = scenario.events_tx.subscribe();

    let planned = scenario
        .plan("post a tweet about Rust")
        .await
        .expect("Director::plan should accept the canned single-node herald plan");

    let _handle = scenario
        .run(planned)
        .await
        .expect("SwarmCoordinator::run should accept the approved plan");

    let events = drain_events_until_terminal(event_rx, Duration::from_secs(10))
        .await
        .expect("a terminal event should arrive within 10s");

    // (A) Terminal is SwarmCompleted, NOT SwarmCancelled. Per PB-4
    //     the coordinator emits SwarmCompleted on Err paths at HEAD
    //     (the spawn block at coordinator.rs:131-148 sets
    //     cancelled=false on Err and emits SwarmCompleted as the
    //     terminal event). Mirrors scenario_d.rs:162-167.
    assert!(
        matches!(events.last(), Some(SwarmEvent::SwarmCompleted { .. })),
        "terminal event should be SwarmCompleted (the spawn block emits it on Err with cancelled=false); got {:?}",
        events.last().map(event_kind_str)
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, SwarmEvent::SwarmCancelled { .. }))
            .count(),
        0,
        "the run is not cancelled"
    );

    // (B) ZERO retry_attempt NodeEvents — the BK fail-closed
    //     classifier treats CapabilityDenied as non-retryable.
    let retry_events: Vec<&SwarmEvent> = events
        .iter()
        .filter(|e| matches!(e, SwarmEvent::NodeEvent { phase, .. } if phase == "retry_attempt"))
        .collect();
    assert!(
        retry_events.is_empty(),
        "non-retryable failure must not trigger retries; got {} retry_attempt events",
        retry_events.len()
    );

    // (C) request_ids_seen has exactly ONE entry (no retry).
    let ids = executor.request_ids_seen().await;
    assert_eq!(
        ids.len(),
        1,
        "non-retryable failure must consume exactly one attempt; got {ids:?}"
    );

    // (D) NodeFailed event for the herald node appears with a
    //     reason mentioning "capability" or "denied" (loose
    //     substring match — mirrors scenario_d.rs:124-141 pattern).
    let herald_failure_reason = events
        .iter()
        .find_map(|e| match e {
            SwarmEvent::NodeFailed { r#ref, reason, .. } if r#ref.node_id == "n1" => {
                Some(reason.clone())
            }
            _ => None,
        })
        .expect("expected a NodeFailed event for the herald node (id n1)");
    let lower = herald_failure_reason.to_lowercase();
    assert!(
        lower.contains("capability") || lower.contains("denied"),
        "NodeFailed reason should reference the capability denial; got: {herald_failure_reason}"
    );

    // (E) capability_call_log: same shape as scenario E.
    //     HeraldHarnessCapability does not push to the shared log.
    //     Single-node herald plan → empty log.
    let call_log = scenario.capability_call_log();
    assert!(
        call_log.is_empty(),
        "single-node herald plan should leave capability_call_log empty (HeraldHarnessCapability does not push); got {call_log:?}"
    );

    // ScriptedPublishExecutor consumed its sole outcome.
    assert_eq!(
        executor.remaining().await,
        0,
        "the single scripted outcome should have been consumed"
    );

    // No NodeCompleted for the herald node (the failure prevented
    // completion). Coordinator-level NodeFailed for "(coordinator)"
    // may also appear per the spawn block's Err handling at
    // coordinator.rs:131-148; the herald NodeFailed at (D) is the
    // primary signal.
    let completed_count = events
        .iter()
        .filter(|e| matches!(e, SwarmEvent::NodeCompleted { .. }))
        .count();
    assert_eq!(
        completed_count, 0,
        "non-retryable failure should not produce a NodeCompleted; got {completed_count}"
    );
}
