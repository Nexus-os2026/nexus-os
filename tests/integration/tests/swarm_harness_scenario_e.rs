//! Phase BL.3a — Scenario E: retryable failure → retry → success.
//!
//! Drives the swarm coordinator through HeraldHarnessCapability with a
//! ScriptedPublishExecutor that errors once with a rate-limit
//! SupervisorError, then succeeds. Asserts that the BK retry decorator:
//!   - retries the failure (second attempt reaches the executor),
//!   - emits exactly one `NodeEvent` with phase="retry_attempt"
//!     (BK.3 contract: emission only on attempt_num >= 2),
//!   - reuses the same `request_id` across both attempts (BK.2
//!     contract via the lifted-once-then-reused UUID),
//!   - completes the node successfully with a final `NodeCompleted`,
//!   - the run terminates with `SwarmCompleted` (no `SwarmCancelled`).
//!
//! The first scripted SupervisorError carries
//! "retry after 1 ms"; per parse_retry_after_ms_to_secs's div_ceil
//! rounding this becomes a 1-second hint; the decorator caps and waits
//! ~1 second before the second attempt. Test wall-clock ~1.5s.

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
async fn retryable_failure_then_success_emits_retry_attempt() {
    let executor = ScriptedPublishExecutor::new(
        true,
        vec![
            ScriptedOutcome::SupervisorErr("social.x rate limited, retry after 1 ms".into()),
            ScriptedOutcome::Ok("tw-success-after-retry".into()),
        ],
    );

    let scenario = ScenarioBuilder::new()
        .with_canned_plan(CANNED_HERALD_PLAN)
        .with_publish_capability(
            Arc::clone(&executor) as Arc<dyn social_poster_agent::swarm_entry::PublishExecutor>
        )
        .with_oracle_decision(GovernanceDecision::Approved {
            capability_token: "scenario-e-approval-token".to_string(),
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

    let events = drain_events_until_terminal(event_rx, Duration::from_secs(30))
        .await
        .expect("a terminal event should arrive within 30s");

    // (A) Terminal is SwarmCompleted, not SwarmCancelled.
    assert!(
        matches!(events.last(), Some(SwarmEvent::SwarmCompleted { .. })),
        "the captured stream's last event is SwarmCompleted; got {:?}",
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

    // (B) Exactly one NodeEvent with phase="retry_attempt".
    let retry_events: Vec<&serde_json::Value> = events
        .iter()
        .filter_map(|e| match e {
            SwarmEvent::NodeEvent { phase, payload, .. } if phase == "retry_attempt" => {
                Some(payload)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        retry_events.len(),
        1,
        "exactly one retry_attempt NodeEvent expected (one retry); got {}",
        retry_events.len()
    );

    // (C) Payload shape: attempt_num=2, wait_secs>=0,
    //     last_error_summary contains "rate limited".
    let payload = retry_events[0];
    assert_eq!(
        payload["attempt_num"].as_u64(),
        Some(2),
        "first retry has attempt_num=2; payload={payload}"
    );
    let wait_secs = payload["wait_secs"]
        .as_f64()
        .expect("wait_secs must be a number");
    assert!(
        wait_secs >= 0.0,
        "wait_secs must be >= 0.0; got {wait_secs}"
    );
    let summary = payload["last_error_summary"]
        .as_str()
        .expect("last_error_summary must be a string");
    assert!(
        summary.to_lowercase().contains("rate limited"),
        "summary should reflect the underlying rate-limit error: got {summary:?}"
    );

    // (D) request_ids_seen has length 2 with both equal
    //     (request_id reused across retries — BK.2 contract).
    let ids = executor.request_ids_seen().await;
    assert_eq!(
        ids.len(),
        2,
        "two attempts expected (one retry); request_ids_seen={ids:?}"
    );
    assert_eq!(
        ids[0], ids[1],
        "request_id must be reused across retries; got {ids:?}"
    );

    // (E) Final NodeCompleted for the "herald" node fires.
    let completed_count = events
        .iter()
        .filter(|e| matches!(e, SwarmEvent::NodeCompleted { .. }))
        .count();
    assert_eq!(
        completed_count, 1,
        "herald node should complete on retry success; got {completed_count} NodeCompleted events"
    );
    let failed_count = events
        .iter()
        .filter(|e| matches!(e, SwarmEvent::NodeFailed { .. }))
        .count();
    assert_eq!(
        failed_count, 0,
        "no NodeFailed events should fire on retry-then-success path; got {failed_count}"
    );

    // (F) Run-scoped BudgetUpdate monotonicity. Mirror of Scenario B,
    //     but FILTERED to run-scoped updates (node_id=None). Scenario E
    //     also produces per-node BudgetUpdates (node_id=Some,
    //     tokens_remaining=0 — per CoordinatorEmitter's per-node emit
    //     contract) emitted by SocialPosterEntry::execute via
    //     ctx.emit.emit_budget_update. Mixing the two breaks
    //     monotonicity (run-scoped tokens=u64::MAX while per-node=0).
    //     Scenario B uses SyntheticCapability which doesn't emit
    //     per-node updates, so its assertion sees only run-scoped.
    let run_scoped_token_updates: Vec<u64> = events
        .iter()
        .filter_map(|e| match e {
            SwarmEvent::BudgetUpdate {
                tokens_remaining,
                node_id,
                ..
            } if node_id.is_none() => Some(*tokens_remaining),
            _ => None,
        })
        .collect();
    assert!(
        !run_scoped_token_updates.is_empty(),
        "expected at least one run-scoped BudgetUpdate event"
    );
    for window in run_scoped_token_updates.windows(2) {
        assert!(
            window[1] <= window[0],
            "run-scoped tokens_remaining must be monotonically non-increasing; got {run_scoped_token_updates:?}"
        );
    }

    // (G) capability_call_log: HeraldHarnessCapability does NOT push
    //     to the shared call log. Single-node "herald" plan → empty log.
    let call_log = scenario.capability_call_log();
    assert!(
        call_log.is_empty(),
        "single-node herald plan should leave capability_call_log empty (HeraldHarnessCapability does not push); got {call_log:?}"
    );

    // ScriptedPublishExecutor consumed both outcomes.
    assert_eq!(
        executor.remaining().await,
        0,
        "all scripted outcomes should have been consumed"
    );
}
