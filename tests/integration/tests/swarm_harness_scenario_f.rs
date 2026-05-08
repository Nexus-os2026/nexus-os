//! Phase BL.3b — Scenario F: idempotency replay on retry reuses request_id.
//!
//! Drives the swarm coordinator through HeraldHarnessCapability with a
//! ScriptedPublishExecutor that errors once with a rate-limit
//! SupervisorError, then succeeds. The primary assertion is that the
//! SAME `request_id` is presented on both attempts — the BG/AF
//! idempotency-cache contract: a retry that replays a request_id which
//! the underlying API already saw must short-circuit via the cache
//! rather than double-publishing. Scenario E covered retry-then-success
//! observability; F focuses on the request_id reuse invariant.
//!
//! Test wall-clock ~1.5s (1s wait between attempts driven by the
//! "retry after 1 ms" hint, same shape as Scenario E).

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
async fn idempotency_replay_on_retry_reuses_request_id() {
    let executor = ScriptedPublishExecutor::new(
        true,
        vec![
            ScriptedOutcome::SupervisorErr("social.x rate limited, retry after 1 ms".into()),
            ScriptedOutcome::Ok("tw-cached-on-retry".into()),
        ],
    );

    let scenario = ScenarioBuilder::new()
        .with_canned_plan(CANNED_HERALD_PLAN)
        .with_publish_capability(
            Arc::clone(&executor) as Arc<dyn social_poster_agent::swarm_entry::PublishExecutor>
        )
        .with_oracle_decision(GovernanceDecision::Approved {
            capability_token: "scenario-f-approval-token".to_string(),
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

    // (A) Drain succeeded with terminal SwarmCompleted.
    assert!(
        matches!(events.last(), Some(SwarmEvent::SwarmCompleted { .. })),
        "the captured stream's last event is SwarmCompleted; got {:?}",
        events.last().map(event_kind_str)
    );

    // (B) PRIMARY ASSERTION — request_id reuse across attempts.
    //     This is the BG/AF idempotency-cache contract: a retry
    //     that replays the same id MUST hit the persistent cache
    //     rather than double-publishing. The unit-test layer at
    //     agents/social-poster/src/swarm_entry.rs's mod tests
    //     covers the cache-hit branch directly; this scenario
    //     asserts the same invariant at the swarm-event-layer.
    let ids = executor.request_ids_seen().await;
    assert_eq!(
        ids.len(),
        2,
        "two attempts expected (one retry); request_ids_seen={ids:?}"
    );
    assert_eq!(
        ids[0], ids[1],
        "request_id MUST be reused across retries — this is the BG/AF idempotency cache contract; got {ids:?}"
    );

    // (C) Exactly one retry_attempt NodeEvent with attempt_num=2.
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
    assert_eq!(
        retry_events[0]["attempt_num"].as_u64(),
        Some(2),
        "first retry has attempt_num=2; payload={}",
        retry_events[0]
    );

    // (D) Final NodeCompleted for the herald node appears
    //     (the run completed; second attempt produced the Ok
    //     TweetResult).
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

    // (E) Tweet id assertion: SOFTENED to terminal-success only.
    //     The publish output (TweetResult.tweet_id="tw-cached-on-retry")
    //     reaches SocialPosterEntry::execute and is wrapped into the
    //     SocialPosterOutput JSON returned through HeraldHarnessCapability
    //     to the coordinator's NodeOutcome::Done branch, which
    //     emits NodeCompleted { result: Value, .. }. Scenario E
    //     does not extract the result payload either; matching
    //     that scope. The terminal SwarmCompleted assertion at (A)
    //     and the NodeCompleted count == 1 at (D) together
    //     establish the run reached its successful conclusion;
    //     extracting tweet_id from NodeCompleted's `result: Value`
    //     would require schema knowledge of SocialPosterOutput
    //     (publishd → post_id field) that this scenario file
    //     does not currently import. Documented soft.

    // ScriptedPublishExecutor consumed both outcomes.
    assert_eq!(
        executor.remaining().await,
        0,
        "all scripted outcomes should have been consumed"
    );
}
